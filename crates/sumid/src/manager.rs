//! Monitor registry: discovery, hotplug, cached state, verified writes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sumi_core::features::{hdr_active, Context as Cx, Page};
use sumi_core::{monitor::monitor_id, Config, Monitor, MonitorState};
use sumi_ddc::discover;
use sumi_proto::VcpReply;
use tracing::{info, warn};

pub struct Manager {
    pub read_only: bool,
    pub config_path: PathBuf,
    pub config: Mutex<Config>,
    pub(crate) appsync: Mutex<crate::appsync::AppSyncState>,
    cache_dir: Option<PathBuf>,
    /// One lock per monitor: its bus is used by one transaction at a time.
    monitors: Mutex<BTreeMap<String, Arc<Mutex<Monitor>>>>,
    /// Last context seen by the watcher, to report OSD-side changes.
    last: Mutex<BTreeMap<String, Watch>>,
    /// DRM connector status snapshot, to notice hotplug cheaply.
    connectors: Mutex<Vec<(String, String)>>,
    /// ASUS monitors seen by EDID whose DDC/CI did not answer: id → error.
    pub problems: Mutex<BTreeMap<String, String>>,
    /// Pending reply from the KWin tiling script, and a lock serialising runs.
    pub(crate) tile_reply: Mutex<Option<std::sync::mpsc::Sender<String>>>,
    pub(crate) ai: Mutex<crate::ai::AiState>,
    /// Model kept for `AiClassifyNow` diagnostics.
    pub(crate) ai_clip: Mutex<Option<Arc<sumi_ai::clip::Clip>>>,
    /// Last GameVisual value written to each monitor by any Sumi writer
    /// (DisplayWidget's shared `PreviousSetMode`).
    pub(crate) last_mode: Mutex<BTreeMap<String, u16>>,
    pub(crate) tile_lock: Mutex<()>,
    pub(crate) firmware: Mutex<crate::firmware::FwState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Watch {
    pub context: Cx,
    pub brightness: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorSummary {
    pub id: String,
    pub model: String,
    pub line: sumi_core::ProductLine,
    pub oled: bool,
    pub bus: u32,
    pub fw: u16,
    pub connector: Option<String>,
}

fn connector_status() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let status = std::fs::read_to_string(e.path().join("status")).ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), status.trim().to_string()))
        })
        .collect();
    v.sort();
    v
}

impl Manager {
    pub fn new(read_only: bool, config_path: PathBuf) -> Result<Self> {
        let config = Config::load(&config_path)?;
        Ok(Manager {
            read_only,
            config_path,
            config: Mutex::new(config),
            appsync: Mutex::new(Default::default()),
            cache_dir: dirs::cache_dir().map(|d| d.join("sumi")),
            monitors: Mutex::new(BTreeMap::new()),
            last: Mutex::new(BTreeMap::new()),
            connectors: Mutex::new(Vec::new()),
            problems: Mutex::new(BTreeMap::new()),
            tile_reply: Mutex::new(None),
            ai: Mutex::new(Default::default()),
            ai_clip: Mutex::new(None),
            last_mode: Mutex::new(BTreeMap::new()),
            tile_lock: Mutex::new(()),
            firmware: Mutex::new(Default::default()),
        })
    }

    /// Has any DRM connector changed status since the last call?
    pub fn connectors_changed(&self) -> bool {
        let now = connector_status();
        let mut last = self.connectors.lock().unwrap();
        let changed = *last != now;
        *last = now;
        changed
    }

    /// Attach new ASUS monitors and drop vanished ones. Returns (added, removed).
    pub fn refresh(&self) -> (Vec<String>, Vec<String>) {
        let found: BTreeMap<String, sumi_ddc::Found> =
            discover().into_iter().filter(|f| f.is_asus()).map(|f| (monitor_id(&f), f)).collect();
        let mut mons = self.monitors.lock().unwrap();
        let removed: Vec<String> = mons.keys().filter(|k| !found.contains_key(*k)).cloned().collect();
        self.problems.lock().unwrap().retain(|k, _| found.contains_key(k));
        for k in &removed {
            mons.remove(k);
            self.last.lock().unwrap().remove(k);
            info!("detached {k}");
        }
        let mut added = Vec::new();
        for (k, f) in found {
            if mons.contains_key(&k) {
                continue;
            }
            match Monitor::attach_cached(f, self.cache_dir.as_deref()) {
                Ok(mut m) => {
                    m.display.allow_writes(!self.read_only);
                    info!(
                        "attached {k}: FW {} VCP scheme {}.{} {:?}",
                        m.fw.fw_version, m.fw.scheme_major, m.fw.scheme_minor, m.line
                    );
                    mons.insert(k.clone(), Arc::new(Mutex::new(m)));
                    self.problems.lock().unwrap().remove(&k);
                    added.push(k);
                }
                Err(e) => {
                    warn!("{k}: attach failed: {e:#}");
                    self.problems.lock().unwrap().insert(k, format!("{e:#}"));
                }
            }
        }
        (added, removed)
    }

    pub(crate) fn get(&self, id: &str) -> Result<Arc<Mutex<Monitor>>> {
        self.monitors.lock().unwrap().get(id).cloned().with_context(|| format!("no monitor {id:?}"))
    }

    pub fn list(&self) -> Vec<MonitorSummary> {
        let mons: Vec<_> = self.monitors.lock().unwrap().values().cloned().collect();
        mons.iter()
            .map(|m| {
                let m = m.lock().unwrap();
                MonitorSummary {
                    id: m.id(),
                    model: m.model().to_string(),
                    line: m.line,
                    oled: m.profile().is_oled(),
                    bus: m.found.bus,
                    fw: m.fw.fw_version,
                    connector: m.found.connector.clone(),
                }
            })
            .collect()
    }

    pub fn state(&self, id: &str, pages: Option<Vec<Page>>) -> Result<MonitorState> {
        let m = self.get(id)?;
        let mut m = m.lock().unwrap();
        let st = m.state(pages.as_deref());
        Ok(st)
    }

    pub fn apply(&self, id: &str, feature: &str, value: u16) -> Result<String> {
        let r = self.apply_one(id, feature, value)?;
        // Mode Sync: mirror GameVisual-page changes to same-model monitors.
        let page = sumi_core::features::by_id(feature).map(|f| f.page);
        if self.config.lock().unwrap().mode_sync && page == Some(Page::GameVisual) && feature != "reset" {
            let model = id.split('@').next().unwrap_or_default().to_string();
            let others: Vec<String> =
                self.connectors().into_iter().map(|(k, _)| k).filter(|k| k != id && k.split('@').next() == Some(model.as_str())).collect();
            for other in others {
                if let Err(e) = self.apply_one(&other, feature, value) {
                    warn!("Mode Sync to {other}: {e:#}");
                }
            }
        }
        Ok(r)
    }

    fn apply_one(&self, id: &str, feature: &str, value: u16) -> Result<String> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        if feature == "gamevisual" {
            self.leave_hdr_for_mode(id, true)?;
        }
        let m = self.get(id)?;
        let mut m = m.lock().unwrap();
        match m.apply(feature, value) {
            Ok(s) => {
                if feature == "gamevisual" {
                    self.last_mode.lock().unwrap().insert(id.to_string(), value);
                }
                info!("{id}: {s}");
                Ok(s)
            }
            Err(e) => {
                warn!("{id}: {feature}={value} failed: {e:#}");
                Err(e)
            }
        }
    }

    pub fn raw_get(&self, id: &str, code: u8) -> Result<(u16, u16)> {
        let m = self.get(id)?;
        let mut m = m.lock().unwrap();
        let r = m.get(code)?;
        Ok((r.current, r.max))
    }

    pub fn capabilities(&self, id: &str) -> Result<String> {
        let m = self.get(id)?;
        let m = m.lock().unwrap();
        Ok(serde_json::to_string(&m.caps)?)
    }

    /// Read the context registers of every monitor; return ids whose context
    /// changed (OSD joystick, HDR signal, power saving) since the last poll.
    pub fn poll(&self) -> Vec<(String, Watch)> {
        let mons: Vec<(String, Arc<Mutex<Monitor>>)> =
            self.monitors.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut changed = Vec::new();
        for (id, m) in mons {
            let w = {
                let mut m = m.lock().unwrap();
                let new_vcp = m.fw.new_vcp();
                let regs: BTreeMap<u8, Option<VcpReply>> =
                    [0xDC, 0xE2, 0xE1, 0x14, 0xE6, 0x10].into_iter().map(|c| (c, m.display.get_vcp(c).ok())).collect();
                let reg = |c: u8| regs.get(&c).copied().flatten();
                if reg(0xDC).is_none() && reg(0xE2).is_none() {
                    continue; // asleep or busy; try again next round
                }
                Watch { context: Cx::from_regs(new_vcp, reg), brightness: reg(0x10).map(|r| r.current) }
            };
            let mut last = self.last.lock().unwrap();
            if last.get(&id) != Some(&w) {
                let first = !last.contains_key(&id);
                last.insert(id.clone(), w.clone());
                if !first {
                    changed.push((id, w));
                }
            }
        }
        changed
    }
}

impl Manager {
    /// (monitor id, DRM connector) for every attached monitor.
    pub fn connectors(&self) -> Vec<(String, Option<String>)> {
        let mons: Vec<_> = self.monitors.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        mons.into_iter().map(|(k, m)| (k, m.lock().unwrap().found.connector.clone())).collect()
    }

    /// Monitor holding the active window, else the first one.
    pub fn target_monitor(&self) -> Result<String> {
        let out = self.appsync.lock().unwrap().active_output.clone();
        let all = self.connectors();
        all.iter()
            .find(|(_, c)| c.is_some() && *c == out)
            .or(all.first())
            .map(|(id, _)| id.clone())
            .context("no monitor attached")
    }

    pub fn save_config(&self, cfg: Config) -> Result<()> {
        cfg.save(&self.config_path)?;
        *self.config.lock().unwrap() = cfg;
        self.reset_app_sync();
        Ok(())
    }

    fn connector_of(&self, id: &str) -> Result<String> {
        self.connectors().into_iter().find(|(k, _)| k == id).and_then(|(_, c)| c).context("monitor's display output is unknown")
    }

    /// KDE's HDR state for the monitor's output (kscreen-doctor).
    pub fn host_hdr(&self, id: &str) -> Result<bool> {
        let conn = self.connector_of(id)?;
        let out = std::process::Command::new("kscreen-doctor").arg("-j").output().context("running kscreen-doctor")?;
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("parsing kscreen-doctor -j")?;
        v["outputs"]
            .as_array()
            .and_then(|a| a.iter().find(|o| o["name"] == conn.as_str()))
            .and_then(|o| o["hdr"].as_bool())
            .context("output not reported by KScreen")
    }

    /// Turn KDE HDR on/off for the monitor's output.
    pub fn set_host_hdr(&self, id: &str, on: bool) -> Result<()> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let conn = self.connector_of(id)?;
        let arg = format!("output.{conn}.hdr.{}", if on { "enable" } else { "disable" });
        let st = std::process::Command::new("kscreen-doctor").arg(&arg).status().context("running kscreen-doctor")?;
        if !st.success() {
            bail!("kscreen-doctor {arg} failed");
        }
        info!("{id}: KDE HDR {}", if on { "on" } else { "off" });
        Ok(())
    }

    pub fn set_target_mode(&self, on: bool) -> Result<()> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let strength = self.config.lock().unwrap().target_mode.strength;
        let conn = zbus::blocking::Connection::session()?;
        crate::kwin::set_target_mode(&conn, on, strength)?;
        info!("Target Mode {}", if on { "on" } else { "off" });
        Ok(())
    }

    pub fn target_mode(&self) -> Result<bool> {
        let conn = zbus::blocking::Connection::session()?;
        crate::kwin::target_mode_active(&conn)
    }

    pub fn export_profile(&self, id: &str) -> Result<serde_json::Value> {
        let m = self.get(id)?;
        let mut m = m.lock().unwrap();
        m.export_profile()
    }

    pub fn import_profile(&self, id: &str, doc: &serde_json::Value, switch_mode: bool) -> Result<Vec<String>> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let m = self.get(id)?;
        let mut m = m.lock().unwrap();
        let log = m.import_profile(doc, switch_mode)?;
        info!("{id}: imported settings: {}", log.join("; "));
        Ok(log)
    }

    /// Run one hot-key action on the target monitor.
    pub fn hotkey(&self, action: &str) -> Result<()> {
        if self.read_only {
            bail!("read-only");
        }
        let act = parse_hotkey(action).with_context(|| format!("unknown action {action:?}"))?;
        if act == Hot::TargetMode {
            let on = self.target_mode().unwrap_or(false);
            return self.set_target_mode(!on);
        }
        let id = self.target_monitor()?;
        if act == Hot::Hdr {
            let on = self.host_hdr(&id)?;
            return self.set_host_hdr(&id, !on);
        }
        let step = self.config.lock().unwrap().hotkeys.step.max(1);
        let st = self.state(&id, None)?;
        match hot_target(&act, &st, step)? {
            Some((fid, value)) => self.apply(&id, &fid, value).map(|_| ()),
            None => Ok(()),
        }
    }
}

/// What a hot-key action asks for, before it touches a monitor.
#[derive(Debug, PartialEq, Eq)]
enum Hot<'a> {
    TargetMode,
    Hdr,
    /// An EZ OSD key (the 0xEB value).
    Osd(u16),
    PipToggle,
    KvmToggle,
    PowerSavingToggle,
    GameVisualStep { forward: bool },
    /// Raise or lower a feature (`<feature id>_up` / `_down`).
    Nudge { feature: &'a str, up: bool },
}

/// Read an action id from `sumi_core::hotkeys::ACTIONS`; `None` if it is not one.
fn parse_hotkey(action: &str) -> Option<Hot<'_>> {
    Some(match action {
        "target_mode_toggle" => Hot::TargetMode,
        "hdr_toggle" => Hot::Hdr,
        "osd_close" => Hot::Osd(0),
        "osd_show" => Hot::Osd(1),
        "osd_up" => Hot::Osd(2),
        "osd_down" => Hot::Osd(3),
        "osd_right" => Hot::Osd(4),
        "osd_left" => Hot::Osd(5),
        "osd_enter" => Hot::Osd(6),
        "osd_back" => Hot::Osd(7),
        "pip_toggle" => Hot::PipToggle,
        "kvm_toggle" => Hot::KvmToggle,
        "power_saving_toggle" => Hot::PowerSavingToggle,
        "gamevisual_next" => Hot::GameVisualStep { forward: true },
        "gamevisual_prev" => Hot::GameVisualStep { forward: false },
        a if a.starts_with("osd_") => return None,
        a => {
            let (feature, dir) = a.rsplit_once('_').filter(|(f, _)| !f.is_empty())?;
            Hot::Nudge { feature, up: match dir { "up" => true, "down" => false, _ => return None } }
        }
    })
}

/// The feature write an action comes to on this monitor: (feature id, value),
/// or `None` when there is nothing to change (already at the end of the
/// range, or a global action that writes no feature).
fn hot_target(act: &Hot, st: &MonitorState, step: u16) -> Result<Option<(String, u16)>> {
    let feat = |fid: &str| st.features.iter().find(|f| f.id == fid && f.supported);
    let (fid, value) = match *act {
        Hot::TargetMode | Hot::Hdr => return Ok(None),
        Hot::Osd(key) => ("osd_key", key),
        Hot::PipToggle => {
            let f = feat("pip_mode").context("PIP/PBP not supported")?;
            let cur = f.value.unwrap_or(0);
            ("pip_mode", if cur == 0 { 1 } else { 0 })
        }
        Hot::KvmToggle => {
            // Manual upstream B ↔ C (DisplayWidget's held-modifier switch).
            let f = feat("kvm").context("KVM not supported")?;
            ("kvm", if f.value == Some(2) { 3 } else { 2 })
        }
        Hot::PowerSavingToggle => {
            let f = feat("power_saving").context("power saving not supported")?;
            ("power_saving", if f.value == Some(1) { 0 } else { 1 })
        }
        Hot::GameVisualStep { forward } => {
            let f = feat("gamevisual").context("GameVisual not supported")?;
            let n = f.options.len();
            if n == 0 {
                bail!("no GameVisual modes");
            }
            let i = f.options.iter().position(|o| Some(o.value) == f.value).unwrap_or(0);
            let j = if forward { (i + 1) % n } else { (i + n - 1) % n };
            ("gamevisual", f.options[j].value)
        }
        Hot::Nudge { feature: fid, up } => {
            let f = feat(fid).with_context(|| format!("{fid} not available"))?;
            let cur = f.value.unwrap_or(0) as i32;
            let next = if !f.options.is_empty() {
                let i = f.options.iter().position(|o| o.value as i32 == cur).unwrap_or(0) as i32;
                let j = (i + if up { 1 } else { -1 }).clamp(0, f.options.len() as i32 - 1);
                f.options[j as usize].value as i32
            } else {
                let s = if matches!(fid, "brightness" | "contrast" | "saturation") { step as i32 } else { 1 };
                (cur + if up { s } else { -s }).clamp(0, f.max.unwrap_or(100) as i32)
            };
            if next == cur {
                return Ok(None);
            }
            (fid, next as u16)
        }
    };
    Ok(Some((fid.to_string(), value)))
}

impl Manager {
    /// GameVisual modes are SDR modes: if the monitor is showing HDR, turn KDE
    /// HDR off and give the monitor time to switch (DisplayWidget waits 3 s).
    /// With `allowed == false` (App Sync with the option off) it errors instead.
    pub fn leave_hdr_for_mode(&self, id: &str, allowed: bool) -> Result<()> {
        let hdr_on_monitor = self.raw_get(id, 0xE2).map(|(cur, _)| hdr_active(cur)).unwrap_or(false);
        match hdr_step(hdr_on_monitor, || self.host_hdr(id).unwrap_or(false), allowed) {
            HdrStep::Nothing => Ok(()),
            HdrStep::Refuse => bail!("HDR is on"),
            HdrStep::TurnOff => {
                self.set_host_hdr(id, false)?;
                std::thread::sleep(std::time::Duration::from_secs(3));
                Ok(())
            }
        }
    }
}

/// What `leave_hdr_for_mode` does about HDR.
#[derive(Debug, PartialEq, Eq)]
enum HdrStep {
    /// SDR on the monitor or in KDE: nothing in the way.
    Nothing,
    /// HDR is on but App Sync may not turn it off.
    Refuse,
    TurnOff,
}

/// Decide from what the monitor reports; `host_hdr` (a `kscreen-doctor`
/// call) is asked only when the monitor itself shows HDR.
fn hdr_step(monitor_hdr: bool, host_hdr: impl FnOnce() -> bool, allowed: bool) -> HdrStep {
    if !monitor_hdr || !host_hdr() {
        HdrStep::Nothing
    } else if allowed {
        HdrStep::TurnOff
    } else {
        HdrStep::Refuse
    }
}

impl Manager {
    /// The monitor's sRGB GameVisual value: 0x0A with sRGB calibration, else 3.
    pub fn srgb_value(&self, id: &str) -> u16 {
        let Ok(m) = self.get(id) else { return 3 };
        let m = m.lock().unwrap();
        let f = sumi_core::features::by_id("gamevisual").expect("catalogue entry");
        if m.profile().options(f).iter().any(|o| o.value == 0x0A) { 0x0A } else { 3 }
    }
}

#[cfg(test)]
impl Manager {
    /// A manager with the default config (no file is read) and no monitors.
    pub(crate) fn for_test(read_only: bool) -> Manager {
        Manager::new(read_only, PathBuf::from("/nonexistent/sumi-test/config.toml")).expect("default config")
    }

    /// Register a ready-made monitor (on a fake link) under its own id.
    pub(crate) fn attach_test(&self, m: Monitor) -> String {
        let id = m.id();
        self.monitors.lock().unwrap().insert(id.clone(), Arc::new(Mutex::new(m)));
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sumi_core::hotkeys::ACTIONS;
    use sumi_core::testing::{self, Fake};

    /// Actions that talk to KDE (Target Mode over D-Bus, HDR through kscreen-doctor).
    /// They are checked by `parse_hotkey` only, never run.
    const KDE_ACTIONS: &[&str] = &["target_mode_toggle", "hdr_toggle"];

    /// A manager with one idle PG32UCDM3 on DP-3, and a handle on its link.
    fn mgr() -> (Manager, Fake, String) {
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        let id = mgr.attach_test(testing::attach(&fake, Some("DP-3")));
        fake.clear_logs();
        (mgr, fake, id)
    }

    // Hot keys

    #[test]
    fn every_registered_action_is_known_to_the_dispatch() {
        let mut seen = std::collections::BTreeSet::new();
        for (id, label) in ACTIONS {
            assert!(parse_hotkey(id).is_some(), "{id} falls through to \"unknown action\"");
            assert!(seen.insert(*id), "{id} is listed twice");
            assert!(!label.is_empty());
        }
        assert!(KDE_ACTIONS.iter().all(|a| ACTIONS.iter().any(|(id, _)| id == a)));
    }

    #[test]
    fn nudged_actions_name_features_the_catalogue_has() {
        let mut nudged = 0;
        for (id, _) in ACTIONS {
            if let Some(Hot::Nudge { feature, .. }) = parse_hotkey(id) {
                nudged += 1;
                let f = sumi_core::features::by_id(feature).unwrap_or_else(|| panic!("{id}: no feature {feature:?}"));
                assert!(matches!(f.encoding, sumi_core::features::Encoding::Range | sumi_core::features::Encoding::Choice), "{id}");
            }
        }
        assert_eq!(nudged, 10);
    }

    #[test]
    fn osd_actions_send_the_key_with_the_same_name() {
        let keys = sumi_core::features::by_id("osd_key").unwrap().options;
        for (id, _) in ACTIONS.iter().filter(|(id, _)| id.starts_with("osd_")) {
            let Some(Hot::Osd(v)) = parse_hotkey(id) else { panic!("{id}") };
            let label = keys.iter().find(|o| o.value == v).map(|o| o.label.to_lowercase());
            assert_eq!(format!("osd_{}", label.unwrap()), *id);
        }
    }

    #[test]
    fn unknown_actions_are_not_guessed() {
        for a in ["", "nonsense", "osd_sideways", "osd_", "brightness_sideways", "brightness_", "_up", "brightness"] {
            assert_eq!(parse_hotkey(a), None, "{a:?}");
        }
        // A feature id that is not a catalogue entry parses (it fails later as "not available").
        assert_eq!(parse_hotkey("flux_up"), Some(Hot::Nudge { feature: "flux", up: true }));
        assert_eq!(parse_hotkey("shadow_boost_down"), Some(Hot::Nudge { feature: "shadow_boost", up: false }));
        let (mgr, fake, _) = mgr();
        let e = mgr.hotkey("osd_sideways").unwrap_err().to_string();
        assert!(e.contains("unknown action"), "{e}");
        assert!(mgr.hotkey("flux_up").unwrap_err().to_string().contains("flux not available"));
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn every_other_action_runs_against_the_monitor() {
        // Set up so that no action is a no-op (brightness and friends are mid-range).
        let (mgr, fake, _) = mgr();
        for (code, v) in [(0x10, 50), (0x12, 50), (0x8A, 50), (0xE6, 1), (0xE5, 2)] {
            fake.set_current(code, v);
        }
        fake.clear_logs();
        for (id, _) in ACTIONS.iter().filter(|(id, _)| !KDE_ACTIONS.contains(id)) {
            let before = fake.writes().len();
            mgr.hotkey(id).unwrap_or_else(|e| panic!("{id}: {e:#}"));
            assert!(fake.writes().len() > before, "{id} wrote nothing");
        }
    }

    #[test]
    fn hot_keys_write_the_expected_values() {
        let (mgr, fake, _) = mgr();
        for (code, v) in [(0x10, 50), (0x12, 50), (0x8A, 50), (0xE6, 1), (0xE5, 2)] {
            fake.set_current(code, v);
        }
        fake.clear_logs();
        let run = |a: &str| {
            fake.clear_logs();
            mgr.hotkey(a).unwrap();
            fake.writes()
        };
        // Brightness, contrast and saturation move by the configured step (5).
        assert_eq!(run("brightness_up"), [(0x10, 55)]);
        assert_eq!(run("brightness_down"), [(0x10, 50)]);
        assert_eq!(run("contrast_up"), [(0x12, 55)]);
        assert_eq!(run("saturation_down"), [(0x8A, 45)]);
        // Other ranges move one at a time.
        assert_eq!(run("blue_light_up"), [(0xE6, 2)]);
        // Choices step through their options.
        assert_eq!(run("shadow_boost_up"), [(0xE5, 3)]);
        assert_eq!(run("shadow_boost_down"), [(0xE5, 2)]);
        assert_eq!(run("pip_toggle"), [(0xF4, 1)]);
        assert_eq!(run("pip_toggle"), [(0xF4, 0)]);
        assert_eq!(run("kvm_toggle"), [(0xF3, 3)]);
        assert_eq!(run("kvm_toggle"), [(0xF3, 2)]);
        assert_eq!(run("power_saving_toggle"), [(0xE1, 1)]);
        assert_eq!(run("power_saving_toggle"), [(0xE1, 0)]);
        assert_eq!(run("osd_enter"), [(0xEB, 6)]);
        assert_eq!(run("osd_back"), [(0xEB, 7)]);
        // GameVisual cycles through the monitor's modes in the tile order, and wraps.
        assert_eq!(run("gamevisual_next"), [(0xDC, 1)]);
        assert_eq!(run("gamevisual_prev"), [(0xDC, 7)]);
        fake.set_current(0xDC, 6);
        assert_eq!(run("gamevisual_prev"), [(0xDC, 9)]);
        assert_eq!(run("gamevisual_next"), [(0xDC, 6)]);
    }

    #[test]
    fn hot_keys_stop_at_the_ends_of_a_range() {
        let (mgr, fake, _) = mgr();
        // Brightness is 100/100: nothing to do, nothing written.
        mgr.hotkey("brightness_up").unwrap();
        assert!(fake.writes().is_empty());
        fake.set_current(0x10, 3);
        mgr.hotkey("brightness_down").unwrap();
        assert_eq!(fake.writes(), [(0x10, 0)]);
        fake.clear_logs();
        mgr.hotkey("brightness_down").unwrap();
        assert!(fake.writes().is_empty());
        // The first and last option are the ends of a choice.
        fake.set_current(0xE5, 0);
        mgr.hotkey("shadow_boost_down").unwrap();
        fake.set_current(0xE5, 4);
        mgr.hotkey("shadow_boost_up").unwrap();
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn the_step_comes_from_the_config_and_is_at_least_one() {
        let (mgr, fake, _) = mgr();
        fake.set_current(0x10, 50);
        mgr.config.lock().unwrap().hotkeys.step = 20;
        mgr.hotkey("brightness_up").unwrap();
        mgr.config.lock().unwrap().hotkeys.step = 0;
        mgr.hotkey("brightness_up").unwrap();
        assert_eq!(fake.writes(), [(0x10, 70), (0x10, 71)]);
    }

    #[test]
    fn hot_keys_are_refused_when_read_only_or_without_a_monitor() {
        let ro = Manager::for_test(true);
        let fake = Fake::pg32ucdm3();
        ro.attach_test(testing::attach(&fake, None));
        fake.clear_logs();
        for (id, _) in ACTIONS {
            assert!(ro.hotkey(id).unwrap_err().to_string().contains("read-only"), "{id}");
        }
        assert!(fake.writes().is_empty() && fake.reads().is_empty());
        let empty = Manager::for_test(false);
        assert!(empty.hotkey("pip_toggle").unwrap_err().to_string().contains("no monitor attached"));
    }

    #[test]
    fn hot_key_reports_what_the_monitor_lacks() {
        // A capability string with no PIP/PBP, KVM or blue light filter.
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        fake.set_caps("(prot(monitor)model(PG32UCDM3)vcp(10 12 DC E1 E2 FD))");
        mgr.attach_test(testing::attach(&fake, None));
        fake.clear_logs();
        for (action, what) in [
            ("pip_toggle", "PIP/PBP not supported"),
            ("kvm_toggle", "KVM not supported"),
            ("blue_light_up", "blue_light not available"),
            ("shadow_boost_down", "shadow_boost not available"),
        ] {
            let e = mgr.hotkey(action).unwrap_err().to_string();
            assert!(e.contains(what), "{action}: {e}");
        }
        assert!(fake.writes().is_empty());
    }

    // The shared last mode

    #[test]
    fn the_last_mode_follows_gamevisual_writes_only() {
        let (mgr, _fake, id) = mgr();
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), None);
        mgr.apply(&id, "brightness", 40).unwrap();
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), None);
        mgr.apply(&id, "gamevisual", 5).unwrap();
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), Some(&5));
        mgr.apply(&id, "gamevisual", 0x0A).unwrap();
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), Some(&0x0A));
    }

    #[test]
    fn a_failed_mode_write_does_not_move_the_last_mode() {
        let (mgr, fake, id) = mgr();
        mgr.apply(&id, "gamevisual", 5).unwrap();
        // Refused by the catalogue, then ignored by the monitor.
        assert!(mgr.apply(&id, "gamevisual", 99).is_err());
        fake.ignore_writes(0xDC);
        assert!(mgr.apply(&id, "gamevisual", 6).is_err());
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), Some(&5));
    }

    #[test]
    fn writes_are_refused_when_sumid_is_read_only() {
        let ro = Manager::for_test(true);
        let fake = Fake::pg32ucdm3();
        let id = ro.attach_test(testing::attach(&fake, None));
        fake.clear_logs();
        let e = ro.apply(&id, "brightness", 40).unwrap_err().to_string();
        assert_eq!(e, "sumid is running read-only");
        assert!(ro.import_profile(&id, &serde_json::json!({}), false).is_err());
        assert!(ro.set_host_hdr(&id, true).is_err());
        assert!(ro.set_target_mode(true).is_err());
        assert!(fake.writes().is_empty() && fake.reads().is_empty());
        // Reads still work.
        assert_eq!(ro.raw_get(&id, 0x10).unwrap(), (100, 100));
    }

    #[test]
    fn mode_sync_mirrors_gamevisual_changes_to_same_model_monitors() {
        let (mgr, first, id) = mgr();
        let second = Fake::pg32ucdm3();
        let id2 = mgr.attach_test(testing::attach_on(&second, 6, Some("DP-1")));
        assert_ne!(id, id2);
        second.clear_logs();
        // Off by default.
        mgr.apply(&id, "gamevisual", 5).unwrap();
        assert!(second.writes().is_empty());
        mgr.config.lock().unwrap().mode_sync = true;
        mgr.apply(&id, "gamevisual", 6).unwrap();
        assert_eq!(second.writes(), [(0xDC, 6)]);
        // Only the GameVisual page is mirrored, and not the reset button.
        mgr.apply(&id, "brightness", 20).unwrap();
        mgr.apply(&id, "crosshair", 7).unwrap();
        mgr.apply(&id, "reset", 1).unwrap();
        assert_eq!(second.writes(), [(0xDC, 6), (0x10, 20)]);
        assert_eq!(first.current(0xDC), 6);
    }

    #[test]
    fn a_mode_mismatch_on_import_asks_to_switch_first() {
        // The D-Bus layer turns this wording into its ModeMismatch error.
        let (mgr, fake, id) = mgr();
        let doc = serde_json::json!({ "ModelName": "PG32UCDM3", "DisplayMode": 5, "vcp": { "10": "50" } });
        let e = mgr.import_profile(&id, &doc, false).unwrap_err().to_string();
        assert!(e.contains("switch first"), "{e}");
        assert!(fake.writes().is_empty());
        let log = mgr.import_profile(&id, &doc, true).unwrap();
        assert_eq!(log, ["0xDC GameVisual -> 5", "0x10 = 50"]);
        assert!(mgr.export_profile(&id).unwrap()["vcp"]["10"] == "50");
    }

    // HDR

    #[test]
    fn hdr_is_only_asked_about_in_kde_when_the_monitor_shows_it() {
        let mut asked = 0;
        assert_eq!(hdr_step(false, || { asked += 1; true }, true), HdrStep::Nothing);
        assert_eq!(asked, 0, "KDE should not be asked when the monitor is SDR");
        assert_eq!(hdr_step(true, || { asked += 1; false }, true), HdrStep::Nothing);
        assert_eq!(asked, 1);
        assert_eq!(hdr_step(true, || true, true), HdrStep::TurnOff);
        assert_eq!(hdr_step(true, || true, false), HdrStep::Refuse);
        // Allowed or not makes no difference when there is no HDR in the way.
        assert_eq!(hdr_step(false, || true, false), HdrStep::Nothing);
        assert_eq!(hdr_step(true, || false, false), HdrStep::Nothing);
    }

    #[test]
    fn kde_hdr_that_cannot_be_read_counts_as_off() {
        // The monitor shows HDR but its output is unknown: nothing can be asked of
        // KDE, so nothing is in the way (and nothing is run).
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        fake.set_current(0xE2, 0x0102);
        let id = mgr.attach_test(testing::attach(&fake, None));
        mgr.leave_hdr_for_mode(&id, true).unwrap();
        mgr.leave_hdr_for_mode(&id, false).unwrap();
        mgr.apply(&id, "gamevisual", 5).unwrap();
        assert_eq!(fake.current(0xDC), 5);
    }

    #[test]
    fn leaving_hdr_is_a_no_op_for_an_sdr_monitor() {
        let (mgr, fake, id) = mgr();
        mgr.leave_hdr_for_mode(&id, true).unwrap();
        mgr.leave_hdr_for_mode(&id, false).unwrap();
        // 0xFE is "no HDR", like 0.
        fake.set_current(0xE2, 0x00FE);
        mgr.leave_hdr_for_mode(&id, false).unwrap();
        assert!(fake.writes().is_empty());
        assert_eq!(fake.reads(), [0xE2; 3]);
    }

    // Watching the monitor

    #[test]
    fn poll_reports_changes_made_on_the_monitor() {
        let (mgr, fake, id) = mgr();
        // The first look only records.
        assert!(mgr.poll().is_empty());
        assert!(mgr.poll().is_empty());
        fake.set_current(0xE2, 0x0102);
        let changed = mgr.poll();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0, id);
        assert!(changed[0].1.context.hdr);
        assert!(mgr.poll().is_empty());
        // Brightness alone counts (the OSD joystick), and the mode.
        fake.set_current(0x10, 40);
        assert_eq!(mgr.poll()[0].1.brightness, Some(40));
        fake.set_current(0xDC, 5);
        assert_eq!(mgr.poll()[0].1.context.mode, 5);
        assert!(fake.writes().is_empty());
    }

    #[test]
    fn poll_reads_the_context_like_state_does() {
        let (mgr, fake, _) = mgr();
        mgr.poll();
        fake.set_current(0xE1, 1);
        fake.set_current(0x14, 0x0B);
        fake.set_current(0xE6, 4);
        let w = mgr.poll().remove(0).1;
        assert!(w.context.power_saving && w.context.user_color && w.context.blf_max);
        assert_eq!(w.brightness, Some(100));
    }

    #[test]
    fn poll_ignores_power_saving_on_the_old_scheme() {
        // 0xE1 is ASCR there: a 1 means "ASCR on", not "power saving".
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        fake.set_reg(0xEF, 0x0112, 0xB1);
        mgr.attach_test(testing::attach(&fake, None));
        mgr.poll();
        fake.set_current(0xE1, 1);
        fake.set_current(0x10, 90);
        let w = mgr.poll().remove(0).1;
        assert!(!w.context.power_saving);
    }

    #[test]
    fn poll_skips_a_monitor_that_does_not_answer() {
        let (mgr, fake, _) = mgr();
        mgr.poll();
        fake.fail_reads(0xDC);
        fake.fail_reads(0xE2);
        fake.set_current(0x10, 10);
        assert!(mgr.poll().is_empty(), "asleep: no change reported, none recorded");
        fake.heal_reads(0xDC);
        fake.heal_reads(0xE2);
        assert_eq!(mgr.poll()[0].1.brightness, Some(10));
    }

    // Registry

    #[test]
    fn target_monitor_is_the_one_with_the_active_window() {
        let (mgr, _first, id) = mgr();
        let id2 = mgr.attach_test(testing::attach_on(&Fake::pg32ucdm3(), 6, Some("DP-1")));
        // Nothing focused: the first by id.
        assert_eq!(mgr.target_monitor().unwrap(), id);
        mgr.appsync.lock().unwrap().active_output = Some("DP-1".into());
        assert_eq!(mgr.target_monitor().unwrap(), id2);
        // An output that is not an ASUS monitor falls back to the first.
        mgr.appsync.lock().unwrap().active_output = Some("HDMI-A-1".into());
        assert_eq!(mgr.target_monitor().unwrap(), id);
        assert!(Manager::for_test(false).target_monitor().is_err());
    }

    #[test]
    fn the_registry_describes_and_reads_its_monitors() {
        let (mgr, _fake, id) = mgr();
        let l = mgr.list();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].id.as_str(), l[0].model.as_str(), l[0].bus, l[0].fw), (id.as_str(), "PG32UCDM3", 5, 103));
        assert!(l[0].oled && l[0].connector.as_deref() == Some("DP-3"));
        assert_eq!(mgr.state(&id, Some(vec![Page::DeviceInfo])).unwrap().features.len(), 3);
        let e = mgr.state("nope@i2c-9", None).unwrap_err().to_string();
        assert_eq!(e, "no monitor \"nope@i2c-9\"");
        assert!(mgr.raw_get("nope@i2c-9", 0x10).is_err());
        let caps: serde_json::Value = serde_json::from_str(&mgr.capabilities(&id).unwrap()).unwrap();
        assert_eq!(caps["fields"]["model"], "PG32UCDM3");
    }

    #[test]
    fn the_srgb_value_depends_on_the_monitors_calibration() {
        let (mgr, fake, id) = mgr();
        assert_eq!(mgr.srgb_value(&id), 0x0A);
        // Without 0x0A in the DC list the monitor uses plain 3; an unknown monitor too.
        let plain = Manager::for_test(false);
        fake.set_caps(&sumi_core::testing::PG32UCDM3_CAPS.replace("0A )", ")").replace("09 0A)", "09)"));
        let id2 = plain.attach_test(testing::attach(&fake, None));
        assert_eq!(plain.srgb_value(&id2), 3);
        assert_eq!(plain.srgb_value("nope@i2c-9"), 3);
    }
}
