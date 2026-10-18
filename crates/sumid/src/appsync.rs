//! App Sync ("App Tweaker"): switch GameVisual when a listed app gets focus.
//!
//! Vendor semantics: a rule's mode is written when its app is focused on the
//! monitor; a mode is not re-sent if it was the last one applied there (so a
//! manual OSD change sticks until another rule fires); unlisted apps get the
//! default mode, if one is set. Extra: `restore_on_unfocus` puts back the mode
//! that was active before the first rule fired.

use std::collections::{HashMap, VecDeque};

use serde::Serialize;
use sumi_core::config::{AppRule, Config};
use tracing::{info, warn};

use crate::manager::Manager;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WindowInfo {
    pub class: String,
    pub desktop_file: String,
    pub exe: String,
    pub output: String,
}

impl WindowInfo {
    /// Identifiers rules are matched against.
    pub fn keys(&self) -> Vec<&str> {
        let desktop = self.desktop_file.trim_end_matches(".desktop");
        vec![self.class.as_str(), desktop, self.exe.as_str()]
    }
}

#[derive(Default)]
pub struct AppSyncState {
    /// Last mode App Sync wrote, per monitor id.
    last_applied: HashMap<String, u16>,
    /// Mode before a rule took over (for restore_on_unfocus), per monitor id.
    before_rule: HashMap<String, u16>,
    pub recent: VecDeque<WindowInfo>,
    pub active_output: Option<String>,
}

/// What App Sync wants on one monitor after a focus change: (mode, from a rule).
/// `before_rule` is the mode saved when a rule first took over there.
fn sync_target(cfg: &Config, rule: Option<&AppRule>, focused_here: bool, before_rule: Option<u16>) -> Option<(u16, bool)> {
    match (rule, focused_here) {
        (Some(r), true) => Some((r.mode, true)),
        // With AI Visual on, it handles unlisted apps (DisplayWidget hides the default row).
        _ if cfg.ai_visual.enabled => None,
        _ => cfg
            .app_sync
            .default_mode
            .map(|m| (m, false))
            .or_else(|| cfg.app_sync.restore_on_unfocus.then_some(before_rule).flatten().map(|m| (m, false))),
    }
}

fn exe_name(pid: i32) -> String {
    if pid <= 0 {
        return String::new();
    }
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().trim_end_matches(" (deleted)").to_string()))
        .unwrap_or_default()
}

impl Manager {
    pub fn on_window(&self, class: String, desktop_file: String, pid: i32, output: String) {
        let w = WindowInfo { class, desktop_file, exe: exe_name(pid), output };
        {
            let mut st = self.appsync.lock().unwrap();
            st.active_output = Some(w.output.clone()).filter(|o| !o.is_empty());
            if !w.class.is_empty() || !w.exe.is_empty() {
                st.recent.retain(|r| (r.class.as_str(), r.desktop_file.as_str()) != (w.class.as_str(), w.desktop_file.as_str()));
                st.recent.push_front(w.clone());
                st.recent.truncate(20);
            }
        }
        let cfg = self.config.lock().unwrap().clone();
        if !cfg.app_sync.enabled || self.read_only {
            return;
        }
        let rule = cfg.rule_for(&w.keys()).cloned();
        for (id, connector) in self.connectors() {
            let focused_here = connector.as_deref() == Some(w.output.as_str());
            let before_rule = self.appsync.lock().unwrap().before_rule.get(&id).copied();
            let Some((mode, from_rule)) = sync_target(&cfg, rule.as_ref(), focused_here, before_rule) else { continue };
            self.app_sync_apply(&id, mode, from_rule, &w);
        }
    }

    fn app_sync_apply(&self, id: &str, mode: u16, from_rule: bool, w: &WindowInfo) {
        let first_rule = {
            let st = self.appsync.lock().unwrap();
            if st.last_applied.get(id) == Some(&mode) {
                return;
            }
            from_rule && !st.before_rule.contains_key(id)
        };
        if first_rule {
            if let Ok((cur, _)) = self.raw_get(id, 0xDC) {
                self.appsync.lock().unwrap().before_rule.insert(id.to_string(), cur & 0xFF);
            }
        }
        let hdr_ok = self.config.lock().unwrap().app_sync.hdr_off_for_mode;
        if let Err(e) = self.leave_hdr_for_mode(id, hdr_ok) {
            info!("App Sync: {id}: not switching to mode {mode}: {e:#}");
            return;
        }
        match self.apply(id, "gamevisual", mode) {
            Ok(s) => {
                info!("App Sync ({}): {id}: {s}", if w.class.is_empty() { &w.exe } else { &w.class });
                let mut st = self.appsync.lock().unwrap();
                st.last_applied.insert(id.to_string(), mode);
                // Only a restore that landed ends the rule; a refused one is retried at the next unlisted focus.
                if !from_rule {
                    st.before_rule.remove(id);
                }
            }
            Err(e) => warn!("App Sync: {id}: mode {mode}: {e:#}"),
        }
    }

    /// Forget what App Sync applied (after config changes or a manual switch on).
    pub fn reset_app_sync(&self) {
        let mut st = self.appsync.lock().unwrap();
        st.last_applied.clear();
        st.before_rule.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sumi_core::testing::{self, Fake};

    fn win(class: &str, desktop: &str, exe: &str) -> WindowInfo {
        WindowInfo { class: class.into(), desktop_file: desktop.into(), exe: exe.into(), output: "DP-3".into() }
    }

    fn rule(app: &str, mode: u16) -> AppRule {
        AppRule { app: app.into(), name: String::new(), mode }
    }

    fn cfg_with(rules: Vec<AppRule>) -> Config {
        let mut c = Config::default();
        c.app_sync.rules = rules;
        c
    }

    // Rule matching

    #[test]
    fn a_windows_keys_are_class_desktop_id_and_executable() {
        assert_eq!(win("Steam_App_1", "steam_app_1.desktop", "game").keys(), ["Steam_App_1", "steam_app_1", "game"]);
        // No desktop file: an empty key, which never matches anything.
        assert_eq!(win("x", "", "").keys(), ["x", "", ""]);
    }

    #[test]
    fn rules_match_any_key_without_regard_to_case() {
        let cfg = cfg_with(vec![rule("steam_app_1172470", 7), rule("org.kde.dolphin", 2), rule("GameBin", 5)]);
        let mode = |w: WindowInfo| cfg.rule_for(&w.keys()).map(|r| r.mode);
        assert_eq!(mode(win("Steam_App_1172470", "", "")), Some(7));
        assert_eq!(mode(win("dolphin", "org.kde.dolphin.desktop", "")), Some(2));
        assert_eq!(mode(win("", "", "gamebin")), Some(5));
        assert_eq!(mode(win("other", "other.desktop", "other")), None);
        // A partial name is not a match.
        assert_eq!(mode(win("steam_app_11", "", "")), None);
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let cfg = cfg_with(vec![rule("app", 1), rule("APP", 2), rule("  app ", 3)]);
        assert_eq!(cfg.rule_for(&["app"]).map(|r| r.mode), Some(1));
        // Blanks around a rule's key are ignored.
        let cfg = cfg_with(vec![rule("  app ", 3)]);
        assert_eq!(cfg.rule_for(&["app"]).map(|r| r.mode), Some(3));
    }

    #[test]
    fn blank_rules_and_blank_windows_never_match() {
        let cfg = cfg_with(vec![rule("", 1), rule("   ", 2)]);
        assert!(cfg.rule_for(&["", "", ""]).is_none());
        assert!(cfg.rule_for(&["app"]).is_none());
    }

    // What App Sync wants

    #[test]
    fn a_rule_on_the_focused_monitor_wins() {
        let mut cfg = Config::default();
        cfg.app_sync.default_mode = Some(2);
        let r = rule("app", 7);
        assert_eq!(sync_target(&cfg, Some(&r), true, Some(4)), Some((7, true)));
    }

    #[test]
    fn unlisted_apps_get_the_default_mode() {
        let mut cfg = Config::default();
        assert_eq!(sync_target(&cfg, None, true, None), None);
        cfg.app_sync.default_mode = Some(2);
        assert_eq!(sync_target(&cfg, None, true, None), Some((2, false)));
        // A rule for another monitor's window is no rule here.
        let r = rule("app", 7);
        assert_eq!(sync_target(&cfg, Some(&r), false, None), Some((2, false)));
    }

    #[test]
    fn restore_on_unfocus_puts_back_the_saved_mode() {
        let mut cfg = Config::default();
        cfg.app_sync.restore_on_unfocus = true;
        assert_eq!(sync_target(&cfg, None, true, Some(4)), Some((4, false)));
        // Nothing saved, nothing to restore.
        assert_eq!(sync_target(&cfg, None, true, None), None);
        // The default mode comes first.
        cfg.app_sync.default_mode = Some(2);
        assert_eq!(sync_target(&cfg, None, true, Some(4)), Some((2, false)));
        // Off: the saved mode is left alone.
        cfg.app_sync.default_mode = None;
        cfg.app_sync.restore_on_unfocus = false;
        assert_eq!(sync_target(&cfg, None, true, Some(4)), None);
    }

    #[test]
    fn ai_visual_handles_unlisted_apps_but_not_rules() {
        let mut cfg = Config::default();
        cfg.app_sync.default_mode = Some(2);
        cfg.app_sync.restore_on_unfocus = true;
        cfg.ai_visual.enabled = true;
        assert_eq!(sync_target(&cfg, None, true, Some(4)), None);
        let r = rule("app", 7);
        assert_eq!(sync_target(&cfg, Some(&r), true, Some(4)), Some((7, true)));
        assert_eq!(sync_target(&cfg, Some(&r), false, Some(4)), None);
    }

    // Driving the manager with focus changes (a fake monitor on DP-3)

    const GAME: u16 = 5;

    fn rig() -> (Manager, Fake, String) {
        let mgr = Manager::for_test(false);
        let fake = Fake::pg32ucdm3();
        // HDR off: switching modes then never asks KDE about HDR.
        assert_eq!(fake.current(0xE2), 0);
        let id = mgr.attach_test(testing::attach(&fake, Some("DP-3")));
        fake.clear_logs();
        mgr.config.lock().unwrap().app_sync.rules = vec![rule("game", GAME), rule("movie", 1)];
        (mgr, fake, id)
    }

    fn focus(mgr: &Manager, class: &str, output: &str) {
        mgr.on_window(class.into(), String::new(), 0, output.into());
    }

    fn before_rule(mgr: &Manager, id: &str) -> Option<u16> {
        mgr.appsync.lock().unwrap().before_rule.get(id).copied()
    }

    #[test]
    fn a_rule_sets_its_mode_on_the_monitor_the_window_is_on() {
        let (mgr, fake, id) = rig();
        focus(&mgr, "Game", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, GAME)]);
        assert_eq!(mgr.last_mode.lock().unwrap().get(&id), Some(&GAME));
        // A window on another output is not this monitor's business.
        fake.clear_logs();
        focus(&mgr, "Movie", "DP-1");
        assert!(fake.writes().is_empty());
        // The focus is remembered either way.
        let st = mgr.appsync.lock().unwrap();
        assert_eq!(st.active_output.as_deref(), Some("DP-1"));
        assert_eq!(st.recent.front().map(|w| w.class.as_str()), Some("Movie"));
    }

    #[test]
    fn a_mode_is_not_sent_again_so_an_osd_change_sticks() {
        let (mgr, fake, _) = rig();
        focus(&mgr, "Game", "DP-3");
        fake.clear_logs();
        // The user picks another mode in the monitor's own menu.
        fake.set_current(0xDC, 7);
        focus(&mgr, "Game", "DP-3");
        focus(&mgr, "game", "DP-3");
        assert!(fake.writes().is_empty());
        assert_eq!(fake.current(0xDC), 7);
        // A different rule fires, and then the first one is back in force.
        focus(&mgr, "Movie", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, 1)]);
        focus(&mgr, "Game", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, 1), (0xDC, GAME)]);
    }

    #[test]
    fn unlisted_apps_get_the_default_mode_once() {
        let (mgr, fake, _) = rig();
        mgr.config.lock().unwrap().app_sync.default_mode = Some(2);
        focus(&mgr, "browser", "DP-3");
        focus(&mgr, "editor", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, 2)]);
        focus(&mgr, "game", "DP-3");
        focus(&mgr, "editor", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, 2), (0xDC, GAME), (0xDC, 2)]);
    }

    #[test]
    fn without_a_default_or_restore_unlisted_apps_change_nothing() {
        let (mgr, fake, _) = rig();
        focus(&mgr, "browser", "DP-3");
        focus(&mgr, "game", "DP-3");
        focus(&mgr, "browser", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, GAME)]);
    }

    #[test]
    fn ai_visual_leaves_unlisted_apps_to_itself() {
        let (mgr, fake, _) = rig();
        {
            let mut cfg = mgr.config.lock().unwrap();
            cfg.app_sync.default_mode = Some(2);
            cfg.ai_visual.enabled = true;
        }
        focus(&mgr, "browser", "DP-3");
        assert!(fake.writes().is_empty());
        focus(&mgr, "game", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, GAME)]);
    }

    #[test]
    fn restore_on_unfocus_returns_to_the_mode_from_before_the_rule() {
        let (mgr, fake, id) = rig();
        mgr.config.lock().unwrap().app_sync.restore_on_unfocus = true;
        // Nothing to restore yet.
        focus(&mgr, "browser", "DP-3");
        assert!(fake.writes().is_empty());
        fake.set_current(0xDC, 0x0107); // FPS (the low byte is the mode)
        focus(&mgr, "game", "DP-3");
        assert_eq!(before_rule(&mgr, &id), Some(7));
        focus(&mgr, "browser", "DP-3");
        // Only the low byte is the mode: the high byte stays as the monitor had it.
        assert_eq!(fake.writes(), [(0xDC, 0x0100 | GAME), (0xDC, 0x0107)]);
        assert_eq!(before_rule(&mgr, &id), None);
        // Done: more unlisted windows do not write again.
        focus(&mgr, "editor", "DP-3");
        assert_eq!(fake.writes().len(), 2);
        // And the next rule saves the then-current mode afresh.
        fake.set_current(0xDC, 3);
        focus(&mgr, "movie", "DP-3");
        assert_eq!(before_rule(&mgr, &id), Some(3));
    }

    #[test]
    fn the_first_rule_keeps_the_saved_mode_across_rules() {
        let (mgr, fake, id) = rig();
        mgr.config.lock().unwrap().app_sync.restore_on_unfocus = true;
        focus(&mgr, "game", "DP-3");
        focus(&mgr, "movie", "DP-3");
        // The second rule must not save the first rule's mode as "before".
        assert_eq!(before_rule(&mgr, &id), Some(7));
        focus(&mgr, "browser", "DP-3");
        assert_eq!(fake.current(0xDC), 7);
    }

    #[test]
    fn the_default_mode_beats_restore_and_forgets_the_saved_mode() {
        let (mgr, fake, id) = rig();
        {
            let mut cfg = mgr.config.lock().unwrap();
            cfg.app_sync.restore_on_unfocus = true;
            cfg.app_sync.default_mode = Some(2);
        }
        focus(&mgr, "game", "DP-3");
        assert_eq!(before_rule(&mgr, &id), Some(7));
        focus(&mgr, "browser", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, GAME), (0xDC, 2)]);
        assert_eq!(before_rule(&mgr, &id), None);
    }

    #[test]
    fn a_restore_the_monitor_did_not_take_is_tried_again() {
        let (mgr, fake, id) = rig();
        mgr.config.lock().unwrap().app_sync.restore_on_unfocus = true;
        focus(&mgr, "game", "DP-3");
        assert_eq!(before_rule(&mgr, &id), Some(7));
        // The restore is lost: the saved mode must survive for the next try.
        fake.ignore_writes(0xDC);
        focus(&mgr, "browser", "DP-3");
        assert_eq!(fake.current(0xDC), GAME);
        assert_eq!(before_rule(&mgr, &id), Some(7));
        fake.accept_writes(0xDC);
        focus(&mgr, "editor", "DP-3");
        assert_eq!(fake.current(0xDC), 7);
        assert_eq!(before_rule(&mgr, &id), None);
    }

    #[test]
    fn a_failed_rule_write_is_tried_again_at_the_next_focus() {
        let (mgr, fake, _) = rig();
        fake.ignore_writes(0xDC);
        focus(&mgr, "game", "DP-3");
        assert_eq!(fake.current(0xDC), 7);
        fake.accept_writes(0xDC);
        focus(&mgr, "game", "DP-3");
        assert_eq!(fake.current(0xDC), GAME);
    }

    #[test]
    fn app_sync_off_or_a_read_only_daemon_writes_nothing_but_still_watches_focus() {
        let (mgr, fake, _) = rig();
        mgr.config.lock().unwrap().app_sync.enabled = false;
        focus(&mgr, "game", "DP-3");
        assert!(fake.writes().is_empty());
        assert_eq!(mgr.appsync.lock().unwrap().recent.len(), 1);

        let ro = Manager::for_test(true);
        let fake = Fake::pg32ucdm3();
        ro.attach_test(testing::attach(&fake, Some("DP-3")));
        ro.config.lock().unwrap().app_sync.rules = vec![rule("game", GAME)];
        fake.clear_logs();
        focus(&ro, "game", "DP-3");
        assert!(fake.writes().is_empty() && fake.reads().is_empty());
        assert_eq!(ro.appsync.lock().unwrap().active_output.as_deref(), Some("DP-3"));
    }

    #[test]
    fn resetting_makes_the_next_focus_send_again() {
        let (mgr, fake, id) = rig();
        mgr.config.lock().unwrap().app_sync.restore_on_unfocus = true;
        focus(&mgr, "game", "DP-3");
        fake.clear_logs();
        mgr.reset_app_sync();
        assert_eq!(before_rule(&mgr, &id), None);
        focus(&mgr, "game", "DP-3");
        assert_eq!(fake.writes(), [(0xDC, GAME)]);
    }

    // Window bookkeeping

    #[test]
    fn recent_windows_keep_the_latest_twenty_without_repeats() {
        let (mgr, _fake, _) = rig();
        for i in 0..25 {
            focus(&mgr, &format!("app{i}"), "DP-3");
        }
        {
            let st = mgr.appsync.lock().unwrap();
            assert_eq!(st.recent.len(), 20);
            assert_eq!(st.recent.front().map(|w| w.class.as_str()), Some("app24"));
            assert_eq!(st.recent.back().map(|w| w.class.as_str()), Some("app5"));
        }
        // Focusing one again moves it to the front instead of adding a copy.
        focus(&mgr, "app10", "DP-3");
        let st = mgr.appsync.lock().unwrap();
        assert_eq!(st.recent.len(), 20);
        assert_eq!(st.recent.front().map(|w| w.class.as_str()), Some("app10"));
        assert_eq!(st.recent.iter().filter(|w| w.class == "app10").count(), 1);
    }

    #[test]
    fn windows_without_a_name_are_not_remembered() {
        let (mgr, _fake, _) = rig();
        focus(&mgr, "", "");
        let st = mgr.appsync.lock().unwrap();
        assert!(st.recent.is_empty());
        // An empty output is "none", not an output called "".
        assert_eq!(st.active_output, None);
    }

    #[test]
    fn the_executable_name_comes_from_proc() {
        assert_eq!(exe_name(0), "");
        assert_eq!(exe_name(-1), "");
        let me = std::env::current_exe().unwrap();
        assert_eq!(exe_name(std::process::id() as i32), me.file_name().unwrap().to_string_lossy());
        // No such process.
        assert_eq!(exe_name(i32::MAX), "");
    }
}
