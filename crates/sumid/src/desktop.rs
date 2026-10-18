//! DisplayWidget's OLED-friendly desktop toggles, mapped to Plasma: panel
//! auto-hide (taskbar auto-hide) and a dark colour scheme. Sumi saves what the
//! user had and restores it when the toggle is turned off.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tracing::info;
use zbus::blocking::Connection;

use crate::manager::Manager;

#[derive(Debug, Clone, Serialize)]
pub struct DesktopPrefs {
    pub panel_autohide: Option<bool>,
    pub dark_theme: Option<bool>,
}

fn plasma_script(script: &str) -> Result<String> {
    let conn = Connection::session()?;
    let reply = conn
        .call_method(Some("org.kde.plasmashell"), "/PlasmaShell", Some("org.kde.PlasmaShell"), "evaluateScript", &(script,))
        .context("Plasma shell scripting")?;
    Ok(reply.body().deserialize()?)
}

fn panel_hiding() -> Result<Vec<String>> {
    let out = plasma_script("print(panels().map(function (p) { return p.hiding; }).join('\\n'));")?;
    Ok(out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
}

fn set_panel_hiding(values: &[String]) -> Result<()> {
    let json = serde_json::to_string(values)?;
    plasma_script(&format!(
        "var v = {json}; panels().forEach(function (p, i) {{ p.hiding = v[Math.min(i, v.length - 1)]; }});"
    ))?;
    Ok(())
}

fn color_scheme() -> Result<String> {
    let out = std::process::Command::new("kreadconfig6")
        .args(["--file", "kdeglobals", "--group", "General", "--key", "ColorScheme", "--default", "BreezeLight"])
        .output()
        .context("running kreadconfig6")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn apply_color_scheme(name: &str) -> Result<()> {
    let st = std::process::Command::new("plasma-apply-colorscheme").arg(name).status().context("running plasma-apply-colorscheme")?;
    if !st.success() {
        bail!("plasma-apply-colorscheme {name} failed");
    }
    Ok(())
}

fn is_dark(scheme: &str) -> bool {
    scheme.to_ascii_lowercase().contains("dark")
}

impl Manager {
    pub fn desktop_prefs(&self) -> DesktopPrefs {
        DesktopPrefs {
            panel_autohide: panel_hiding().ok().filter(|v| !v.is_empty()).map(|v| v.iter().all(|h| h == "autohide")),
            dark_theme: color_scheme().ok().map(|s| is_dark(&s)),
        }
    }

    pub fn set_panel_autohide(&self, on: bool) -> Result<()> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let mut cfg = self.config.lock().unwrap().clone();
        let current = panel_hiding()?;
        if current.is_empty() {
            bail!("no Plasma panels found");
        }
        if on {
            if current.iter().all(|h| h == "autohide") {
                return Ok(());
            }
            cfg.desktop_saved.panel_hiding = current.clone();
            set_panel_hiding(&vec!["autohide".to_string(); current.len()])?;
        } else {
            let before = std::mem::take(&mut cfg.desktop_saved.panel_hiding);
            let restore: Vec<String> = if before.is_empty() || before.iter().all(|h| h == "autohide") {
                vec!["none".to_string(); current.len()]
            } else {
                before
            };
            set_panel_hiding(&restore)?;
        }
        self.save_config(cfg)?;
        info!("panel auto-hide {}", if on { "on" } else { "off" });
        Ok(())
    }

    pub fn set_dark_theme(&self, on: bool) -> Result<()> {
        if self.read_only {
            bail!("sumid is running read-only");
        }
        let mut cfg = self.config.lock().unwrap().clone();
        let current = color_scheme()?;
        if on {
            if is_dark(&current) {
                return Ok(());
            }
            cfg.desktop_saved.color_scheme = Some(current);
            apply_color_scheme("BreezeDark")?;
        } else {
            let before = cfg.desktop_saved.color_scheme.take().filter(|s| !is_dark(s)).unwrap_or_else(|| "BreezeLight".into());
            apply_color_scheme(&before)?;
        }
        self.save_config(cfg)?;
        info!("dark theme {}", if on { "on" } else { "off" });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_schemes_are_recognised_by_name() {
        assert!(is_dark("BreezeDark"));
        assert!(is_dark("org.kde.breezedark.desktop"));
        assert!(!is_dark("BreezeLight"));
        assert!(!is_dark(""));
    }
}

