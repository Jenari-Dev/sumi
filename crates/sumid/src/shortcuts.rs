//! Hot keys: Sumi's actions registered with KDE global shortcuts (component
//! "sumi"). Like DisplayWidget, nothing is bound by default; the user binds
//! keys in System Settings › Shortcuts. Actions target the monitor that holds
//! the active window (DisplayWidget: the monitor under the mouse cursor).

use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{info, warn};
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type;
use zbus::MatchRule;

use crate::manager::Manager;

const SERVICE: &str = "org.kde.kglobalaccel";
pub const COMPONENT: &str = "sumi";

pub use sumi_core::hotkeys::ACTIONS;

fn action_id(action: &str, label: &str) -> Vec<String> {
    vec![COMPONENT.into(), action.into(), "Sumi".into(), crate::i18n::tr(label)]
}

#[derive(Debug, Clone, Serialize)]
pub struct Binding {
    pub action: &'static str,
    pub label: &'static str,
    /// Human-readable key sequences currently bound in KDE.
    pub keys: Vec<String>,
}

/// Register every action (keeps the user's saved keys; no defaults).
pub fn register(conn: &Connection) -> Result<()> {
    for (action, label) in ACTIONS {
        let id = action_id(action, label);
        conn.call_method(Some(SERVICE), "/kglobalaccel", Some("org.kde.KGlobalAccel"), "doRegister", &(id.clone(),))?;
        // SetPresent (2): mark the action present; empty default; KDE loads saved keys.
        conn.call_method(Some(SERVICE), "/kglobalaccel", Some("org.kde.KGlobalAccel"), "setShortcut", &(id, Vec::<i32>::new(), 2u32))?;
    }
    Ok(())
}

/// Current bindings as text, for the HotKey page.
pub fn bindings(conn: &Connection) -> Result<Vec<Binding>> {
    ACTIONS
        .iter()
        .map(|(action, label)| {
            let id = action_id(action, label);
            let reply = conn.call_method(Some(SERVICE), "/kglobalaccel", Some("org.kde.KGlobalAccel"), "shortcut", &(id,))?;
            let keys: Vec<i32> = reply.body().deserialize()?;
            Ok(Binding { action, label, keys: keys.into_iter().filter(|&k| k != 0).map(key_text).collect() })
        })
        .collect()
}

/// Qt key code (with modifier bits) → "Ctrl+Alt+0"-style text.
pub fn key_text(code: i32) -> String {
    // Qt 6 / Plasma order: Meta+Ctrl+Alt+Shift.
    let mut parts = Vec::new();
    if code & 0x1000_0000 != 0 {
        parts.push("Meta".to_string());
    }
    if code & 0x0400_0000 != 0 {
        parts.push("Ctrl".to_string());
    }
    if code & 0x0800_0000 != 0 {
        parts.push("Alt".to_string());
    }
    if code & 0x0200_0000 != 0 {
        parts.push("Shift".to_string());
    }
    let k = code & 0x01FF_FFFF;
    let name = match k {
        0x20 => "Space".to_string(),
        0x21..=0x7E => char::from_u32(k as u32).map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default(),
        0x0100_0000 => "Esc".into(),
        0x0100_0001 => "Tab".into(),
        0x0100_0003 => "Backspace".into(),
        0x0100_0004 | 0x0100_0005 => "Enter".into(),
        0x0100_0006 => "Ins".into(),
        0x0100_0007 => "Del".into(),
        0x0100_0010 => "Home".into(),
        0x0100_0011 => "End".into(),
        0x0100_0012 => "Left".into(),
        0x0100_0013 => "Up".into(),
        0x0100_0014 => "Right".into(),
        0x0100_0015 => "Down".into(),
        0x0100_0016 => "PgUp".into(),
        0x0100_0017 => "PgDown".into(),
        0x0100_0030..=0x0100_0052 => format!("F{}", k - 0x0100_0030 + 1),
        _ => format!("0x{k:X}"),
    };
    parts.push(name);
    parts.join("+")
}

pub fn spawn(mgr: Arc<Manager>) {
    std::thread::Builder::new()
        .name("shortcuts".into())
        .spawn(move || {
            if let Err(e) = run(&mgr) {
                warn!("global shortcuts unavailable (KDE kglobalaccel): {e:#}");
            }
        })
        .ok();
}

fn run(mgr: &Arc<Manager>) -> Result<()> {
    let conn = Connection::session()?;
    register(&conn).context("registering with kglobalaccel")?;
    let rule = MatchRule::builder().msg_type(Type::Signal).interface("org.kde.kglobalaccel.Component")?.member("globalShortcutPressed")?.build();
    info!("hot keys registered (bind them in System Settings › Shortcuts › Sumi)");
    for msg in MessageIterator::for_match_rule(rule, &conn, None)? {
        let Ok(msg) = msg else { continue };
        let Ok((component, action, _ts)) = msg.body().deserialize::<(String, String, i64)>() else { continue };
        if component != COMPONENT || !mgr.config.lock().unwrap().hotkeys.enabled {
            continue;
        }
        let m = mgr.clone();
        std::thread::spawn(move || {
            if let Err(e) = m.hotkey(&action) {
                warn!("hot key {action}: {e:#}");
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: i32 = 0x0400_0000;
    const ALT: i32 = 0x0800_0000;
    const META: i32 = 0x1000_0000;
    const SHIFT: i32 = 0x0200_0000;
    const SPECIAL: i32 = 0x0100_0000;

    #[test]
    fn letters_and_digits_are_upper_case() {
        assert_eq!(key_text('A' as i32), "A");
        assert_eq!(key_text('z' as i32), "Z");
        assert_eq!(key_text('0' as i32), "0");
        assert_eq!(key_text('+' as i32), "+");
        assert_eq!(key_text(0x20), "Space");
    }

    #[test]
    fn modifiers_are_named() {
        assert_eq!(key_text(CTRL | ALT | '0' as i32), "Ctrl+Alt+0");
        assert_eq!(key_text(CTRL | 'S' as i32), "Ctrl+S");
        assert_eq!(key_text(ALT | 'x' as i32), "Alt+X");
        assert_eq!(key_text(META | 'E' as i32), "Meta+E");
        assert_eq!(key_text(SHIFT | CTRL | 'K' as i32), "Ctrl+Shift+K");
        assert_eq!(key_text(CTRL | ALT | META | SHIFT | 'Q' as i32), "Meta+Ctrl+Alt+Shift+Q");
    }

    #[test]
    fn function_keys_are_numbered_from_f1() {
        assert_eq!(key_text(SPECIAL | 0x30), "F1");
        assert_eq!(key_text(SPECIAL | 0x3B), "F12");
        assert_eq!(key_text(SPECIAL | 0x52), "F35");
        assert_eq!(key_text(CTRL | SHIFT | SPECIAL | 0x33), "Ctrl+Shift+F4");
        // The next code up is not an F-key.
        assert_eq!(key_text(SPECIAL | 0x53), "0x1000053");
    }

    #[test]
    fn arrows_and_navigation_keys() {
        for (code, name) in [
            (0x12, "Left"),
            (0x13, "Up"),
            (0x14, "Right"),
            (0x15, "Down"),
            (0x10, "Home"),
            (0x11, "End"),
            (0x16, "PgUp"),
            (0x17, "PgDown"),
        ] {
            assert_eq!(key_text(SPECIAL | code), name);
        }
        assert_eq!(key_text(CTRL | ALT | SPECIAL | 0x13), "Ctrl+Alt+Up");
    }

    #[test]
    fn editing_keys() {
        for (code, name) in [(0x00, "Esc"), (0x01, "Tab"), (0x03, "Backspace"), (0x04, "Enter"), (0x05, "Enter"), (0x06, "Ins"), (0x07, "Del")] {
            assert_eq!(key_text(SPECIAL | code), name);
        }
    }

    #[test]
    fn unknown_keys_show_their_code() {
        assert_eq!(key_text(SPECIAL | 0x02), "0x1000002");
        assert_eq!(key_text(CTRL | 0xE9), "Ctrl+0xE9");
        assert_eq!(key_text(0), "0x0");
        assert_eq!(key_text(CTRL), "Ctrl+0x0");
    }

    #[test]
    fn the_keypad_modifier_is_not_shown() {
        assert_eq!(key_text(0x2000_0000 | '5' as i32), "5");
    }

    #[test]
    fn every_action_has_a_kglobalaccel_id() {
        for (action, label) in ACTIONS {
            let id = action_id(action, label);
            // component, action, friendly component name, friendly action name
            assert_eq!(id[..3], [COMPONENT, *action, "Sumi"]);
            assert!(!id[3].is_empty());
        }
    }
}
