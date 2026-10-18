import QtQuick

// What the AI Visual bubble shows, and when. No windows here: this watches the daemon's
// AiStatus and keeps, per monitor, the facts the overlay draws.
//
// - Polling: AiStatus about once a second, only while `ai_visual.enabled` and
//   `ai_visual.notify` are both on (also while the main window sits in the tray), and
//   never otherwise. When the AI panel is open it polls too; its answers are used
//   instead of asking twice.
// - A switch is `last_sent` changing to a new, non-empty mode after the first poll. The
//   mode found by the first poll is only remembered, never announced.
// - A monitor gets an orb when AI Visual runs on it (it has an entry in the daemon's
//   state) and its connector (DP-3 ...) is known from the monitor list.
//
// Test hook: with SUMI_AI_FAKE=<file> the status, and optionally the AI config and the
// monitor list, come from a JSON file read on every poll instead of the daemon:
//   {"config": {"enabled": true, "notify": true},
//    "monitors": [{"id": "...", "connector": "DP-3"}],
//    "status": { ...what AiStatus returns... }}
Item {
    id: model

    property var host: null
    property var backend: null
    property var monitors: []          // [{id, connector, ...}] from ListMonitors
    property int pollInterval: 1000
    property string fakePath: ""

    visible: false

    // ---- Config
    property var fake: null
    readonly property var ai: fake !== null && fake.config ? fake.config
        : (host && host.config && host.config.ai_visual ? host.config.ai_visual : ({}))
    readonly property bool active: ai.enabled === true && ai.notify !== false
    readonly property var monitorList: fake !== null && fake.monitors ? fake.monitors : monitors

    // ---- What the overlay draws
    property var targetIds: []         // monitors with an orb, in a stable order
    property var states: ({})          // id -> {id, connector, paused, mode, switchSeq, analyseSeq}
    property var dismissed: ({})       // id -> true until the next switch (double-click)
    property var positions: ({})       // connector -> {right, bottom}

    signal switched(string id, string mode)

    // ---- Layer-shell or not (see probeShell)
    property url layerSource: Qt.resolvedUrl("AiLayerWindow.qml")
    property url plainSource: Qt.resolvedUrl("AiWindow.qml")
    property bool layerShell: false
    property var shell: null           // the Component the overlay windows are made from

    // ---- Internals
    property var seen: ({})            // id -> {mode, verdict} as of the last poll
    property var counters: ({})        // id -> {sw, an}
    property var lastMonitors: ({})    // id -> the daemon's entry of the last poll
    property bool seeded: false
    property double lastStatusAt: 0
    property bool warned: false

    function stateOf(id) {
        return states[id] || null;
    }

    function shown(id) {
        return active && states[id] !== undefined && dismissed[id] !== true;
    }

    // The Qt screen of a connector (QScreen.name is the output name under Wayland), or null.
    function screenFor(connector) {
        if (screenMap[connector]) {
            return screenMap[connector];
        }
        const all = Qt.application.screens;
        for (let i = 0; i < all.length; i++) {
            if (all[i].name === connector) {
                return all[i];
            }
        }
        return null;
    }

    property var screenMap: ({})       // connector -> QScreen, for tests (Qt's offscreen screen has no name)
    property var warnedScreens: ({})
    function noteMissingScreen(connector) {
        if (!warnedScreens[connector]) {
            warnedScreens[connector] = true;
            console.warn("Sumi: AI bubble: no screen named " + connector + " (Qt sees: "
                + Qt.application.screens.map(s => s.name).join(", ") + ")");
        }
    }

    function connectorOf(id) {
        const m = monitorList.find(x => x.id === id);
        return m && m.connector ? String(m.connector) : "";
    }

    // ---- Names
    function modeLabel(key) {
        switch (key) {
        case "rts_rpg": return i18n("RTS/RPG");
        case "fps": return i18n("FPS");
        case "cinema": return i18n("Cinema");
        case "scenery": return i18n("Scenery");
        case "racing": return i18n("Racing");
        case "srgb": return i18n("sRGB");
        case "moba": return i18n("MOBA");
        case "user": return i18n("User");
        default: return String(key);
        }
    }

    function message(key) {
        return i18n("Switched to %1 mode", modeLabel(key));
    }

    // ---- Polling
    function poll() {
        if (fakePath !== "") {
            readFake();
            if (active && fake !== null) {
                ingest(fake.status || fake);
            }
            return;
        }
        if (!active || !host) {
            return;
        }
        // The AI panel polls as well; its answer is as good as ours.
        if (Date.now() - lastStatusAt < pollInterval * 0.8) {
            return;
        }
        host.loadAi();
    }

    function readFake() {
        const text = backend ? String(backend.read_text_file("file://" + fakePath)) : "";
        if (text === "") {
            return;
        }
        try {
            fake = JSON.parse(text);
        } catch (e) {
            console.warn("Sumi: SUMI_AI_FAKE is not valid JSON:", e);
        }
    }

    Timer {
        interval: model.pollInterval
        repeat: true
        triggeredOnStart: true
        running: model.fakePath !== "" || model.active
        onTriggered: model.poll()
    }

    Connections {
        target: model.host
        ignoreUnknownSignals: true
        function onAiStatusChanged() {
            if (model.fakePath === "" && model.active) {
                model.ingest(model.host.aiStatus);
            }
        }
    }

    onActiveChanged: {
        if (!active) {
            reset();
        }
    }
    onMonitorListChanged: publish()

    function reset() {
        seen = {};
        counters = {};
        lastMonitors = {};
        seeded = false;
        dismissed = {};
        lastStatusAt = 0;
        publish();
    }

    // ---- One AiStatus answer
    function ingest(status) {
        if (!active) {
            return;
        }
        lastStatusAt = Date.now();
        const st = status && status.state ? status.state : {};
        const mons = st.running === false ? {} : (st.monitors || {});
        const announce = [];
        const nextSeen = Object.assign({}, seen);
        const nextCounters = Object.assign({}, counters);
        let nextDismissed = dismissed;
        for (const id of Object.keys(mons)) {
            const m = mons[id] || {};
            const before = seen[id];
            const mode = m.last_sent ? String(m.last_sent) : "";
            const verdict = m.verdict ? JSON.stringify(m.verdict) : "";
            const c = Object.assign({ sw: 0, an: 0 }, counters[id]);
            const paused = m.paused ? String(m.paused) : "";
            if (seeded && !paused) {
                if (before && before.verdict !== verdict) {
                    c.an++;
                }
                if (mode !== "" && mode !== (before ? before.mode : "")) {
                    c.sw++;
                    announce.push({ id: id, mode: mode });
                    if (nextDismissed[id]) {
                        nextDismissed = Object.assign({}, nextDismissed);
                        delete nextDismissed[id];
                    }
                }
            }
            nextSeen[id] = { mode: mode !== "" ? mode : (before ? before.mode : ""), verdict: verdict };
            nextCounters[id] = c;
        }
        seen = nextSeen;
        counters = nextCounters;
        lastMonitors = mons;
        seeded = true;
        dismissed = nextDismissed;
        publish();
        for (const a of announce) {
            if (connectorOf(a.id) !== "") {
                switched(a.id, a.mode);
            }
        }
    }

    // Targets and per-monitor state from the last poll and the current monitor list.
    function publish() {
        const ids = [];
        const next = {};
        if (active) {
            for (const id of Object.keys(lastMonitors)) {
                const connector = connectorOf(id);
                if (connector === "") {
                    continue;
                }
                const m = lastMonitors[id] || {};
                const c = counters[id] || { sw: 0, an: 0 };
                ids.push(id);
                next[id] = {
                    id: id,
                    connector: connector,
                    paused: m.paused ? String(m.paused) : "",
                    mode: seen[id] ? seen[id].mode : "",
                    switchSeq: c.sw,
                    analyseSeq: c.an
                };
            }
        }
        ids.sort();
        if (JSON.stringify(ids) !== JSON.stringify(targetIds)) {
            targetIds = ids;
        }
        if (JSON.stringify(next) !== JSON.stringify(states)) {
            states = next;
        }
    }

    // ---- Double-click: hide until the next switch
    function dismiss(id) {
        const d = Object.assign({}, dismissed);
        d[id] = true;
        dismissed = d;
    }

    // ---- Where the orb sits: pixels from the right and bottom edge of its screen, per connector.
    readonly property var defaultPosition: ({ right: 32, bottom: 96 })

    function position(connector) {
        const p = positions[connector];
        return p ? p : defaultPosition;
    }

    function setPosition(connector, right, bottom) {
        const p = Object.assign({}, positions);
        p[connector] = { right: Math.round(right), bottom: Math.round(bottom) };
        positions = p;
        if (backend) {
            backend.set_pref("ai_bubble_pos", JSON.stringify(p));
        }
    }

    function loadPositions() {
        let p = {};
        try {
            p = JSON.parse(backend ? String(backend.pref("ai_bubble_pos")) || "{}" : "{}");
        } catch (e) {
            p = {};
        }
        const clean = {};
        for (const k of Object.keys(p || {})) {
            const v = p[k];
            if (v && Number.isFinite(v.right) && Number.isFinite(v.bottom)) {
                clean[k] = { right: Math.max(0, v.right), bottom: Math.max(0, v.bottom) };
            }
        }
        positions = clean;
    }

    // ---- Layer-shell: the module is KDE's; without it (or on X11) the overlay falls back to a
    // plain frameless stay-on-top tool window. Said once.
    function probeShell() {
        const layer = Qt.createComponent(layerSource);
        if (layer.status === Component.Ready) {
            shell = layer;
            layerShell = true;
            return;
        }
        if (!warned) {
            warned = true;
            console.warn("Sumi: AI bubble: no layer-shell (" + String(layer.errorString()).trim().split("\n")[0]
                + "); using a plain stay-on-top window");
        }
        layerShell = false;
        shell = Qt.createComponent(plainSource);
    }

    Component.onCompleted: {
        loadPositions();
        probeShell();
    }
}
