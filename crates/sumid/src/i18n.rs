//! Translations for the few strings sumid shows itself (notifications, KDE
//! shortcut names), from the same `sumi` gettext catalogue the app uses, via
//! glibc's libintl. The language follows the app's setting
//! (`~/.config/sumi/app.json` → `language`), else the session locale.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;

const DOMAIN: &str = "sumi";

unsafe extern "C" {
    fn bindtextdomain(domain: *const c_char, dir: *const c_char) -> *mut c_char;
    fn bind_textdomain_codeset(domain: *const c_char, codeset: *const c_char) -> *mut c_char;
    fn dgettext(domain: *const c_char, msgid: *const c_char) -> *mut c_char;
}

/// The locale the app is set to, if any ("" = system, "osd" = the cached OSD locale).
fn app_language() -> Option<String> {
    let path = dirs::config_dir()?.join("sumi").join("app.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let lang = v.get("language")?.as_str()?.to_string();
    match lang.as_str() {
        "" => None,
        "osd" => v.get("osd_locale").and_then(|l| l.as_str()).map(String::from).filter(|l| !l.is_empty()),
        _ => Some(lang),
    }
}

/// Where `sumi.mo` catalogues live: the per-user install, else the system.
fn locale_dir() -> PathBuf {
    let user = dirs::data_dir().map(|d| d.join("locale"));
    match user {
        Some(d) if std::fs::read_dir(&d).into_iter().flatten().flatten().any(|e| e.path().join("LC_MESSAGES").join("sumi.mo").exists()) => d,
        _ => PathBuf::from("/usr/share/locale"),
    }
}

pub fn init() {
    if let Some(lang) = app_language() {
        // SAFETY: called once at startup before other threads read the environment.
        unsafe { std::env::set_var("LANGUAGE", lang) };
    }
    let empty = CString::new("").expect("no NUL");
    let domain = CString::new(DOMAIN).expect("no NUL");
    let dir = CString::new(locale_dir().to_string_lossy().as_bytes()).unwrap_or_else(|_| CString::new("/usr/share/locale").expect("no NUL"));
    let utf8 = CString::new("UTF-8").expect("no NUL");
    unsafe {
        libc::setlocale(libc::LC_ALL, empty.as_ptr());
        bindtextdomain(domain.as_ptr(), dir.as_ptr());
        bind_textdomain_codeset(domain.as_ptr(), utf8.as_ptr());
    }
}

/// Translate one msgid (English text when no catalogue has it).
pub fn tr(msgid: &str) -> String {
    let (Ok(domain), Ok(id)) = (CString::new(DOMAIN), CString::new(msgid)) else { return msgid.to_string() };
    unsafe {
        let p = dgettext(domain.as_ptr(), id.as_ptr());
        if p.is_null() { msgid.to_string() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}

/// Translate and fill KDE-style `%1` placeholders.
pub fn tr1(msgid: &str, arg: &str) -> String {
    tr(msgid).replace("%1", arg)
}

#[cfg(test)]
mod tests {
    use super::*;

    // `init` is never called here, so no catalogue is bound and the locale is "C".

    #[test]
    fn tr_returns_the_msgid_without_a_catalogue() {
        assert_eq!(tr("AI Visual"), "AI Visual");
        assert_eq!(tr("Switched to %1 mode"), "Switched to %1 mode");
        assert_eq!(tr("EZ OSD: open the monitor menu"), "EZ OSD: open the monitor menu");
        assert_eq!(tr("日本語 and ünïcode"), "日本語 and ünïcode");
        assert_eq!(tr(""), "");
    }

    #[test]
    fn tr_survives_a_nul_in_the_msgid() {
        assert_eq!(tr("a\0b"), "a\0b");
    }

    #[test]
    fn tr1_fills_the_placeholder() {
        assert_eq!(tr1("Switched to %1 mode", "FPS"), "Switched to FPS mode");
        assert_eq!(tr1("%1", "Cinema"), "Cinema");
        assert_eq!(tr1("%1 and %1", "x"), "x and x");
        // No placeholder: the text is unchanged.
        assert_eq!(tr1("AI Visual", "FPS"), "AI Visual");
    }

    #[test]
    fn every_daemon_string_passes_through_unchanged() {
        for s in sumi_core::strings::all() {
            assert_eq!(tr(s), s);
        }
    }
}
