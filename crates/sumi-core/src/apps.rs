//! Installed applications from XDG desktop entries, for the App Sync picker.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct App {
    /// Desktop-file id ("org.kde.dolphin", "steam_app_1172470"…).
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The key App Sync matches a window against (window class when known).
    pub key: String,
}

fn data_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = dirs::data_dir() {
        v.push(d);
    }
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    v
}

/// Parse the `[Desktop Entry]` group of one file.
fn parse_entry(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    out
}

/// The window class an entry's windows will most likely report.
fn match_key(id: &str, e: &BTreeMap<String, String>) -> String {
    if let Some(c) = e.get("StartupWMClass").filter(|c| !c.is_empty()) {
        return c.clone();
    }
    // Steam shortcuts: `steam steam://rungameid/<appid>` → Proton/XWayland class `steam_app_<appid>`.
    if let Some(exec) = e.get("Exec") {
        if let Some(rest) = exec.split("steam://rungameid/").nth(1) {
            let appid: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !appid.is_empty() {
                return format!("steam_app_{appid}");
            }
        }
    }
    id.to_string()
}

fn scan_dir(root: &Path, dir: &Path, out: &mut BTreeMap<String, App>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            scan_dir(root, &path, out);
            continue;
        }
        if path.extension().is_none_or(|x| x != "desktop") {
            continue;
        }
        // Desktop-file id: path below applications/ with '/' → '-', minus ".desktop".
        let Ok(rel) = path.strip_prefix(root) else { continue };
        let id = rel.to_string_lossy().trim_end_matches(".desktop").replace('/', "-");
        if out.contains_key(&id) {
            continue; // earlier data dirs take precedence
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let e = parse_entry(&text);
        let yes = |k: &str| e.get(k).is_some_and(|v| v == "true");
        if e.get("Type").map(String::as_str) != Some("Application") || yes("NoDisplay") || yes("Hidden") {
            continue;
        }
        let Some(name) = e.get("Name").cloned() else { continue };
        let key = match_key(&id, &e);
        out.insert(id.clone(), App { id, name, icon: e.get("Icon").cloned().unwrap_or_default(), key });
    }
}

/// Visible applications, sorted by name.
pub fn installed() -> Vec<App> {
    let mut found = BTreeMap::new();
    for d in data_dirs() {
        let root = d.join("applications");
        scan_dir(&root, &root, &mut found);
    }
    let mut v: Vec<App> = found.into_values().collect();
    v.sort_by_key(|a| a.name.to_lowercase());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_and_wmclass_keys() {
        let e = parse_entry("[Desktop Entry]\nType=Application\nName=Game\nExec=steam steam://rungameid/1172470\n");
        assert_eq!(match_key("Game", &e), "steam_app_1172470");
        let e = parse_entry("[Desktop Entry]\nName=X\nStartupWMClass=firefox\n[Desktop Action new]\nName=Y\n");
        assert_eq!(e.get("Name").unwrap(), "X");
        assert_eq!(match_key("org.mozilla.firefox", &e), "firefox");
        assert_eq!(match_key("org.kde.dolphin", &parse_entry("[Desktop Entry]\nName=D\n")), "org.kde.dolphin");
    }
}
