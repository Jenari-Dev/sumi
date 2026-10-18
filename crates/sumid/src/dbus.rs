//! D-Bus API on the session bus.
//!
//! Bus name `io.github.JenAriDev.Sumi`, object `/io/github/JenAriDev/Sumi`,
//! interface `io.github.JenAriDev.Sumi1`. Structured data is passed as JSON.

use std::sync::Arc;

use sumi_core::features::{self, Page};
use zbus::{fdo, interface, object_server::SignalEmitter};

use crate::manager::Manager;

pub const BUS_NAME: &str = "io.github.JenAriDev.Sumi";
pub const PATH: &str = "/io/github/JenAriDev/Sumi";

pub struct Sumi {
    pub mgr: Arc<Manager>,
}

/// Errors callers may want to tell apart (name `io.github.JenAriDev.Sumi1.Error.<Variant>`).
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.JenAriDev.Sumi1.Error")]
pub enum SumiError {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// The settings file is for another GameVisual mode; retry with switch_mode.
    ModeMismatch(String),
    Failed(String),
}

fn err(e: anyhow::Error) -> fdo::Error {
    fdo::Error::Failed(format!("{e:#}"))
}

/// Run blocking DDC work off the async executor.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> fdo::Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| fdo::Error::Failed(e.to_string()))?.map_err(err)
}

fn parse_pages(pages: &[String]) -> fdo::Result<Option<Vec<Page>>> {
    if pages.is_empty() {
        return Ok(None);
    }
    pages
        .iter()
        .map(|p| serde_json::from_value(serde_json::Value::String(p.clone())).map_err(|_| fdo::Error::InvalidArgs(format!("unknown page {p:?}"))))
        .collect::<fdo::Result<Vec<Page>>>()
        .map(Some)
}

#[interface(name = "io.github.JenAriDev.Sumi1")]
impl Sumi {
    /// Attached ASUS monitors as a JSON array of {id, model, line, oled, bus, fw}.
    async fn list_monitors(&self) -> String {
        serde_json::to_string(&self.mgr.list()).unwrap_or_default()
    }

    /// ASUS monitors found by EDID whose DDC/CI does not answer, as JSON
    /// [{id, error}] (DDC/CI off in the OSD, no i2c access, monitor asleep…).
    async fn problems(&self) -> String {
        let p = self.mgr.problems.lock().unwrap();
        let v: Vec<_> = p.iter().map(|(id, e)| serde_json::json!({ "id": id, "error": e })).collect();
        serde_json::to_string(&v).unwrap_or_default()
    }

    /// Decoded feature state as JSON. `pages` limits the read to some pages
    /// ("game_visual", "game_plus", "oled_care", "multi_screen", "system",
    /// "device_info"); empty = everything.
    async fn get_state(&self, id: String, pages: Vec<String>) -> fdo::Result<String> {
        let pages = parse_pages(&pages)?;
        let mgr = self.mgr.clone();
        let st = blocking(move || mgr.state(&id, pages)).await?;
        Ok(serde_json::to_string(&st).unwrap_or_default())
    }

    /// Change one feature (verified by read-back). Returns a description.
    async fn apply(&self, id: String, feature: String, value: u16) -> fdo::Result<String> {
        if features::by_id(&feature).is_none() {
            return Err(fdo::Error::InvalidArgs(format!("unknown feature {feature:?}")));
        }
        let mgr = self.mgr.clone();
        blocking(move || mgr.apply(&id, &feature, value)).await
    }

    /// Raw VCP read for diagnostics: (current, max).
    async fn get_vcp(&self, id: String, code: u8) -> fdo::Result<(u16, u16)> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.raw_get(&id, code)).await
    }

    /// Parsed capability string as JSON.
    async fn capabilities(&self, id: String) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.capabilities(&id)).await
    }

    /// The static feature catalogue as JSON.
    async fn catalogue(&self) -> String {
        serde_json::to_string(features::FEATURES).unwrap_or_default()
    }

    /// Called by Sumi's KWin script when the active window or its output changes.
    async fn window_activated(&self, class: String, desktop_file: String, pid: i32, output: String) {
        let mgr = self.mgr.clone();
        tokio::task::spawn_blocking(move || mgr.on_window(class, desktop_file, pid, output));
    }

    /// Host-side configuration (App Sync rules, hot keys, Target Mode) as JSON.
    async fn get_config(&self) -> String {
        serde_json::to_string(&*self.mgr.config.lock().unwrap()).unwrap_or_default()
    }

    /// Replace and save the configuration (JSON with the `GetConfig` shape).
    async fn set_config(&self, json: String) -> fdo::Result<()> {
        let cfg: sumi_core::Config = serde_json::from_str(&json).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))?;
        let mgr = self.mgr.clone();
        blocking(move || mgr.save_config(cfg)).await
    }

    /// Installed applications (XDG desktop entries) as JSON [{id, name, icon, key}].
    async fn list_apps(&self) -> fdo::Result<String> {
        blocking(|| Ok(serde_json::to_string(&sumi_core::apps::installed())?)).await
    }

    /// Recently focused windows as JSON [{class, desktop_file, exe, output}], newest first.
    async fn recent_windows(&self) -> String {
        serde_json::to_string(&self.mgr.appsync.lock().unwrap().recent).unwrap_or_default()
    }

    /// Hot-key actions and their current KDE bindings as JSON [{action, label, keys}].
    async fn hotkeys(&self) -> fdo::Result<String> {
        blocking(|| {
            let conn = zbus::blocking::Connection::session()?;
            Ok(serde_json::to_string(&crate::shortcuts::bindings(&conn)?)?)
        })
        .await
    }

    /// Run a hot-key action by id (as if its key was pressed).
    async fn run_action(&self, action: String) -> fdo::Result<()> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.hotkey(&action)).await
    }

    /// Host-side state of a monitor as JSON {hdr, target_mode, connector}.
    async fn get_host_state(&self, id: String) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || {
            let connector = mgr.connectors().into_iter().find(|(k, _)| *k == id).and_then(|(_, c)| c);
            Ok(serde_json::json!({
                "connector": connector,
                "hdr": mgr.host_hdr(&id).ok(),
                "target_mode": mgr.target_mode().ok(),
            })
            .to_string())
        })
        .await
    }

    /// KDE HDR on/off for the monitor's output.
    async fn set_hdr(&self, id: String, on: bool) -> fdo::Result<()> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.set_host_hdr(&id, on)).await
    }

    /// Target Mode (KWin Dim Inactive) on/off.
    async fn set_target_mode(&self, on: bool) -> fdo::Result<()> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.set_target_mode(on)).await
    }

    /// Current GameVisual mode's settings in DisplayWidget's export format.
    async fn export_settings(&self, id: String) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(serde_json::to_string_pretty(&mgr.export_profile(&id)?)?)).await
    }

    /// Import a DisplayWidget/Sumi settings file; returns the per-code log.
    async fn import_settings(&self, id: String, json: String, switch_mode: bool) -> Result<String, SumiError> {
        let doc: serde_json::Value = serde_json::from_str(&json).map_err(|e| SumiError::Failed(format!("not a settings file: {e}")))?;
        let mgr = self.mgr.clone();
        let r = tokio::task::spawn_blocking(move || mgr.import_profile(&id, &doc, switch_mode))
            .await
            .map_err(|e| SumiError::Failed(e.to_string()))?;
        match r {
            Ok(log) => Ok(log.join("\n")),
            Err(e) => {
                let msg = format!("{e:#}");
                Err(if msg.contains("switch first") { SumiError::ModeMismatch(msg) } else { SumiError::Failed(msg) })
            }
        }
    }

    /// MultiFrame layouts: DisplayWidget's numbering and names, JSON [[n, name]].
    async fn tile_layout_names(&self) -> String {
        serde_json::to_string(sumi_core::multiframe::LAYOUTS).unwrap_or_default()
    }

    /// Every output's current KWin tile layout as JSON [{output, aspect, layout, tiles}].
    async fn get_tile_layouts(&self) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(serde_json::to_string(&mgr.tile_layouts()?)?)).await
    }

    /// Put MultiFrame layout `layout` (1–13, 0 = none) on a KWin output ("DP-3").
    async fn set_tile_layout(&self, output: String, layout: u8) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(serde_json::to_string(&mgr.set_tile_layout(&output, layout)?)?)).await
    }

    /// Called back by Sumi's KWin tiling script.
    async fn tile_layout_report(&self, json: String) {
        self.mgr.on_tile_report(json);
    }

    /// Plasma desktop toggles as JSON {panel_autohide: bool?, dark_theme: bool?}.
    async fn get_desktop_prefs(&self) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(serde_json::to_string(&mgr.desktop_prefs())?)).await
    }

    /// Auto-hide every Plasma panel (on), or restore what they were (off).
    async fn set_panel_autohide(&self, on: bool) -> fdo::Result<()> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.set_panel_autohide(on)).await
    }

    /// Breeze Dark (on), or back to the previous colour scheme (off).
    async fn set_dark_theme(&self, on: bool) -> fdo::Result<()> {
        let mgr = self.mgr.clone();
        blocking(move || mgr.set_dark_theme(on)).await
    }

    /// Connected game controllers as a JSON array of names.
    async fn gamepads(&self) -> String {
        let names: Vec<String> = crate::gamepad::gamepads().into_iter().map(|(_, n)| n).collect();
        serde_json::to_string(&names).unwrap_or_default()
    }

    /// AI Visual status as JSON {installed, prompts, model{name,url,size,sha256},
    /// state{running, error, download:[done,total]?, monitors{id:{verdict, last_sent, paused}}}}.
    async fn ai_status(&self) -> String {
        self.mgr.ai_status().to_string()
    }

    /// Download the AI Visual model (≈352 MB from Hugging Face, SHA-256 checked).
    /// Progress shows in `AiStatus().state.download`.
    async fn download_ai_model(&self) -> fdo::Result<()> {
        self.mgr.ai_download().map_err(err)
    }

    /// Capture and classify a monitor once; switches nothing. Returns JSON
    /// {observation (raw), verdict (smoothed over previous calls, with the mode
    /// AI Visual would pick), frame, ms} — the panel's preview while AI is off.
    async fn ai_classify_now(&self, id: String) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(mgr.ai_classify_now(&id)?.to_string())).await
    }

    /// Ask ASUS's download server whether newer firmware exists (DisplayWidget's check: HEAD
    /// requests for the ten versions after the current one). Only when called; returns JSON
    /// {current: "MCM103", latest: "MCM105"|null, url|null, checked_at, tried: [[url, status]…]}.
    /// Writes nothing to the monitor, so it also works with --read-only.
    async fn check_firmware(&self, id: String) -> fdo::Result<String> {
        let mgr = self.mgr.clone();
        blocking(move || Ok(mgr.firmware_check_with(&id, sumi_core::firmware::http_head)?.to_string())).await
    }

    /// Download the zip the last CheckFirmware found into the user's Download folder (never
    /// overwriting). Progress and the saved path show in `FirmwareStatus().download`.
    async fn download_firmware(&self, id: String) -> fdo::Result<()> {
        self.mgr
            .firmware_download_with(&id, sumi_core::firmware::download_dir(), |url, dir, p| sumi_core::firmware::download(url, dir, p))
            .map_err(err)
    }

    /// JSON {checks: {id: <CheckFirmware reply>}, download: {state: idle|running|done|error, id, done, total, path?, error?}}.
    async fn firmware_status(&self) -> String {
        self.mgr.firmware_status().to_string()
    }

    /// Delete the downloaded AI Visual model.
    async fn remove_ai_model(&self) -> fdo::Result<()> {
        if self.mgr.read_only {
            return Err(fdo::Error::Failed("sumid is running read-only".into()));
        }
        blocking(sumi_ai::model::remove).await
    }

    /// Daemon version.
    #[zbus(property)]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[zbus(property)]
    async fn read_only(&self) -> bool {
        self.mgr.read_only
    }

    #[zbus(signal)]
    pub async fn monitor_added(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn monitor_removed(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;

    /// Something changed on the monitor side (OSD, HDR signal, power saving).
    /// `context` is JSON {context, brightness}; re-read state for details.
    #[zbus(signal)]
    pub async fn state_changed(emitter: &SignalEmitter<'_>, id: &str, context: &str) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(names: &[&str]) -> fdo::Result<Option<Vec<Page>>> {
        parse_pages(&names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn no_pages_means_all_of_them() {
        assert_eq!(pages(&[]).unwrap(), None);
    }

    #[test]
    fn pages_are_named_in_snake_case() {
        assert_eq!(pages(&["game_visual", "oled_care"]).unwrap(), Some(vec![Page::GameVisual, Page::OledCare]));
        assert_eq!(pages(&["device_info"]).unwrap(), Some(vec![Page::DeviceInfo]));
    }

    #[test]
    fn an_unknown_page_is_an_invalid_argument() {
        for bad in [&["nope"][..], &["game_visual", "GameVisual"], &[""]] {
            let e = pages(bad).unwrap_err();
            assert!(matches!(e, fdo::Error::InvalidArgs(_)), "{bad:?}: {e}");
        }
    }
}

