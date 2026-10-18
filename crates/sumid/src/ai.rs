//! AI Visual runner: about once a second, capture each ASUS monitor's output
//! from KWin, classify it with CLIP and let DisplayWidget's decision logic
//! pick the GameVisual mode. App Sync rules take precedence (as in
//! DisplayWidget), and nothing runs while the monitor shows HDR.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sumi_ai::{capture, clip, model, Engine, Mode, Settings, Verdict};
use sumi_core::features::hdr_active;
use sumi_core::strings;
use tracing::{info, warn};
use zbus::blocking::Connection;

use crate::manager::Manager;

#[derive(Debug, Clone, Default, Serialize)]
pub struct MonitorAi {
    pub verdict: Option<Verdict>,
    /// Mode AI Visual last sent to this monitor.
    pub last_sent: Option<Mode>,
    /// Why the monitor was skipped this round, if it was.
    pub paused: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct AiState {
    pub running: bool,
    pub error: Option<String>,
    pub monitors: HashMap<String, MonitorAi>,
    /// (bytes done, total) while a model download runs.
    pub download: Option<(u64, u64)>,
    #[serde(skip)]
    engines: HashMap<String, Engine>,
    #[serde(skip)]
    hdr_checked: HashMap<String, (Instant, bool)>,
    #[serde(skip)]
    notification: u32,
    /// Fingerprint and result of the last frame, per monitor (unchanged
    /// screens reuse the result instead of running the model again).
    #[serde(skip)]
    last_frame: HashMap<String, (u64, sumi_ai::Observation)>,
    /// Engines fed only by `AiClassifyNow` (the panel's preview while AI
    /// Visual is off): same smoothing, never switches anything.
    #[serde(skip)]
    preview: HashMap<String, Engine>,
}

/// Cheap fingerprint of a frame: a 64×36 grid of pixels.
fn fingerprint(c: &capture::Capture) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for gy in 0..36 {
        let y = gy * c.height / 36;
        for gx in 0..64 {
            let i = y * c.stride + (gx * c.width / 64) * 4;
            c.data[i..i + 3].hash(&mut h);
        }
    }
    h.finish()
}

/// Is the output's display powered (DPMS On)?
fn display_on(connector: &str) -> bool {
    std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().split_once('-').is_some_and(|(card, c)| card.starts_with("card") && c == connector))
        .and_then(|e| std::fs::read_to_string(e.path().join("dpms")).ok())
        .is_none_or(|s| s.trim() == "On")
}

/// Is the session locked / the screen saver running?
fn screen_locked(conn: &Connection) -> bool {
    conn.call_method(Some("org.freedesktop.ScreenSaver"), "/ScreenSaver", Some("org.freedesktop.ScreenSaver"), "GetActive", &())
        .ok()
        .and_then(|m| m.body().deserialize::<bool>().ok())
        .unwrap_or(false)
}

/// Why AI Visual leaves a monitor alone this round (`None` = go ahead), in
/// priority order. A lock screen or a dark panel would read as "Office". The
/// checks after `locked` are lazy: the last one costs a DDC read.
fn pause_reason(
    locked: bool,
    display_on: impl FnOnce() -> bool,
    app_rule: impl FnOnce() -> bool,
    hdr: impl FnOnce() -> bool,
) -> Option<&'static str> {
    if locked {
        Some(strings::AI_PAUSED_LOCKED)
    } else if !display_on() {
        Some(strings::AI_PAUSED_DISPLAY_OFF)
    } else if app_rule() {
        Some(strings::AI_PAUSED_APP_RULE)
    } else if hdr() {
        Some(strings::AI_PAUSED_HDR)
    } else {
        None
    }
}

/// The mode to write, if any: AI Visual sends when it wants a mode other than
/// the last one Sumi wrote to the monitor (by AI Visual, App Sync or the app),
/// like DisplayWidget's shared `PreviousSetMode`.
fn mode_to_send(want: Option<(Mode, u16)>, last: Option<u16>, read_only: bool) -> Option<(Mode, u16)> {
    want.filter(|(_, v)| Some(*v) != last && !read_only)
}

fn settings(cfg: &sumi_core::config::AiVisual) -> Settings {
    let parse = |s: &String| serde_json::from_value::<Mode>(serde_json::Value::String(s.clone())).ok();
    Settings { enabled_modes: cfg.modes.iter().filter_map(parse).collect(), fallback: cfg.fallback.as_ref().and_then(parse) }
}

fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Cinema => "Cinema",
        Mode::Scenery => "Scenery",
        Mode::Srgb => "sRGB",
        Mode::User => "User",
        Mode::Racing => "Racing",
        Mode::RtsRpg => "RTS/RPG",
        Mode::Fps => "FPS",
        Mode::Moba => "MOBA",
    }
}

impl Manager {
    /// Is the monitor showing HDR? Checked at most every 10 s (one DDC read).
    fn ai_hdr(&self, id: &str) -> bool {
        let now = Instant::now();
        if let Some((at, on)) = self.ai.lock().unwrap().hdr_checked.get(id) {
            if now - *at < Duration::from_secs(10) {
                return *on;
            }
        }
        let on = self.raw_get(id, 0xE2).map(|(cur, _)| hdr_active(cur)).unwrap_or(false);
        self.ai.lock().unwrap().hdr_checked.insert(id.to_string(), (now, on));
        on
    }

    /// The window App Sync last saw focused, and whether a rule covers it.
    fn ai_rule_applies(&self, connector: &str) -> bool {
        let st = self.appsync.lock().unwrap();
        let Some(w) = st.recent.front() else { return false };
        if w.output != connector {
            return false;
        }
        let cfg = self.config.lock().unwrap();
        cfg.app_sync.enabled && cfg.rule_for(&w.keys()).is_some()
    }

    /// Tell the user about a switch. The app shows its on-screen AI bubble
    /// (DisplayWidget's "Show Notifications"); a KDE notification stands in
    /// only while the app isn't running.
    fn ai_notify(&self, conn: &Connection, text: &str) {
        let app_running = conn
            .call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(crate::APP_NAME,))
            .ok()
            .and_then(|m| m.body().deserialize::<bool>().ok())
            .unwrap_or(false);
        if app_running {
            return;
        }
        let replaces = self.ai.lock().unwrap().notification;
        let hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
        let r = conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &("Sumi", replaces, "io.github.jenaridev.sumi", crate::i18n::tr("AI Visual").as_str(), text, Vec::<&str>::new(), hints, 4000i32),
        );
        if let Ok(id) = r.and_then(|m| m.body().deserialize::<u32>().map_err(Into::into)) {
            self.ai.lock().unwrap().notification = id;
        }
    }

    /// One AI Visual round over every monitor.
    fn ai_round(&self, conn: &Connection, clip: &clip::Clip) {
        let cfg = self.config.lock().unwrap().ai_visual.clone();
        let settings = settings(&cfg);
        let locked = screen_locked(conn);
        for (id, connector) in self.connectors() {
            let Some(connector) = connector else { continue };
            // A lock screen or a dark panel would read as "Office": leave the mode alone.
            let paused = pause_reason(locked, || display_on(&connector), || self.ai_rule_applies(&connector), || self.ai_hdr(&id));
            if let Some(why) = paused {
                self.ai.lock().unwrap().monitors.entry(id).or_default().paused = Some(why.into());
                continue;
            }
            let obs = match capture::capture_output(conn, &connector).and_then(|c| {
                let fp = fingerprint(&c);
                let cached = self.ai.lock().unwrap().last_frame.get(&id).filter(|(f, _)| *f == fp).map(|(_, o)| o.clone());
                match cached {
                    Some(o) => Ok(o),
                    None => {
                        let o = clip.classify(&c.frame())?;
                        self.ai.lock().unwrap().last_frame.insert(id.clone(), (fp, o.clone()));
                        Ok(o)
                    }
                }
            }) {
                Ok(o) => o,
                Err(e) => {
                    self.ai.lock().unwrap().error = Some(format!("{e:#}"));
                    continue;
                }
            };
            let (verdict, send) = {
                let mut st = self.ai.lock().unwrap();
                st.error = None;
                st.engines.entry(id.clone()).or_default().observe(&obs, &settings)
            };
            // Send when the wanted mode differs from what Sumi last wrote here (by
            // AI Visual, App Sync or the app), like DisplayWidget's shared PreviousSetMode.
            let _ = send;
            let want = verdict.target.map(|m| (m, m.vcp_value(self.srgb_value(&id))));
            let last = self.last_mode.lock().unwrap().get(&id).copied();
            if let Some((mode, value)) = mode_to_send(want, last, self.read_only) {
                match self.apply(&id, "gamevisual", value) {
                    Ok(s) => {
                        info!("AI Visual: {id}: {s}");
                        if cfg.notify {
                            self.ai_notify(conn, &crate::i18n::tr1("Switched to %1 mode", &crate::i18n::tr(mode_label(mode))));
                        }
                    }
                    Err(e) => warn!("AI Visual: {id}: {e:#}"),
                }
            }
            let mut st = self.ai.lock().unwrap();
            let last = st.engines.get(&id).and_then(Engine::last_sent);
            let m = st.monitors.entry(id).or_default();
            m.verdict = Some(verdict);
            m.last_sent = last;
            m.paused = None;
        }
    }

    pub fn ai_status(&self) -> serde_json::Value {
        let st = self.ai.lock().unwrap();
        serde_json::json!({
            "installed": model::is_installed(),
            "prompts": clip::shipped_prompts().is_ok(),
            "model": model::VISION,
            "state": &*st,
        })
    }

    /// Fetch the model in the background (user-initiated: the Download button).
    pub fn ai_download(self: &Arc<Self>) -> anyhow::Result<()> {
        if self.read_only {
            anyhow::bail!("sumid is running read-only");
        }
        if self.ai.lock().unwrap().download.is_some() {
            anyhow::bail!("a download is already running");
        }
        self.ai.lock().unwrap().download = Some((0, model::VISION.size));
        let mgr = self.clone();
        std::thread::spawn(move || {
            info!("AI Visual: downloading {} ({} MB) from {}", model::VISION.name, model::VISION.size / 1_000_000, model::VISION.url);
            let r = model::download(&model::VISION, |done, total| mgr.ai.lock().unwrap().download = Some((done, total)));
            let mut st = mgr.ai.lock().unwrap();
            st.download = None;
            match r {
                Ok(p) => {
                    info!("AI Visual: model installed at {}", p.display());
                    st.error = None;
                }
                Err(e) => {
                    warn!("AI Visual: download failed: {e:#}");
                    st.error = Some(format!("download failed: {e:#}"));
                }
            }
        });
        Ok(())
    }
}

/// Start the AI Visual thread (idle until enabled and the model is present).
pub fn spawn(mgr: Arc<Manager>) {
    std::thread::Builder::new()
        .name("ai-visual".into())
        .spawn(move || {
            let mut clip: Option<clip::Clip> = None;
            let mut conn: Option<Connection> = None;
            loop {
                let cfg = mgr.config.lock().unwrap().ai_visual.clone();
                let interval = Duration::from_millis(cfg.interval_ms.clamp(250, 10_000));
                if !cfg.enabled || !model::is_installed() {
                    if clip.take().is_some() {
                        info!("AI Visual: stopped");
                    }
                    let mut st = mgr.ai.lock().unwrap();
                    st.running = false;
                    st.engines.clear();
                    drop(st);
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                if clip.is_none() {
                    let loaded = clip::shipped_prompts().and_then(|p| clip::Clip::load(&model::vision_path(), &p));
                    match loaded {
                        Ok(c) => {
                            info!("AI Visual: model loaded");
                            clip = Some(c);
                        }
                        Err(e) => {
                            mgr.ai.lock().unwrap().error = Some(format!("{e:#}"));
                            std::thread::sleep(Duration::from_secs(10));
                            continue;
                        }
                    }
                }
                if conn.is_none() {
                    conn = Connection::session().ok();
                }
                let started = Instant::now();
                if let (Some(c), Some(conn)) = (&clip, &conn) {
                    mgr.ai.lock().unwrap().running = true;
                    mgr.ai_round(conn, c);
                }
                std::thread::sleep(interval.saturating_sub(started.elapsed()).max(Duration::from_millis(100)));
            }
        })
        .ok();
}

impl Manager {
    /// Capture and classify a monitor once, without switching anything (diagnostics).
    pub fn ai_classify_now(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        use anyhow::Context as _;
        let connector = self.connectors().into_iter().find(|(k, _)| k == id).and_then(|(_, c)| c).context("monitor's output is unknown")?;
        let conn = Connection::session()?;
        let t0 = Instant::now();
        let cap = capture::capture_output(&conn, &connector)?;
        let t1 = Instant::now();
        let clip = {
            let mut cached = self.ai_clip.lock().unwrap();
            if cached.is_none() {
                *cached = Some(Arc::new(clip::Clip::load(&model::vision_path(), &clip::shipped_prompts()?)?));
            }
            cached.clone().expect("just loaded")
        };
        let t2 = Instant::now();
        let obs = clip.classify(&cap.frame())?;
        let t3 = Instant::now();
        let cfg = self.config.lock().unwrap().ai_visual.clone();
        let (verdict, _) = self.ai.lock().unwrap().preview.entry(id.to_string()).or_default().observe(&obs, &settings(&cfg));
        Ok(serde_json::json!({
            "observation": obs,
            "verdict": verdict,
            "frame": [cap.width, cap.height],
            "ms": {"capture": (t1 - t0).as_millis(), "load": (t2 - t1).as_millis(), "classify": (t3 - t2).as_millis()},
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use sumi_ai::preprocess::PixelFormat;
    use sumi_core::config::{AiVisual, AppRule};
    use sumi_core::testing::{self, Fake};

    /// Run `pause_reason`, counting how many of the lazy checks were asked.
    fn pause(locked: bool, display_on: bool, app_rule: bool, hdr: bool) -> (Option<&'static str>, [u32; 3]) {
        let asked = [Cell::new(0), Cell::new(0), Cell::new(0)];
        let ask = |i: usize, v: bool| {
            asked[i].set(asked[i].get() + 1);
            v
        };
        let r = pause_reason(locked, || ask(0, display_on), || ask(1, app_rule), || ask(2, hdr));
        (r, [asked[0].get(), asked[1].get(), asked[2].get()])
    }

    #[test]
    fn nothing_pauses_a_plain_unlocked_sdr_monitor() {
        assert_eq!(pause(false, true, false, false), (None, [1, 1, 1]));
    }

    #[test]
    fn each_condition_gives_its_own_reason() {
        assert_eq!(pause(true, true, false, false).0, Some(strings::AI_PAUSED_LOCKED));
        assert_eq!(pause(false, false, false, false).0, Some(strings::AI_PAUSED_DISPLAY_OFF));
        assert_eq!(pause(false, true, true, false).0, Some(strings::AI_PAUSED_APP_RULE));
        assert_eq!(pause(false, true, false, true).0, Some(strings::AI_PAUSED_HDR));
    }

    #[test]
    fn the_first_reason_in_priority_order_wins() {
        // Lock screen > display off > App Tweaker rule > HDR.
        assert_eq!(pause(true, false, true, true).0, Some(strings::AI_PAUSED_LOCKED));
        assert_eq!(pause(false, false, true, true).0, Some(strings::AI_PAUSED_DISPLAY_OFF));
        assert_eq!(pause(false, true, true, true).0, Some(strings::AI_PAUSED_APP_RULE));
    }

    #[test]
    fn later_checks_are_not_made_once_a_reason_is_found() {
        // The HDR check is a DDC read: it must not run for a locked or dark screen.
        assert_eq!(pause(true, true, true, true).1, [0, 0, 0]);
        assert_eq!(pause(false, false, true, true).1, [1, 0, 0]);
        assert_eq!(pause(false, true, true, true).1, [1, 1, 0]);
        assert_eq!(pause(false, true, false, true).1, [1, 1, 1]);
    }

    #[test]
    fn every_reason_has_a_translatable_string() {
        for (locked, on, rule, hdr) in [(true, true, false, false), (false, false, false, false), (false, true, true, false), (false, true, false, true)] {
            let why = pause(locked, on, rule, hdr).0.unwrap();
            assert!(strings::AI_PAUSE_REASONS.contains(&why), "{why}");
        }
    }

    const ALL_MODES: [Mode; 8] = [Mode::Cinema, Mode::Scenery, Mode::Srgb, Mode::User, Mode::Racing, Mode::RtsRpg, Mode::Fps, Mode::Moba];

    #[test]
    fn mode_names_match_the_translation_list() {
        let names: Vec<_> = ALL_MODES.iter().map(|&m| mode_label(m)).collect();
        for n in &names {
            assert!(strings::AI_MODE_NAMES.contains(n), "{n} is not in AI_MODE_NAMES");
        }
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), strings::AI_MODE_NAMES.len());
    }

    #[test]
    fn a_new_mode_is_sent_once_and_never_when_read_only() {
        let fps = Some((Mode::Fps, 7));
        assert_eq!(mode_to_send(fps, None, false), fps);
        assert_eq!(mode_to_send(fps, Some(5), false), fps);
        // Already the last mode Sumi wrote (by anyone): not again.
        assert_eq!(mode_to_send(fps, Some(7), false), None);
        assert_eq!(mode_to_send(fps, Some(5), true), None);
        assert_eq!(mode_to_send(None, Some(5), false), None);
    }

    #[test]
    fn srgb_is_sent_as_the_monitors_own_value() {
        assert_eq!(Mode::Srgb.vcp_value(0x0A), 0x0A);
        assert_eq!(Mode::Srgb.vcp_value(3), 3);
        assert_eq!(Mode::Fps.vcp_value(0x0A), 7);
        // A last mode of 3 (plain sRGB) is not the sRGB-calibrated 0x0A.
        let srgb = Some((Mode::Srgb, Mode::Srgb.vcp_value(0x0A)));
        assert_eq!(mode_to_send(srgb, Some(3), false), srgb);
    }

    #[test]
    fn the_settings_read_the_configured_mode_names() {
        let s = settings(&AiVisual::default());
        assert_eq!(s.enabled_modes.len(), AiVisual::default().modes.len(), "every default mode name should parse");
        assert_eq!(s.fallback, None);
        let cfg = AiVisual {
            modes: ["cinema", "user", "rts_rpg", "nonsense", "Fps"].map(String::from).to_vec(),
            fallback: Some("srgb".into()),
            ..AiVisual::default()
        };
        let s = settings(&cfg);
        assert_eq!(s.enabled_modes, [Mode::Cinema, Mode::User, Mode::RtsRpg]);
        assert_eq!(s.fallback, Some(Mode::Srgb));
        // A fallback that is not a mode is "remain unchanged".
        assert_eq!(settings(&AiVisual { fallback: Some("office".into()), ..AiVisual::default() }).fallback, None);
    }

    #[test]
    fn every_config_mode_name_is_a_mode() {
        for name in ["rts_rpg", "fps", "scenery", "racing", "srgb", "moba", "cinema", "user"] {
            let cfg = AiVisual { modes: vec![name.into()], ..AiVisual::default() };
            assert_eq!(settings(&cfg).enabled_modes.len(), 1, "{name}");
        }
    }

    // Manager-side checks

    fn rig() -> (Manager, Fake, String) {
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        let id = mgr.attach_test(testing::attach(&fake, Some("DP-3")));
        fake.clear_logs();
        (mgr, fake, id)
    }

    fn focus(mgr: &Manager, class: &str, output: &str) {
        mgr.on_window(class.into(), String::new(), 0, output.into());
    }

    #[test]
    fn an_app_tweaker_rule_covers_the_focused_window_on_its_output() {
        let (mgr, _fake, _) = rig();
        mgr.config.lock().unwrap().app_sync.rules = vec![AppRule { app: "game".into(), name: String::new(), mode: 5 }];
        assert!(!mgr.ai_rule_applies("DP-3"), "no window seen yet");
        focus(&mgr, "browser", "DP-3");
        assert!(!mgr.ai_rule_applies("DP-3"));
        focus(&mgr, "Game", "DP-3");
        assert!(mgr.ai_rule_applies("DP-3"));
        // Another output is not covered by that window.
        assert!(!mgr.ai_rule_applies("DP-1"));
        // Rules only count while App Sync is on.
        mgr.config.lock().unwrap().app_sync.enabled = false;
        assert!(!mgr.ai_rule_applies("DP-3"));
    }

    #[test]
    fn the_hdr_check_is_cached_for_ten_seconds() {
        let (mgr, fake, id) = rig();
        assert!(!mgr.ai_hdr(&id));
        fake.set_current(0xE2, 0x0102);
        assert!(!mgr.ai_hdr(&id), "still the cached answer");
        assert_eq!(fake.reads(), [0xE2]);
        // Once the cache is old, the monitor is asked again.
        mgr.ai.lock().unwrap().hdr_checked.insert(id.clone(), (Instant::now() - Duration::from_secs(11), false));
        assert!(mgr.ai_hdr(&id));
        assert_eq!(fake.reads(), [0xE2, 0xE2]);
        // An unreadable monitor is not in HDR.
        fake.fail_reads(0xE2);
        mgr.ai.lock().unwrap().hdr_checked.clear();
        assert!(!mgr.ai_hdr(&id));
    }

    // Frame fingerprint

    fn frame(w: usize, h: usize, fill: u8) -> capture::Capture {
        capture::Capture { data: vec![fill; w * h * 4], width: w, height: h, stride: w * 4, format: PixelFormat::Bgra }
    }

    #[test]
    fn identical_frames_share_a_fingerprint_and_a_changed_pixel_differs() {
        let (w, h) = (128, 72);
        let a = frame(w, h, 10);
        assert_eq!(fingerprint(&a), fingerprint(&frame(w, h, 10)));
        assert_ne!(fingerprint(&a), fingerprint(&frame(w, h, 11)));
        // The grid samples x = gx * w / 64 and y = gy * h / 36: a sampled pixel counts…
        let mut b = frame(w, h, 10);
        b.data[2 * 4] = 200;
        assert_ne!(fingerprint(&a), fingerprint(&b));
        // …and the alpha byte does not.
        let mut c = frame(w, h, 10);
        c.data[3] = 99;
        assert_eq!(fingerprint(&a), fingerprint(&c));
    }

    #[test]
    fn small_frames_fingerprint_without_panicking() {
        let _ = fingerprint(&frame(1, 1, 0));
        let _ = fingerprint(&frame(64, 36, 0));
    }
}
