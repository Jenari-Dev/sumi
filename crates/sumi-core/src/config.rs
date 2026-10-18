//! On-disk configuration: `~/.config/sumi/config.toml`.
//!
//! Monitor settings live in the monitor itself (as with DisplayWidget); this
//! file only holds host-side behaviour.
//!
//! ```toml
//! [app_sync]
//! enabled = true
//! default_mode = 7          # GameVisual for unlisted apps; omit = leave as is
//! restore_on_unfocus = false
//!
//! [[app_sync.rule]]
//! app = "steam_app_1172470" # window class, desktop-file id or executable name
//! name = "Apex Legends"
//! mode = 7                  # GameVisual (0xDC) value
//!
//! [hotkeys]
//! enabled = true
//! step = 5
//!
//! [target_mode]
//! strength = 44
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub app_sync: AppSync,
    pub hotkeys: Hotkeys,
    pub target_mode: TargetMode,
    pub gamepad: Gamepad,
    pub ai_visual: AiVisual,
    /// Mirror GameVisual-page changes to other connected monitors of the same
    /// model (DisplayWidget's ColorSync / Mode Sync).
    pub mode_sync: bool,
    /// Desktop state Sumi changed, so turning its toggles off restores it.
    #[serde(skip_serializing_if = "DesktopSaved::is_empty")]
    pub desktop_saved: DesktopSaved,
}

/// AI Visual: pick the GameVisual mode from what is on screen (DisplayWidget's
/// AI GameVisual). Off by default; needs the model download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiVisual {
    pub enabled: bool,
    /// Desktop notification when AI Visual changes the mode ("Show Notifications").
    pub notify: bool,
    /// Modes AI Visual may pick: "rts_rpg", "fps", "scenery", "racing",
    /// "srgb", "moba", "cinema", "user".
    pub modes: Vec<String>,
    /// "Assign my preference": mode for unknown / uncertain scenes; none =
    /// "Remain unchanged".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Milliseconds between frames (DisplayWidget: about one per second).
    pub interval_ms: u64,
}

impl Default for AiVisual {
    fn default() -> Self {
        AiVisual {
            enabled: false,
            notify: true,
            modes: ["rts_rpg", "fps", "scenery", "racing", "srgb", "moba"].map(String::from).to_vec(),
            fallback: None,
            interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Gamepad {
    /// Drive the monitor OSD from a game controller: hold LB+RB+LT+RT to
    /// start, D-pad to move, A to select, B/Back to go back.
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DesktopSaved {
    /// Plasma panel `hiding` values before auto-hide was switched on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub panel_hiding: Vec<String>,
    /// Colour scheme before the dark theme was switched on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<String>,
}

impl DesktopSaved {
    fn is_empty(&self) -> bool {
        self.panel_hiding.is_empty() && self.color_scheme.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSync {
    pub enabled: bool,
    /// GameVisual value applied when a non-listed app gets focus on the monitor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<u16>,
    /// Put back the mode that was active before a rule fired once a
    /// non-listed app is focused (DisplayWidget leaves the rule's mode on).
    pub restore_on_unfocus: bool,
    /// Turn KDE HDR off before applying a (SDR) GameVisual mode, as
    /// DisplayWidget does; otherwise rules are skipped while HDR is on.
    pub hdr_off_for_mode: bool,
    #[serde(rename = "rule", skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<AppRule>,
}

impl Default for AppSync {
    fn default() -> Self {
        // DisplayWidget ships App Sync switched on (with no rules).
        AppSync { enabled: true, default_mode: None, restore_on_unfocus: false, hdr_off_for_mode: true, rules: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppRule {
    /// Match key: window class (`resourceClass`), desktop-file id or executable
    /// name, compared case-insensitively.
    pub app: String,
    /// Display name for the UI.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// GameVisual (0xDC low byte) value.
    pub mode: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Hotkeys {
    /// Register Sumi's actions with KDE global shortcuts (keys are bound in
    /// System Settings › Shortcuts; all unbound by default).
    pub enabled: bool,
    /// Brightness / contrast / saturation step per key press (DisplayWidget: 5).
    pub step: u16,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys { enabled: true, step: 5 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TargetMode {
    /// Dimming of inactive windows in percent (DisplayWidget: ~44 % black).
    pub strength: u8,
}

impl Default for TargetMode {
    fn default() -> Self {
        TargetMode { strength: 44 }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("sumi").join("config.toml")
    }

    /// Load, or defaults when the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("parsing {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Write atomically (temp file + rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The rule matching any of a window's identifiers.
    pub fn rule_for<'a>(&'a self, keys: &[&str]) -> Option<&'a AppRule> {
        self.app_sync.rules.iter().find(|r| {
            let want = r.app.trim().to_ascii_lowercase();
            !want.is_empty() && keys.iter().any(|k| !k.is_empty() && k.to_ascii_lowercase() == want)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_match() {
        let mut c = Config::default();
        c.app_sync.default_mode = Some(2);
        c.app_sync.rules.push(AppRule { app: "steam_app_1172470".into(), name: "Game".into(), mode: 7 });
        let s = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.rule_for(&["", "STEAM_APP_1172470"]).map(|r| r.mode), Some(7));
        assert!(back.rule_for(&["steam_app_1"]).is_none());
    }

    #[test]
    fn empty_file_is_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert!(c.app_sync.enabled && c.hotkeys.enabled && c.hotkeys.step == 5);
    }
}
