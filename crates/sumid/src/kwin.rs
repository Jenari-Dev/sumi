//! KWin integration (Plasma 6): the App Sync window watcher script, and
//! Target Mode via KWin's built-in Dim Inactive effect.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::{info, warn};
use zbus::blocking::Connection;

const KWIN: &str = "org.kde.KWin";
const PLUGIN: &str = "sumi-appsync";

/// Reports the active window (and moves between outputs) to sumid.
const SCRIPT: &str = r#"// Sumi App Sync: tell sumid which window is active and on which output.
var current = null;
function report(w) {
    if (!w || w.specialWindow || w.desktopWindow || w.dock) return;
    callDBus("io.github.JenAriDev.Sumi", "/io/github/JenAriDev/Sumi", "io.github.JenAriDev.Sumi1",
             "WindowActivated", String(w.resourceClass || ""), String(w.desktopFileName || ""),
             w.pid || 0, w.output ? String(w.output.name) : "");
}
function moved() { report(current); }
function track(w) {
    if (current) { try { current.outputChanged.disconnect(moved); } catch (e) {} }
    current = w;
    if (w) { try { w.outputChanged.connect(moved); } catch (e) {} }
    report(w);
}
workspace.windowActivated.connect(track);
track(workspace.activeWindow);
"#;

fn script_path() -> PathBuf {
    dirs::runtime_dir().or_else(dirs::cache_dir).unwrap_or_else(std::env::temp_dir).join("sumi-appsync.js")
}

/// Load (or reload) the App Sync script into KWin.
pub fn load_appsync(conn: &Connection) -> Result<()> {
    let path = script_path();
    std::fs::write(&path, SCRIPT).with_context(|| format!("writing {}", path.display()))?;
    let _ = unload_appsync(conn);
    let reply = conn
        .call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "loadScript", &(path.to_string_lossy().as_ref(), PLUGIN))
        .context("KWin scripting (is this a Plasma 6 session?)")?;
    let id: i32 = reply.body().deserialize()?;
    if id < 0 {
        anyhow::bail!("KWin refused the App Sync script");
    }
    let obj = format!("/Scripting/Script{id}");
    if conn.call_method(Some(KWIN), obj.as_str(), Some("org.kde.kwin.Script"), "run", &()).is_err() {
        conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "start", &())?;
    }
    info!("App Sync window watcher loaded into KWin");
    Ok(())
}

pub fn unload_appsync(conn: &Connection) -> Result<bool> {
    let reply = conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(PLUGIN,))?;
    Ok(reply.body().deserialize()?)
}

fn effects(conn: &Connection, method: &str, name: &str) -> Result<zbus::message::Message> {
    Ok(conn.call_method(Some(KWIN), "/Effects", Some("org.kde.kwin.Effects"), method, &(name,))?)
}

pub fn target_mode_active(conn: &Connection) -> Result<bool> {
    Ok(effects(conn, "isEffectLoaded", "diminactive")?.body().deserialize()?)
}

/// Turn Target Mode (KWin Dim Inactive) on or off; `strength` in percent.
pub fn set_target_mode(conn: &Connection, on: bool, strength: u8) -> Result<()> {
    if on {
        // The effect reads its strength from kwinrc.
        let ok = std::process::Command::new("kwriteconfig6")
            .args(["--file", "kwinrc", "--group", "Effect-diminactive", "--key", "Strength", &strength.min(100).to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            warn!("could not set the Dim Inactive strength (kwriteconfig6)");
        }
        let loaded: bool = effects(conn, "loadEffect", "diminactive")?.body().deserialize()?;
        let _ = effects(conn, "reconfigureEffect", "diminactive");
        if !loaded && !target_mode_active(conn)? {
            anyhow::bail!("KWin could not load the Dim Inactive effect");
        }
    } else {
        effects(conn, "unloadEffect", "diminactive")?;
    }
    Ok(())
}
