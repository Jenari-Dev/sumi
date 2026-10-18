#!/usr/bin/env python3
"""Build a throwaway QML harness: the app's QML with a fake Backend (a tiny pretend sumid that serves
fixtures/state-pg32ucdm3.json), plus the translations of one language taken from po/<lang>.po.
usage: make.py <outdir> <scenario> <page> <png> [snippet-file] [--lang xx] [--size WxH] [--env K=V ...]
scenarios: plain | rich (hot keys, apps, tile layouts) | hdr | rich_hdr | user | ai_ready"""
import json, sys, shutil, glob, os, re
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
out, scenario, page, png = sys.argv[1:5]
snippet = ""
lang = ""
size = None
env = {}
rest = sys.argv[5:]
while rest:
    a = rest.pop(0)
    if a == "--size":
        size = rest.pop(0)
    elif a == "--lang":
        lang = rest.pop(0)
    elif a == "--env":
        k, v = rest.pop(0).split("=", 1)
        env[k] = v
    else:
        snippet = open(a).read()
src = ROOT + "/crates/sumi-gui/qml"
tr = ROOT + "/crates/sumi-gui/tests/qml/Tr.qml"
fx = json.load(open(ROOT + "/fixtures/state-pg32ucdm3.json"))
def feat(i): return next(f for f in fx["features"] if f["id"] == i)
mon = [{"id": fx["id"], "model": fx["model"], "line": "gaming", "oled": True, "bus": 5, "fw": 103, "connector": "DP-3"}]
connected, readonly = True, False
config = {
    "app_sync": {"enabled": True, "restore_on_unfocus": False, "rule": []},
    "hotkeys": {"enabled": True, "step": 5}, "target_mode": {"strength": 44}, "gamepad": {"enabled": False},
    "mode_sync": False,
    "ai_visual": {"enabled": False, "notify": True, "modes": ["rts_rpg", "fps", "scenery", "racing", "srgb", "moba"], "interval_ms": 1000},
    "desktop_saved": {"panel_hiding": ["none"], "color_scheme": "BreezeLight"},
}
api = {
    "GetHostState": {"connector": "DP-3", "hdr": False, "target_mode": False},
    "GetDesktopPrefs": {"panel_autohide": False, "dark_theme": True},
    "Gamepads": [], "Hotkeys": {"error": "org.freedesktop.DBus.Error.ServiceUnknown: The name org.kde.kglobalaccel was not provided by any .service files"},
    "ListApps": [], "RecentWindows": [], "Problems": [],
    "TileLayoutNames": [[1, "Top / bottom"], [2, "Left / right"], [3, "Wide left, narrow right"], [4, "Narrow left, wide right"], [5, "Tall top, short bottom"], [6, "Short top, tall bottom"], [7, "Two left, one right"], [8, "One left, two right"], [9, "One top, two bottom"], [10, "Two top, one bottom"], [11, "Three columns"], [12, "Three rows"], [13, "Four quadrants"]],
    "GetTileLayouts": {"error": "KWin scripting (is this a Plasma 6 session?): org.freedesktop.DBus.Error.ServiceUnknown: The name org.kde.KWin was not provided by any .service files"},
}
if scenario.startswith("rich"):
    api["Gamepads"] = ["Xbox Wireless Controller"]
    labels = ["EZ OSD: open the monitor menu", "EZ OSD: up", "EZ OSD: down", "EZ OSD: left", "EZ OSD: right", "EZ OSD: enter", "EZ OSD: back", "EZ OSD: close the monitor menu",
              "Brightness +", "Brightness \u2212", "Contrast +", "Contrast \u2212", "Blue Light Filter +", "Blue Light Filter \u2212", "Shadow Boost +", "Shadow Boost \u2212", "Saturation +", "Saturation \u2212",
              "PIP/PBP on/off", "KVM: switch USB upstream", "Power saving on/off", "Next GameVisual mode", "Previous GameVisual mode", "Target Mode on/off", "HDR on/off"]
    api["Hotkeys"] = [{"action": "a%d" % i, "label": l, "keys": (["Meta+Alt+%d" % i] if i % 3 == 0 else [])} for i, l in enumerate(labels)]
    api["ListApps"] = [{"id": "org.kde.dolphin", "name": "Dolphin", "icon": "", "key": "dolphin"}, {"id": "steam_app_1172470", "name": "Apex Legends", "icon": "", "key": "steam_app_1172470"}, {"id": "org.mozilla.firefox", "name": "Firefox", "icon": "", "key": "firefox"}]
    api["RecentWindows"] = [{"class": "firefox", "desktop_file": "firefox", "exe": "firefox", "output": "DP-3"}, {"class": "steam_app_1172470", "desktop_file": "", "exe": "r5apex.exe", "output": "DP-3"}]
    api["GetTileLayouts"] = [{"output": "DP-3", "aspect": 1.78, "layout": 2, "tiles": 2}, {"output": "HDMI-A-1", "aspect": 1.78, "layout": 0, "tiles": 0}]
    config["app_sync"] = {"enabled": True, "default_mode": 2, "restore_on_unfocus": True, "rule": [{"app": "dolphin", "name": "Dolphin", "mode": 1}, {"app": "steam_app_1172470", "name": "Apex Legends", "mode": 2}, {"app": "firefox", "name": "Firefox", "mode": 7}]}
AI_MODEL = {"name": "vision_model.onnx", "url": "https://huggingface.co/x", "size": 352000000, "sha256": "00"}
def ai(installed, **state):
    st = {"running": installed, "monitors": {}}
    st.update(state)
    return {"installed": installed, "prompts": True, "model": AI_MODEL, "state": st}
api["AiStatus"] = ai(True)
if scenario in ("ai_ready",):
    config["ai_visual"].update({"enabled": True})
if scenario in ("hdr", "rich_hdr", "hdr_dv"):
    fx["context"]["hdr"] = True
    feat("hdr_mode")["value"] = 518 if scenario == "hdr_dv" else 258; feat("hdr_mode")["enabled"] = True
    feat("gamevisual")["enabled"] = True
    for i in ("brightness", "contrast", "shadow_boost", "saturation", "color_temp", "reset"): feat(i)["enabled"] = False
if scenario == "user":
    fx["context"]["mode"] = 4; feat("gamevisual")["value"] = 4
if scenario == "ja_osd":
    feat("language")["value"] = 6
os.makedirs(out, exist_ok=True)
for f in glob.glob(out + "/*.qml"): os.remove(f)
for f in glob.glob(src + "/*.qml"):
    shutil.copy(f, out)
shutil.copy(tr, out)
if lang:
    import subprocess
    cat = subprocess.run(["python3", HERE + "/po2json.py", ROOT + "/po/%s.po" % lang], capture_output=True, text=True, check=True).stdout
    t = open(out + "/Tr.qml").read().replace("property var catalogue: ({})", "property var catalogue: " + cat)
    open(out + "/Tr.qml", "w").write(t)
open(out + "/qmldir", "w").write("singleton Theme 1.0 Theme.qml\nsingleton Overlay 1.0 Overlay.qml\nsingleton IconData 1.0 IconData.qml\nsingleton Languages 1.0 Languages.qml\nsingleton Tr 1.0 Tr.qml\n")
for f in glob.glob(out + "/*.qml"):
    if f.endswith("/Tr.qml"): continue
    t = open(f).read()
    t = re.sub(r'(^|[^A-Za-z0-9_.])(i18n(?:c|p|cp)?)\(', r'\1Tr.\2(', t, flags=re.M)
    t = t.replace('source: root.name !== "" ? "image://icon/" + root.name : ""', 'source: ""')
    open(f, "w").write(t)
main = open(out + "/Main.qml").read().replace("import io.github.jenaridev.sumi\n", "")
if size:
    w, h = size.split("x")
    main = main.replace("width: Theme.designWidth\n    height: Theme.designHeight\n    minimumWidth", "width: %s\n    height: %s\n    minimumWidth" % (w, h), 1)
idx = main.rstrip().rfind("}")
main = main[:idx] + snippet + "\n}\n"
open(out + "/Main.qml", "w").write(main)
status = "sumid isn't running — start it with: systemctl --user start sumid" if not connected else ""
envs = {"SUMI_SCREENSHOT": png, "SUMI_PAGE": page}
envs.update(env)
fake = '''
import QtQuick
QtObject {
    id: fake
    property bool connected: false
    property bool readonly: %(readonly)s
    property string monitors: "[]"
    property string status: %(status)s
    property bool busy: false
    property string daemon_version: "0.1.0"
    property var state: %(state)s
    property var cfg: %(config)s
    property var api: %(api)s
    property var prefs: ({})
    signal applied(string id, string feature, int value, bool ok, string message)
    signal state_ready(string id, string pages, string json)
    signal monitor_changed(string id, string context)
    signal call_done(string tag, bool ok, string result)
    signal activate_requested()
    function later(ms, fn) { const t = Qt.createQmlObject('import QtQuick; Timer { repeat: false }', fake); t.interval = ms; t.triggered.connect(function() { fn(); t.destroy() }); t.start() }
    function refresh() { later(200, function() { fake.connected = %(connected)s; fake.status = %(status)s; const before = fake.monitors; fake.monitors = %(monitors)s; if (before === fake.monitors) fake.monitorsChanged() }) }
    function request_state(id, pages) { console.log("request_state", id, pages); later(300, function() { state_ready(id, pages, JSON.stringify(fake.state)) }) }
    function apply(id, feature, value) {
        console.log("apply", id, feature, value)
        later(150, function() {
            if (fake.readonly) { applied(id, feature, value, false, "sumid is running read-only"); return }
            const f = fake.state.features.find(x => x.id === feature)
            if (f) f.value = value
            applied(id, feature, value, true, feature + " = " + value)
        })
    }
    function call(method, argsJson, tag, notify) {
        const args = JSON.parse(argsJson)
        console.log("call", method, argsJson)
        later(80, function() {
            let r = fake.api[method]
            if (method === "GetConfig") r = fake.cfg
            if (method === "SetConfig") { fake.cfg = JSON.parse(args[0]); call_done(tag, true, ""); return }
            if (method === "GetVcp") r = [2, 40]
            if (method === "ListMonitors") r = JSON.parse(fake.monitors)
            if (r === undefined) { call_done(tag, false, "no reply for " + method); return }
            if (r && r.error) { call_done(tag, false, r.error); return }
            call_done(tag, true, typeof r === "string" ? r : JSON.stringify(r))
        })
    }
    function manual_url() { return "https://www.asus.com/support/" }
    function restart() { console.log("restart") }
    function tray_available() { return false }
    function start_events() {}
    function claim_instance() { return true }
    function env_var(name) { return (%(envs)s)[name] || "" }
    function copy_text(t) { console.log("copy_text:", t) }
    function app_version() { return "0.1.0" }
    function autostart_enabled() { return false }
    function set_autostart(on) { return true }
    function hidden_start() { return false }
    function pref(k) { return prefs[k] || "" }
    function set_pref(k, v) { prefs[k] = v; return true }
    function read_text_file(u) { return "" }
    function write_text_file(u, t) { return true }
    function open_system_settings(m) { return true }
}
''' % dict(readonly=str(readonly).lower(), status=json.dumps(status), state=json.dumps(fx), config=json.dumps(config), api=json.dumps(api),
           connected=str(connected).lower(), monitors=json.dumps(json.dumps(mon)), envs=json.dumps(envs))
open(out + "/Backend.qml", "w").write(fake)
