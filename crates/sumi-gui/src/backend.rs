//! QML-facing backend: a thin client of sumid's D-Bus API. All monitor access
//! goes through the daemon. Every call runs on a worker thread and reports back
//! through `qt_thread().queue`, so the Qt thread never waits on DDC.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, connected)]
        #[qproperty(bool, readonly)]
        #[qproperty(QString, daemon_version)]
        #[qproperty(QString, monitors)]
        #[qproperty(QString, status)]
        #[qproperty(bool, busy)]
        type Backend = super::BackendRust;

        /// Re-query the daemon: `ListMonitors` plus the `ReadOnly` property.
        /// Sets the properties and emits `monitors_changed`.
        #[qinvokable]
        fn refresh(self: Pin<&mut Backend>);

        /// Ask for a monitor's decoded state. `pages_csv` is a comma-separated
        /// list of page ids ("" = all). Answers with `state_ready` (empty JSON on failure).
        #[qinvokable]
        fn request_state(self: Pin<&mut Backend>, id: &QString, pages_csv: &QString);

        /// Write one feature. Calls are coalesced per (monitor, feature): while a
        /// write is in flight only the latest value is kept and sent afterwards.
        /// Answers with `applied` for every write that was sent.
        #[qinvokable]
        fn apply(self: Pin<&mut Backend>, id: &QString, feature: &QString, value: i32);

        /// Relay daemon signals (call once).
        #[qinvokable]
        fn start_events(self: Pin<&mut Backend>);

        /// Environment variable value ("" if unset); used for headless UI checks
        /// (SUMI_SCREENSHOT=/path.png, SUMI_PAGE=<page>).
        #[qinvokable]
        fn env_var(self: &Backend, name: &QString) -> QString;

        /// Copy text to the clipboard.
        #[qinvokable]
        fn copy_text(self: &Backend, text: &QString);

        /// This app's version.
        #[qinvokable]
        fn app_version(self: &Backend) -> QString;

        /// True when started with `--hidden` (autostart): the window opens minimised.
        #[qinvokable]
        fn hidden_start(self: &Backend) -> bool;

        /// Whether the autostart entry exists.
        #[qinvokable]
        fn autostart_enabled(self: &Backend) -> bool;

        /// The user manual's URL: the installed README, else the vendor's support page.
        #[qinvokable]
        fn manual_url(self: &Backend) -> QString;

        /// Start a new Sumi (it waits for this one to let go of the single-instance name)
        /// and quit this one; used after the language was changed.
        #[qinvokable]
        fn restart(self: &Backend);

        /// Create or remove `~/.config/autostart/io.github.jenaridev.sumi.desktop`.
        #[qinvokable]
        fn set_autostart(self: &Backend, on: bool) -> bool;

        /// Call any other sumid method (see `invoke` for the list). `args_json` is a JSON
        /// array of the method's arguments. Answers with `call_done(tag, ok, result)`: the
        /// method's string result ("" for methods without one), or the error text. With
        /// `notify` an error also lands in `status` (the toast).
        #[qinvokable]
        fn call(self: Pin<&mut Backend>, method: &QString, args_json: &QString, tag: &QString, notify: bool);

        /// A small per-user preference of the app itself ("" if unset), kept in
        /// `~/.config/sumi/app.json` (the daemon's own config is never touched).
        #[qinvokable]
        fn pref(self: &Backend, key: &QString) -> QString;

        #[qinvokable]
        fn set_pref(self: &Backend, key: &QString, value: &QString) -> bool;

        /// Read a text file chosen in a file dialog (`url` is a file:// URL or a path); "" on error.
        #[qinvokable]
        fn read_text_file(self: Pin<&mut Backend>, url: &QString) -> QString;

        /// Write a text file chosen in a file dialog; false on error (see `status`).
        #[qinvokable]
        fn write_text_file(self: Pin<&mut Backend>, url: &QString, text: &QString) -> bool;

        /// Open a KDE System Settings page ("kcm_keys").
        #[qinvokable]
        fn open_system_settings(self: &Backend, module: &QString) -> bool;

        /// Whether a system tray (a StatusNotifier host) is running. Qt Labs Platform aborts the
        /// program when it is asked for a tray icon without one, so the tray is only created
        /// when this is true. Set `SUMI_NO_TRAY=1` to force it off.
        #[qinvokable]
        fn tray_available(self: &Backend) -> bool;

        /// Claim the single-instance name on the session bus. When another Sumi already
        /// owns it, that instance is asked to show itself and this returns false.
        #[qinvokable]
        fn claim_instance(self: Pin<&mut Backend>) -> bool;
    }

    unsafe extern "RustQt" {
        // `monitors_changed` (the `monitors` property's notify signal, QML `onMonitorsChanged`)
        // is also emitted after every `refresh`, even when the list is unchanged.

        /// Result of `request_state`: JSON, or "" if the call failed (see `status`).
        #[qsignal]
        fn state_ready(self: Pin<&mut Backend>, id: QString, pages: QString, json: QString);

        /// Result of one sent write.
        #[qsignal]
        fn applied(self: Pin<&mut Backend>, id: QString, feature: QString, value: i32, ok: bool, message: QString);

        /// The monitor changed something itself (OSD, HDR signal, power saving).
        #[qsignal]
        fn monitor_changed(self: Pin<&mut Backend>, id: QString, context: QString);

        /// Result of `call`.
        #[qsignal]
        fn call_done(self: Pin<&mut Backend>, tag: QString, ok: bool, result: QString);

        /// A second Sumi was started: show and raise the window.
        #[qsignal]
        fn activate_requested(self: Pin<&mut Backend>);
    }

    impl cxx_qt::Threading for Backend {}
}

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use zbus::blocking::Connection;

const BUS: &str = "io.github.JenAriDev.Sumi";
const PATH: &str = "/io/github/JenAriDev/Sumi";
const IFACE: &str = "io.github.JenAriDev.Sumi1";
const DESKTOP_FILE: &str = "io.github.jenaridev.sumi.desktop";
const APP_NAME: &str = "io.github.JenAriDev.Sumi.App";
const APP_PATH: &str = "/io/github/JenAriDev/Sumi/App";

/// Calls currently running on worker threads (drives the `busy` property).
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
/// Runtime the single-instance object is served on (zbus spawns its tasks on tokio here).
static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
/// The shared blocking connection (opened lazily, reused by every worker).
static CONN: Mutex<Option<Connection>> = Mutex::new(None);

/// Writes in flight, keyed by (monitor, feature); the value is the latest write waiting for its turn.
type Writes = Arc<Mutex<HashMap<(String, String), Option<u16>>>>;

#[derive(Default)]
pub struct BackendRust {
    connected: bool,
    readonly: bool,
    daemon_version: QString,
    monitors: QString,
    status: QString,
    busy: bool,
    events_started: bool,
    writes: Writes,
}

fn conn() -> zbus::Result<Connection> {
    let mut slot = CONN.lock().unwrap();
    if slot.is_none() {
        *slot = Some(Connection::session()?);
    }
    Ok(slot.clone().expect("just set"))
}

/// Forget the shared connection when the bus itself went away (the next call reconnects).
fn forget_dead_connection(e: &zbus::Error) {
    if matches!(e, zbus::Error::InputOutput(_)) {
        *CONN.lock().unwrap() = None;
    }
}

fn call<B, R>(conn: &Connection, method: &str, body: &B) -> zbus::Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> serde::Deserialize<'d> + zbus::zvariant::Type,
{
    conn.call_method(Some(BUS), PATH, Some(IFACE), method, body)?.body().deserialize()
}

/// The daemon's own error message when there is one.
fn error_text(e: &zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(_, Some(msg), _) => msg.clone(),
        e => e.to_string(),
    }
}

/// True when nobody owns the daemon's bus name (sumid is not running).
fn daemon_missing(e: &zbus::Error) -> bool {
    matches!(e, zbus::Error::MethodError(name, _, _)
        if name.as_str().ends_with("ServiceUnknown") || name.as_str().ends_with("NameHasNoOwner"))
}

/// Why a daemon call failed, kept as data until it reaches the Qt thread (where it is translated).
enum Failure {
    Missing,
    Other(String),
}

impl Failure {
    fn of(e: &zbus::Error) -> Self {
        if daemon_missing(e) { Failure::Missing } else { Failure::Other(error_text(e)) }
    }

    fn text(&self) -> String {
        match self {
            Failure::Missing => crate::i18n::ffi::sumi_tr_not_running().to_string(),
            Failure::Other(t) => t.clone(),
        }
    }
}

/// Call one sumid method by name with JSON arguments; the result is the method's string
/// result, or "" for methods that return nothing.
fn invoke(method: &str, args: &[serde_json::Value]) -> zbus::Result<String> {
    use serde_json::Value;
    let conn = conn()?;
    let s = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or_default().to_string();
    let b = |i: usize| args.get(i).and_then(Value::as_bool).unwrap_or(false);
    match method {
        "ListMonitors" | "Problems" | "Catalogue" | "GetConfig" | "ListApps" | "RecentWindows" | "Hotkeys" | "TileLayoutNames"
        | "GetTileLayouts" | "GetDesktopPrefs" | "Gamepads" | "AiStatus" => call::<(), String>(&conn, method, &()),
        "DownloadAiModel" | "RemoveAiModel" => call::<(), ()>(&conn, method, &()).map(|()| String::new()),
        "Capabilities" | "GetHostState" | "ExportSettings" | "AiClassifyNow" => call::<(String,), String>(&conn, method, &(s(0),)),
        "GetVcp" => {
            let code = args.get(1).and_then(Value::as_u64).unwrap_or(0).min(255) as u8;
            call::<(String, u8), (u16, u16)>(&conn, method, &(s(0), code)).map(|(cur, max)| format!("[{cur},{max}]"))
        }
        "GetState" => {
            let pages: Vec<String> =
                args.get(1).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
            call::<(String, Vec<String>), String>(&conn, method, &(s(0), pages))
        }
        "SetConfig" | "RunAction" => call::<(String,), ()>(&conn, method, &(s(0),)).map(|()| String::new()),
        "SetHdr" => call::<(String, bool), ()>(&conn, method, &(s(0), b(1))).map(|()| String::new()),
        "SetTargetMode" | "SetPanelAutohide" | "SetDarkTheme" => call::<(bool,), ()>(&conn, method, &(b(0),)).map(|()| String::new()),
        "ImportSettings" => call::<(String, String, bool), String>(&conn, method, &(s(0), s(1), b(2))),
        "SetTileLayout" => {
            let n = args.get(1).and_then(Value::as_u64).unwrap_or(0).min(255) as u8;
            call::<(String, u8), String>(&conn, method, &(s(0), n))
        }
        _ => Err(zbus::Error::Failure(format!("unknown method {method}"))),
    }
}

fn config_base() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
}

fn prefs_path() -> Option<std::path::PathBuf> {
    Some(config_base()?.join("sumi").join("app.json"))
}

/// One of the app's own preferences ("" if unset), see `Backend::pref`.
pub fn read_pref(key: &str) -> String {
    prefs_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get(key).and_then(|x| x.as_str().map(String::from)))
        .unwrap_or_default()
}

/// The language the app should use, if it is not the system's: the SUMI_LANGUAGE
/// environment variable wins over the `language` preference ("" = system,
/// "osd" = the language the monitor's menu was last seen in, or a locale).
pub fn chosen_language() -> Option<String> {
    language_from(&std::env::var("SUMI_LANGUAGE").unwrap_or_default(), &read_pref("language"), &read_pref("osd_locale"))
}

fn language_from(env: &str, pref: &str, osd_locale: &str) -> Option<String> {
    if !env.is_empty() {
        return Some(env.to_string());
    }
    match pref {
        "" => None,
        "osd" => Some(osd_locale.to_string()).filter(|l| !l.is_empty()),
        locale => Some(locale.to_string()),
    }
}

/// Where the user manual lives once installed.
fn manual_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))?;
    Some(base.join("doc").join("sumi").join("README.md"))
}

/// A path from a file:// URL (percent-decoded) or a plain path.
fn path_from_url(url: &str) -> std::path::PathBuf {
    let raw = url.strip_prefix("file://").unwrap_or(url);
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    std::path::PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

/// The object other Sumi instances call to bring the running one to the front.
struct AppIface {
    qt: Mutex<cxx_qt::CxxQtThread<qobject::Backend>>,
}

#[zbus::interface(name = "io.github.JenAriDev.Sumi.App")]
impl AppIface {
    fn activate(&self) {
        let _ = self.qt.lock().unwrap().queue(|o| o.activate_requested());
    }
}

/// A property of sumid's interface ("" when it has none, e.g. an older daemon).
fn daemon_property(conn: &Connection, name: &str) -> Option<zbus::zvariant::OwnedValue> {
    conn.call_method(Some(BUS), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(IFACE, name))
        .ok()
        .and_then(|m| m.body().deserialize::<zbus::zvariant::OwnedValue>().ok())
}

fn daemon_version(conn: &Connection) -> String {
    daemon_property(conn, "Version").and_then(|v| String::try_from(v).ok()).unwrap_or_default()
}

fn read_only(conn: &Connection) -> bool {
    daemon_property(conn, "ReadOnly").and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
}

fn qs(s: &str) -> QString {
    QString::from(s)
}

fn autostart_path() -> Option<std::path::PathBuf> {
    Some(config_base()?.join("autostart").join(DESKTOP_FILE))
}

/// The autostart entry: starts the app minimised (`--hidden`) at login.
fn autostart_entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Sumi\nComment=Picture modes, OLED care and GamePlus for your monitor\n\
         Exec={} --hidden\nIcon=io.github.jenaridev.sumi\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_quote(exe)
    )
}

/// Quote a path for a desktop-entry `Exec` line.
fn exec_quote(s: &str) -> String {
    if s.chars().any(|c| c.is_whitespace() || "\"'\\$`<>~|&;*?#()".contains(c)) {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    } else {
        s.to_string()
    }
}

impl qobject::Backend {
    /// Run `work` on a worker thread, then `done` on the Qt thread with its result.
    fn spawn<R, W, D>(mut self: Pin<&mut Self>, work: W, done: D)
    where
        R: Send + 'static,
        W: FnOnce() -> R + Send + 'static,
        D: FnOnce(Pin<&mut qobject::Backend>, R) + Send + 'static,
    {
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let r = work();
            IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
            let _ = qt.queue(move |mut o| {
                done(o.as_mut(), r);
                o.set_busy(IN_FLIGHT.load(Ordering::SeqCst) > 0);
            });
        });
    }

    pub fn refresh(self: Pin<&mut Self>) {
        self.spawn(
            || -> zbus::Result<(String, bool, String)> {
                let conn = conn()?;
                let list: String = call(&conn, "ListMonitors", &())?;
                Ok((list, read_only(&conn), daemon_version(&conn)))
            },
            |mut o, r| {
                let before = o.monitors().to_string();
                // `monitors` goes last: its change signal is the one the UI reacts to.
                match r {
                    Ok((list, ro, version)) => {
                        o.as_mut().set_connected(true);
                        o.as_mut().set_readonly(ro);
                        o.as_mut().set_daemon_version(qs(&version));
                        o.as_mut().set_status(QString::default());
                        o.as_mut().set_monitors(qs(&list));
                    }
                    Err(e) => {
                        forget_dead_connection(&e);
                        let text = match Failure::of(&e) {
                            Failure::Missing => Failure::Missing.text(),
                            Failure::Other(t) => crate::i18n::ffi::sumi_tr_dbus_error(&qs(&t)).to_string(),
                        };
                        o.as_mut().set_connected(false);
                        o.as_mut().set_status(qs(&text));
                        o.as_mut().set_monitors(qs("[]"));
                    }
                }
                if o.monitors().to_string() == before {
                    o.as_mut().monitors_changed();
                }
            },
        );
    }

    pub fn request_state(self: Pin<&mut Self>, id: &QString, pages_csv: &QString) {
        let (id, csv) = (id.to_string(), pages_csv.to_string());
        let pages: Vec<String> = csv.split(',').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect();
        let (id2, csv2) = (id.clone(), csv.clone());
        self.spawn(
            move || -> zbus::Result<String> { call(&conn()?, "GetState", &(id, pages)) },
            move |mut o, r| {
                let json = match r {
                    Ok(json) => {
                        o.as_mut().set_status(QString::default());
                        json
                    }
                    Err(e) => {
                        let f = Failure::of(&e);
                        if matches!(f, Failure::Missing) {
                            o.as_mut().set_connected(false);
                        }
                        o.as_mut().set_status(qs(&f.text()));
                        String::new()
                    }
                };
                o.as_mut().state_ready(qs(&id2), qs(&csv2), qs(&json));
            },
        );
    }

    pub fn apply(mut self: Pin<&mut Self>, id: &QString, feature: &QString, value: i32) {
        let key = (id.to_string(), feature.to_string());
        let mut v = value.clamp(0, u16::MAX as i32) as u16;
        let writes = self.writes.clone();
        {
            let mut map = writes.lock().unwrap();
            if let Some(pending) = map.get_mut(&key) {
                // A write for this feature is already in flight: remember only the latest.
                *pending = Some(v);
                return;
            }
            map.insert(key.clone(), None);
        }
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                let r: zbus::Result<String> = conn().and_then(|c| call(&c, "Apply", &(key.0.clone(), key.1.clone(), v)));
                let outcome: Result<String, Failure> = r.as_ref().map(Clone::clone).map_err(Failure::of);
                let next = {
                    let mut map = writes.lock().unwrap();
                    match map.get_mut(&key).and_then(Option::take) {
                        Some(n) => Some(n),
                        None => {
                            map.remove(&key);
                            None
                        }
                    }
                };
                let last = next.is_none();
                if last {
                    IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
                }
                let (k, sent) = (key.clone(), v);
                let queued = qt.queue(move |mut o| {
                    let (ok, message) = match &outcome {
                        Ok(s) => (true, s.clone()),
                        Err(f) => (false, f.text()),
                    };
                    o.as_mut().set_status(if ok { QString::default() } else { qs(&message) });
                    o.as_mut().applied(qs(&k.0), qs(&k.1), i32::from(sent), ok, qs(&message));
                    o.set_busy(IN_FLIGHT.load(Ordering::SeqCst) > 0);
                });
                match next {
                    Some(n) if queued.is_ok() => v = n,
                    _ => break,
                }
            }
        });
    }

    pub fn start_events(mut self: Pin<&mut Self>) {
        if self.events_started {
            return;
        }
        self.as_mut().rust_mut().events_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let Ok(conn) = conn() else { return };
            let rule = match zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(BUS)
                .and_then(|b| b.interface(IFACE))
            {
                Ok(b) => b.build(),
                Err(_) => return,
            };
            let Ok(iter) = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, None) else { return };
            for msg in iter.flatten() {
                let member = msg.header().member().map(|m| m.to_string()).unwrap_or_default();
                let queued = match member.as_str() {
                    "StateChanged" => {
                        let Ok((id, ctx)) = msg.body().deserialize::<(String, String)>() else { continue };
                        qt.queue(move |mut o| o.as_mut().monitor_changed(qs(&id), qs(&ctx)))
                    }
                    "MonitorAdded" | "MonitorRemoved" => qt.queue(|o| o.refresh()),
                    _ => continue,
                };
                if queued.is_err() {
                    break;
                }
            }
        });
    }

    pub fn call(self: Pin<&mut Self>, method: &QString, args_json: &QString, tag: &QString, notify: bool) {
        let (method, tag) = (method.to_string(), tag.to_string());
        let args: Vec<serde_json::Value> = serde_json::from_str(&args_json.to_string()).unwrap_or_default();
        self.spawn(
            move || invoke(&method, &args).map_err(|e| (Failure::of(&e), e)),
            move |mut o, r| {
                let (ok, text) = match r {
                    Ok(s) => (true, s),
                    Err((f, e)) => {
                        forget_dead_connection(&e);
                        (false, f.text())
                    }
                };
                if !ok && notify {
                    o.as_mut().set_status(qs(&text));
                }
                o.as_mut().call_done(qs(&tag), ok, qs(&text));
            },
        );
    }

    pub fn pref(&self, key: &QString) -> QString {
        qs(&read_pref(&key.to_string()))
    }

    pub fn manual_url(&self) -> QString {
        match manual_path().filter(|p| p.is_file()) {
            Some(p) => qs(&format!("file://{}", p.display())),
            None => qs("https://www.asus.com/support/"),
        }
    }

    pub fn restart(&self) {
        crate::app::ffi::sumi_restart_application();
    }

    pub fn set_pref(&self, key: &QString, value: &QString) -> bool {
        let Some(path) = prefs_path() else { return false };
        let mut doc = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        doc[key.to_string()] = serde_json::Value::from(value.to_string());
        path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, doc.to_string()).is_ok()
    }

    pub fn read_text_file(mut self: Pin<&mut Self>, url: &QString) -> QString {
        let path = path_from_url(&url.to_string());
        match std::fs::read_to_string(&path) {
            Ok(t) => qs(&t),
            Err(e) => {
                self.as_mut().set_status(qs(&format!("{}: {e}", path.display())));
                QString::default()
            }
        }
    }

    pub fn write_text_file(mut self: Pin<&mut Self>, url: &QString, text: &QString) -> bool {
        let path = path_from_url(&url.to_string());
        match std::fs::write(&path, text.to_string()) {
            Ok(()) => true,
            Err(e) => {
                self.as_mut().set_status(qs(&format!("{}: {e}", path.display())));
                false
            }
        }
    }

    pub fn open_system_settings(&self, module: &QString) -> bool {
        let module = match module.to_string().as_str() {
            "kcm_keys" => "kcm_keys",
            _ => return false,
        };
        std::process::Command::new("systemsettings")
            .arg(module)
            .spawn()
            .map(|mut c| {
                std::thread::spawn(move || c.wait());
            })
            .is_ok()
    }

    pub fn tray_available(&self) -> bool {
        if std::env::var_os("SUMI_NO_TRAY").is_some() {
            return false;
        }
        let Ok(conn) = conn() else { return false };
        conn.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.kde.StatusNotifierWatcher", "IsStatusNotifierHostRegistered"),
        )
        .ok()
        .and_then(|m| m.body().deserialize::<zbus::zvariant::OwnedValue>().ok())
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
    }

    pub fn claim_instance(self: Pin<&mut Self>) -> bool {
        use zbus::fdo::{RequestNameFlags, RequestNameReply};
        let Ok(conn) = conn() else { return true };
        let rt = RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("tokio runtime"));
        let _in_runtime = rt.enter();
        let iface = AppIface { qt: Mutex::new(self.qt_thread()) };
        if conn.object_server().at(APP_PATH, iface).is_err() {
            return true;
        }
        // A restart (language change) starts before the old instance has let go of the name.
        let attempts = if std::env::args().any(|a| a == "--restart") { 40 } else { 1 };
        let mut reply = conn.request_name_with_flags(APP_NAME, RequestNameFlags::DoNotQueue.into());
        for _ in 1..attempts {
            if matches!(reply, Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            reply = conn.request_name_with_flags(APP_NAME, RequestNameFlags::DoNotQueue.into());
        }
        match reply {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => true,
            Ok(RequestNameReply::Exists | RequestNameReply::InQueue) | Err(zbus::Error::NameTaken) => {
                // An autostart launch (`--hidden`) must not pop the running window up.
                if !std::env::args().any(|a| a == "--hidden") {
                    let _ = conn.call_method(Some(APP_NAME), APP_PATH, Some(APP_NAME), "Activate", &());
                }
                false
            }
            Err(_) => true,
        }
    }

    pub fn env_var(&self, name: &QString) -> QString {
        qs(&std::env::var(name.to_string()).unwrap_or_default())
    }

    pub fn copy_text(&self, text: &QString) {
        crate::clipboard::ffi::sumi_clipboard_set_text(text);
    }

    pub fn app_version(&self) -> QString {
        qs(env!("CARGO_PKG_VERSION"))
    }

    pub fn hidden_start(&self) -> bool {
        std::env::args().any(|a| a == "--hidden")
    }

    pub fn autostart_enabled(&self) -> bool {
        autostart_path().is_some_and(|p| p.is_file())
    }

    pub fn set_autostart(&self, on: bool) -> bool {
        let Some(path) = autostart_path() else { return false };
        if !on {
            return match std::fs::remove_file(&path) {
                Ok(()) => true,
                Err(e) => e.kind() == std::io::ErrorKind::NotFound,
            };
        }
        let exe = std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "sumi-app".into());
        let entry = autostart_entry(&exe);
        path.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&path, entry).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_paths_are_quoted_for_desktop_entries() {
        assert_eq!(exec_quote("/usr/bin/sumi-app"), "/usr/bin/sumi-app");
        assert_eq!(exec_quote("/opt/My Apps/sumi-app"), "\"/opt/My Apps/sumi-app\"");
        assert_eq!(exec_quote("/tmp/a\"b"), "\"/tmp/a\\\"b\"");
        assert_eq!(exec_quote("/tmp/$HOME"), "\"/tmp/\\$HOME\"");
    }

    #[test]
    fn language_choice_env_beats_pref_and_osd_follows_the_cached_menu_language() {
        assert_eq!(language_from("ja", "de", ""), Some("ja".into()));
        assert_eq!(language_from("", "", "fr"), None, "system language");
        assert_eq!(language_from("", "de", "fr"), Some("de".into()));
        assert_eq!(language_from("", "osd", "fr"), Some("fr".into()));
        assert_eq!(language_from("", "osd", ""), None, "no menu language seen yet");
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(path_from_url("file:///home/u/My%20Files/a%2Bb.json"), std::path::PathBuf::from("/home/u/My Files/a+b.json"));
        assert_eq!(path_from_url("/tmp/x.json"), std::path::PathBuf::from("/tmp/x.json"));
        assert_eq!(path_from_url("file:///tmp/100%"), std::path::PathBuf::from("/tmp/100%"));
    }

    #[test]
    fn autostart_entry_starts_hidden() {
        let e = autostart_entry("/usr/bin/sumi-app");
        assert!(e.starts_with("[Desktop Entry]\n"));
        assert!(e.contains("\nExec=/usr/bin/sumi-app --hidden\n"));
        assert!(e.contains("\nIcon=io.github.jenaridev.sumi\n"));
    }

    #[test]
    fn daemon_errors_are_recognised() {
        let missing = zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from("org.freedesktop.DBus.Error.ServiceUnknown").unwrap(),
            Some("The name is not activatable".into()),
            zbus::message::Message::method_call("/", "Ping").unwrap().interface("a.b").unwrap().destination("a.b").unwrap().build(&()).unwrap(),
        );
        assert!(daemon_missing(&missing));
        assert_eq!(error_text(&missing), "The name is not activatable");
        assert!(!daemon_missing(&zbus::Error::Unsupported));
    }
}
