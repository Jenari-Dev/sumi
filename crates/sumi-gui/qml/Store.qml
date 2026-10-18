import QtQuick

// Monitor state, optimistic edits and the write gate shared by every page.
//
// Reads: `features` holds the last decoded state per feature id; `value()` and
// friends overlay the user's pending edits (`local`) on top of it.
//
// Writes go through `write()` and nothing else:
//  * never while the store is being populated from a daemon reply (`populating`),
//  * never when the daemon is read-only,
//  * one write per feature in flight; while it runs only the latest value is
//    kept and sent when the previous one returns ("coalescing"),
//  * anything with a risk goes through `commit()`, which asks first.
Item {
    id: store

    property var backend: null
    property string monitorId: ""

    property var features: ({})   // feature id -> decoded feature (last daemon state)
    property var local: ({})      // feature id -> value the user just set, not yet confirmed
    property var info: ({})       // {model, line, oled, firmware, bus, connector, hdr_format}
    property var context: ({})    // {mode, hdr, power_saving, user_color, blf_max}
    property var loaded: ({})     // pages csv -> true once its first state arrived
    property var failed: ({})     // pages csv -> true if the last request failed
    property string pageKey: ""   // pages csv of the visible page (what refreshes re-read)
    property bool populating: false
    property var confirm: null    // {title, text, ok, fn, ...} while a dialog is open (see ask / notice)
    property var monitors: []     // every attached monitor {id, model, ...} (set by Main)

    readonly property bool readOnly: backend ? backend.readonly === true : false

    // Writes that change which other controls are usable: re-read the page afterwards.
    readonly property var refreshAfter: ["gamevisual", "color_temp", "blue_light", "power_saving", "hdr_mode", "reset"]

    property var inflight: ({})   // feature id -> value being written
    property var pending: ({})    // feature id -> latest value waiting for the write in flight
    property var applied: ({})    // feature id -> last value known to be on the monitor
    property var seq: []          // queued {id, value} steps (run one after another)
    property var reqBusy: ({})
    property var reqAgain: ({})

    // ---- Reading ---------------------------------------------------------

    function feature(id) {
        return features[id] || null;
    }
    function isSupported(id) {
        const f = features[id];
        return !!f && f.supported === true;
    }
    function isEnabled(id) {
        const f = features[id];
        return !!f && f.supported === true && f.enabled === true;
    }
    function isExtra(id) {
        const f = features[id];
        return !!f && f.vendor_ui === false;
    }
    function value(id) {
        if (local[id] !== undefined) {
            return local[id];
        }
        const f = features[id];
        return f && f.value !== null && f.value !== undefined ? f.value : undefined;
    }
    function maxOf(id) {
        const f = features[id];
        return f && f.max ? f.max : 100;
    }
    function options(id) {
        const f = features[id];
        return f && f.options ? f.options : [];
    }
    function optionLabel(id, v) {
        const o = options(id).find(x => x.value === v);
        return o ? o.label : "";
    }
    // Labels that come from the daemon (feature options, hotkey names…) are shown through
    // i18n at display time: the "sumi" catalogue has them (po/daemon-strings.cpp). Logic keeps
    // using the raw labels. Language names are native names and stay as they are.
    function tr(label) {
        return label === undefined || label === "" ? "" : i18n(label);
    }
    function displayOptions(id) {
        return options(id).map(o => ({ value: o.value, label: id === "language" ? o.label : tr(o.label) }));
    }
    function optionText(id, v) {
        const o = options(id).find(x => x.value === v);
        return o ? (id === "language" ? o.label : tr(o.label)) : "";
    }
    function indexOfValue(id) {
        const v = value(id);
        return options(id).findIndex(o => o.value === v);
    }
    // True once every page of a csv has been read (by this request or an earlier one).
    function isLoaded(csv) {
        return loaded[csv] === true || csv.split(",").every(p => loaded[p] === true);
    }
    // Name of the HDR format the monitor advertises in its EDID ("" if unknown).
    function hdrFormatName() {
        switch (info.hdr_format) {
        case "hdr10": return "HDR10";
        case "dolby_vision": return "Dolby Vision";
        default: return "";
        }
    }
    function bit(id) {
        return value(id) === 1;
    }

    // ---- Daemon replies ---------------------------------------------------

    onMonitorIdChanged: reset()

    function reset() {
        features = {};
        local = {};
        info = {};
        context = {};
        loaded = {};
        failed = {};
        inflight = {};
        pending = {};
        applied = {};
        seq = [];
        reqBusy = {};
        reqAgain = {};
        if (confirm !== null && !confirm.notice) {
            confirm = null;
        }
        snapshot = null;
        lastEditAt = 0;
        compareState = "idle";
    }

    // Ask the daemon for a page's state (no-op while the same request is running).
    function request(csv) {
        if (!backend || monitorId === "") {
            return;
        }
        const key = csv === undefined ? pageKey : csv;
        if (reqBusy[key]) {
            reqAgain[key] = true;
            return;
        }
        reqBusy[key] = true;
        if (failed[key]) {
            const fl = Object.assign({}, failed);
            delete fl[key];
            failed = fl;
        }
        backend.request_state(monitorId, key);
    }

    function handleState(id, csv, json) {
        delete reqBusy[csv];
        if (id !== monitorId) {
            return;
        }
        if (json === "") {
            const fl = Object.assign({}, failed);
            fl[csv] = true;
            failed = fl;
            return;
        }
        ingest(csv, JSON.parse(json));
        if (reqAgain[csv]) {
            delete reqAgain[csv];
            request(csv);
        }
    }

    // Merge a decoded state into the store. Controls update from this; none of
    // that may write, hence `populating`.
    function ingest(csv, st) {
        populating = true;
        try {
            const f = Object.assign({}, features);
            const l = Object.assign({}, local);
            for (const x of st.features) {
                f[x.id] = x;
                if (!(x.id in inflight) && !(x.id in pending)) {
                    delete l[x.id];
                    if (x.value !== null && x.value !== undefined) {
                        applied[x.id] = x.value;
                    }
                }
            }
            features = f;
            local = l;
            noteOsdLanguage(f.language);
            info = { model: st.model, line: st.line, oled: st.oled, firmware: st.firmware, bus: st.bus,
                     connector: st.connector || "", hdr_format: st.hdr_format || "" };
            context = st.context || {};
            // The reply covers every page in the request: each of them counts as loaded.
            const ld = Object.assign({}, loaded);
            ld[csv] = true;
            for (const p of csv.split(",")) {
                if (p !== "") {
                    ld[p] = true;
                }
            }
            loaded = ld;
            const fl = Object.assign({}, failed);
            delete fl[csv];
            failed = fl;
        } finally {
            populating = false;
        }
    }

    // The language the monitor's menu is in decides Sumi's own language when the user
    // picked "Same as the monitor menu": remember the last one seen.
    function noteOsdLanguage(feature) {
        if (!backend || !feature || feature.value === null || feature.value === undefined) {
            return;
        }
        const locale = Languages.localeFor(feature.value);
        if (locale !== "" && backend.pref("osd_locale") !== locale) {
            backend.set_pref("osd_locale", locale);
        }
    }

    function handleApplied(id, feat, val, ok, message) {
        if (id !== monitorId) {
            return;
        }
        delete inflight[feat];
        if (!ok) {
            delete pending[feat];
            clearLocal(feat);
            seq = [];
            request();   // show what the monitor really has
            compareProgress();
            return;
        }
        applied[feat] = val;
        if (feat in pending) {
            const next = pending[feat];
            delete pending[feat];
            if (next !== val) {
                send(feat, next);
                return;
            }
        }
        if (refreshAfter.indexOf(feat) >= 0) {
            request();
        }
        if (seq.length > 0 && seq[0].id === feat) {
            const rest = seq.slice(1);
            seq = rest;
            runNext();
        }
        compareProgress();
    }

    function sameContext(a, b) {
        return a.mode === b.mode && a.hdr === b.hdr && a.power_saving === b.power_saving
            && a.user_color === b.user_color && a.blf_max === b.blf_max;
    }

    // The monitor changed something itself. When the usability context moved
    // (mode, HDR, power saving, colour temperature, blue light) re-read the visible
    // page. When only the brightness moved (the OSD, or our own slider), the new
    // value is in the signal: patch it in and spare the monitor a page read.
    function handleChanged(json) {
        let w = null;
        try {
            w = JSON.parse(json);
        } catch (e) {
            w = null;
        }
        const known = context.mode !== undefined;
        if (w && w.context && known && sameContext(w.context, context) && typeof w.brightness === "number" && "brightness" in features) {
            if (!("brightness" in inflight) && !("brightness" in pending)) {
                const f = Object.assign({}, features);
                f.brightness = Object.assign({}, f.brightness, { value: w.brightness });
                features = f;
                clearLocal("brightness");
                applied["brightness"] = w.brightness;
            }
            return;
        }
        refreshTimer.restart();
    }

    function clearLocal(id) {
        if (local[id] === undefined) {
            return;
        }
        const l = Object.assign({}, local);
        delete l[id];
        local = l;
    }

    function setLocal(id, v) {
        const l = Object.assign({}, local);
        l[id] = v;
        local = l;
    }

    // ---- Writing ----------------------------------------------------------

    function send(id, v) {
        inflight[id] = v;
        backend.apply(monitorId, id, v);
    }

    // The only way a value leaves the app. Returns true when it was sent or queued.
    function write(id, v) {
        if (populating || !backend || monitorId === "" || readOnly) {
            return false;
        }
        const f = features[id];
        const enc = f && f.encoding ? f.encoding.type : "";
        noteEdit(id);
        if (enc !== "command" && enc !== "trigger") {
            setLocal(id, v);
        }
        if (id in inflight) {
            if (inflight[id] === v) {
                delete pending[id];
            } else {
                pending[id] = v;
            }
            return true;
        }
        send(id, v);
        return true;
    }

    // Commit on release: make sure the final value of a drag is what gets written.
    function settle(id, v) {
        if (populating || readOnly) {
            return;
        }
        if (id in inflight) {
            if (inflight[id] === v) {
                delete pending[id];
            } else {
                pending[id] = v;
            }
            return;
        }
        if (applied[id] !== v) {
            write(id, v);
        }
    }

    function riskInfo(id, v) {
        const f = features[id];
        if (id === "gamevisual" && context.hdr === true) {
            // The daemon turns KDE HDR off first (the vendor does the same), then switches the mode.
            return { title: i18n("Turn HDR off"), ok: i18n("Switch"), text: i18n("Switching to %1 turns HDR off for this monitor.", optionText(id, v)) };
        }
        switch (f ? f.risk : "none") {
        case "resets":
            return { title: i18n("Reset mode"), ok: i18n("Reset"), text: i18n("Reset the current GameVisual mode to its defaults?") };
        case "panel_maintenance":
            return { title: i18n("Pixel cleaning"), ok: i18n("Start"), text: i18n("Pixel cleaning takes about 6 minutes. The screen turns off while it runs. Don't unplug the monitor.") };
        case "replug":
            if (id === "hdr_mode") {
                // Moving between HDR10 and Dolby Vision swaps the monitor's EDID (it reconnects);
                // another sub-mode of the same family does not.
                const cur = value(id);
                return cur !== undefined && (cur >> 8) === (v >> 8) ? null
                    : { title: i18n("Change HDR format"), ok: i18n("Switch"), text: i18n("Switching between HDR10 and Dolby Vision makes the monitor reconnect; the screen goes dark for a moment.") };
            }
            return v >= 5 && v <= 7 ? { title: i18n("Change layout"), ok: i18n("Apply"), text: i18n("The monitor will briefly disconnect while the layout changes.") } : null;
        case "moves_usb":
            return { title: i18n("Switch USB host"), ok: i18n("Switch"), text: i18n("Peripherals on the monitor's USB hub (keyboard, mouse…) will move to the other computer.") };
        case "switches_input":
            return { title: i18n("Switch input"), ok: i18n("Switch"), text: i18n("This PC will lose the picture until you switch back on the monitor.") };
        case "locks_features":
            return v === 1 ? { title: i18n("Power saving"), ok: i18n("Confirm"), text: i18n("Power saving mode locks most picture settings; only brightness and contrast stay adjustable.") } : null;
        default:
            return null;
        }
    }

    // Hot-key actions that do something a plain click would have to confirm (the Test button of
    // the HotKey page runs them for real). null for the harmless ones.
    function actionRisk(action) {
        switch (action) {
        case "kvm_toggle":
            return { title: i18n("Switch USB host"), ok: i18n("Switch"), text: i18n("Peripherals on the monitor's USB hub (keyboard, mouse…) will move to the other computer.") };
        case "power_saving_toggle":
            return { title: i18n("Power saving"), ok: i18n("Confirm"), text: i18n("Power saving mode locks most picture settings; only brightness and contrast stay adjustable.") };
        case "pip_toggle":
            return { title: i18n("Change layout"), ok: i18n("Apply"), text: i18n("The monitor will briefly disconnect while the layout changes.") };
        case "hdr_toggle":
            return { title: i18n("Change HDR"), ok: i18n("Switch"), text: i18n("Turning HDR on or off makes the monitor reconnect; the screen goes dark for a moment.") };
        default:
            return null;
        }
    }

    // User intent for anything but a slider: skip no-ops, confirm risky writes, then write.
    function commit(id, v) {
        if (populating || !backend || readOnly || !(id in features)) {
            return false;
        }
        if (value(id) === v) {
            return false;
        }
        const r = riskInfo(id, v);
        if (r) {
            ask(r.title, r.text, r.ok, () => write(id, v));
            return true;
        }
        return write(id, v);
    }

    // Writes in order, each after the previous one was confirmed (aborts on error).
    function runSequence(steps) {
        if (populating || !backend || readOnly) {
            return false;
        }
        seq = steps.slice();
        runNext();
        return true;
    }

    function runNext() {
        if (seq.length === 0) {
            return;
        }
        if (!write(seq[0].id, seq[0].value)) {
            seq = [];
        }
    }

    function ask(title, text, ok, fn) {
        confirm = { title: title, text: text, ok: ok || i18n("Confirm"), fn: fn, showCancel: true, large: false, notice: false };
    }

    // A dialog with one button (and no action), for information and logs.
    function notice(title, text, large) {
        confirm = { title: title, text: text, ok: i18n("OK"), fn: null, showCancel: false, large: large === true, notice: true };
    }

    function answer(yes) {
        const c = confirm;
        confirm = null;
        if (yes && c && c.fn) {
            c.fn();
        }
    }

    // ---- Compare: flip between the values from before the user's last edit and the current ones.
    // The snapshot is taken when the user starts changing something (an edit more than 1.5 s after
    // the previous one); while Compare is held the snapshot values are applied, on release the
    // current ones again (only features that differ).
    readonly property var compareIds: ["brightness", "contrast", "overdrive", "blue_light", "shadow_boost", "ascr",
        "saturation", "hue", "color_temp", "red_gain", "green_gain", "blue_gain", "gamma", "sharpness"]
    property var snapshot: null           // feature id -> value, or null
    property double lastEditAt: 0
    property string compareState: "idle"  // idle | showing | releasing | restoring
    property var compareCurrent: ({})

    function captureValues() {
        const m = {};
        for (const id of compareIds) {
            const v = value(id);
            if (v !== undefined && isSupported(id)) {
                m[id] = v;
            }
        }
        return m;
    }

    function noteEdit(id) {
        if (compareState !== "idle") {
            return;
        }
        if (id === "gamevisual" || id === "hdr_mode") {
            snapshot = null;   // another mode: the old values mean nothing
            lastEditAt = 0;
            return;
        }
        const now = Date.now();
        if (snapshot === null || now - lastEditAt > 1500) {
            snapshot = captureValues();
        }
        lastEditAt = now;
    }

    function idle() {
        return seq.length === 0 && Object.keys(inflight).length === 0;
    }

    function compareStart() {
        if (compareState !== "idle" || snapshot === null || populating || readOnly || !backend || context.hdr === true) {
            return false;
        }
        const cur = captureValues();
        const steps = [];
        for (const id of compareIds) {
            if (id in snapshot && id in cur && snapshot[id] !== cur[id] && isEnabled(id)) {
                steps.push({ id: id, value: snapshot[id] });
            }
        }
        if (steps.length === 0) {
            return false;
        }
        compareCurrent = cur;
        compareState = "showing";
        runSequence(steps);
        return true;
    }

    function compareEnd() {
        if (compareState !== "showing") {
            return;
        }
        compareState = "releasing";
        compareProgress();
    }

    // Move the Compare state machine on once the writes of the current step are done.
    function compareProgress() {
        if (compareState === "releasing" && idle()) {
            const steps = [];
            for (const id of compareIds) {
                if (id in compareCurrent && value(id) !== compareCurrent[id] && isEnabled(id)) {
                    steps.push({ id: id, value: compareCurrent[id] });
                }
            }
            if (steps.length === 0) {
                compareState = "idle";
            } else {
                compareState = "restoring";
                runSequence(steps);
            }
        } else if (compareState === "restoring" && idle()) {
            compareState = "idle";
        }
    }

    Timer {
        id: refreshTimer
        interval: 400
        onTriggered: store.request()
    }

    Connections {
        target: store.backend
        ignoreUnknownSignals: true
        function onApplied(id, feature, value, ok, message) {
            store.handleApplied(id, feature, value, ok, message);
        }
        function onState_ready(id, pages, json) {
            store.handleState(id, pages, json);
        }
        function onMonitor_changed(id, context) {
            if (id === store.monitorId) {
                store.handleChanged(context);
            }
        }
    }
}
