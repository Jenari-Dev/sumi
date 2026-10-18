//! Every user-visible string the daemon supplies, for translation catalogues
//! (`po/extract.sh` turns this list into `po/daemon-strings.cpp`).

use crate::{features, hotkeys, multiframe};

/// Notification texts sumid shows (msgids; `%1` is a GameVisual mode name).
pub const NOTIFICATIONS: &[&str] = &["AI Visual", "Switched to %1 mode"];

/// GameVisual mode names AI Visual reports in notifications.
pub const AI_MODE_NAMES: &[&str] = &["Cinema", "Scenery", "sRGB", "User", "Racing", "RTS/RPG", "FPS", "MOBA"];

/// Why AI Visual is paused on a monitor (shown as "Paused: <reason>").
pub const AI_PAUSED_LOCKED: &str = "screen locked";
pub const AI_PAUSED_DISPLAY_OFF: &str = "display off";
pub const AI_PAUSED_APP_RULE: &str = "an App Tweaker rule covers the focused app";
pub const AI_PAUSED_HDR: &str = "HDR is on";
pub const AI_PAUSE_REASONS: &[&str] = &[AI_PAUSED_LOCKED, AI_PAUSED_DISPLAY_OFF, AI_PAUSED_APP_RULE, AI_PAUSED_HDR];

/// All daemon display strings, deduplicated, in a stable order.
pub fn all() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    let mut push = |s: &'static str| {
        if !s.is_empty() && !v.contains(&s) {
            v.push(s);
        }
    };
    for f in features::FEATURES {
        push(f.label);
        for o in f.options {
            push(o.label);
        }
    }
    for (_, label) in hotkeys::ACTIONS {
        push(label);
    }
    for (_, name) in multiframe::LAYOUTS {
        push(name);
    }
    for s in NOTIFICATIONS.iter().chain(AI_MODE_NAMES).chain(AI_PAUSE_REASONS) {
        push(s);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_has_no_duplicates_or_blanks() {
        let all = all();
        let mut sorted = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len());
        assert!(all.iter().all(|s| !s.is_empty()));
    }

    #[test]
    fn daemon_strings_are_included() {
        let all = all();
        for s in NOTIFICATIONS.iter().chain(AI_MODE_NAMES).chain(AI_PAUSE_REASONS) {
            assert!(all.contains(s), "{s:?}");
        }
        // Feature labels and their options too.
        assert!(all.contains(&"Brightness") && all.contains(&"Gaming HDR") && all.contains(&"Pixel Cleaning"));
    }

    #[test]
    fn the_pause_reasons_are_the_four_the_runner_gives() {
        assert_eq!(AI_PAUSE_REASONS, [AI_PAUSED_LOCKED, AI_PAUSED_DISPLAY_OFF, AI_PAUSED_APP_RULE, AI_PAUSED_HDR]);
    }
}

