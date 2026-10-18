import QtQuick

// The host side of sumid: its configuration, the desktop state it can change
// (KDE HDR, Target Mode, panel auto-hide, dark theme) and a small wrapper over
// the backend's generic `call`. Everything here is asynchronous.
//
// The configuration is always read-modify-write on the whole object: the daemon
// keeps fields of its own in it (for example what it must restore when a desktop
// toggle is turned off), so a stale copy must never be written back.
Item {
    id: host

    property var backend: null

    property var config: ({})        // last known GetConfig
    property bool configLoaded: false
    property var hostStates: ({})    // monitor id -> {connector, hdr, target_mode}
    property var desktop: ({})       // {panel_autohide, dark_theme} (null = not available)
    property bool desktopLoaded: false

    property int seq: 0
    property var callbacks: ({})     // tag -> function(ok, result)
    property var configQueue: []     // mutators waiting for the next read-modify-write
    property bool configBusy: false

    visible: false

    // Call a sumid method; `args` is an array, `notify` shows errors in the toast.
    // `cb(ok, result)` gets the string result (or the error text).
    function call(method, args, notify, cb) {
        if (!backend) {
            return;
        }
        const tag = "c" + (++seq);
        if (cb) {
            callbacks[tag] = cb;
        }
        backend.call(method, JSON.stringify(args || []), tag, notify === true);
    }

    function json(text, fallback) {
        try {
            const v = JSON.parse(text);
            return v === null || v === undefined ? fallback : v;
        } catch (e) {
            return fallback;
        }
    }

    // ---- Configuration
    function loadConfig() {
        call("GetConfig", [], false, (ok, text) => {
            if (ok) {
                config = json(text, {});
                configLoaded = true;
            }
        });
    }

    // Change the configuration: the UI sees `mutator` applied at once; then the
    // current config is read, the mutator applied to it and the whole object saved.
    function updateConfig(mutator) {
        const local = JSON.parse(JSON.stringify(config));
        mutator(local);
        config = local;
        configQueue.push(mutator);
        runConfigQueue();
    }

    function runConfigQueue() {
        if (configBusy || configQueue.length === 0) {
            return;
        }
        configBusy = true;
        call("GetConfig", [], true, (ok, text) => {
            if (!ok) {
                configBusy = false;
                configQueue = [];
                return;
            }
            const fresh = json(text, null);
            const mutators = configQueue;
            configQueue = [];
            if (fresh === null) {
                configBusy = false;
                return;
            }
            for (const m of mutators) {
                m(fresh);
            }
            call("SetConfig", [JSON.stringify(fresh)], true, ok2 => {
                configBusy = false;
                if (ok2) {
                    config = fresh;
                } else {
                    configQueue = [];
                    loadConfig();   // show what the daemon really has
                }
                runConfigQueue();
            });
        });
    }

    // ---- Host state of a monitor
    function loadHostState(id) {
        if (id === "") {
            return;
        }
        call("GetHostState", [id], false, (ok, text) => {
            if (ok) {
                const m = Object.assign({}, hostStates);
                m[id] = json(text, {});
                hostStates = m;
            }
        });
    }

    function hostState(id) {
        return hostStates[id] || null;
    }

    function setHdr(id, on) {
        call("SetHdr", [id, on], true, () => loadHostState(id));
    }

    function setTargetMode(id, on) {
        call("SetTargetMode", [on], true, () => loadHostState(id));
    }

    // ---- AI Visual (the daemon's recogniser and its model)
    property var aiStatus: ({})
    property bool aiLoaded: false
    property string aiError: ""     // set when the daemon has no AiStatus (an older sumid)

    function loadAi() {
        call("AiStatus", [], false, (ok, text) => {
            if (ok) {
                aiStatus = json(text, {});
                aiLoaded = true;
                aiError = "";
            } else {
                aiError = text;
                aiLoaded = false;
            }
        });
    }

    // Preview while AI Visual is off: one read-only capture + classification
    // per call, smoothed by the daemon's preview engine; switches nothing.
    property var aiPreview: ({})
    function previewAi(id) {
        call("AiClassifyNow", [id], false, (ok, text) => {
            aiPreview = ok ? json(text, {}) : ({ error: text });
        });
    }

    function downloadAi() {
        call("DownloadAiModel", [], true, () => loadAi());
    }

    function removeAi(cb) {
        call("RemoveAiModel", [], true, ok => {
            loadAi();
            if (cb) {
                cb(ok);
            }
        });
    }

    // Remember the language of a monitor's menu (VCP 0xCC, one cheap read) for the
    // "Same as the monitor menu" choice.
    function cacheOsdLanguage(id) {
        if (id === "" || !backend) {
            return;
        }
        call("GetVcp", [id, 204], false, (ok, text) => {
            if (!ok) {
                return;
            }
            const locale = Languages.localeFor(json(text, [])[0]);
            if (locale !== "" && backend.pref("osd_locale") !== locale) {
                backend.set_pref("osd_locale", locale);
            }
        });
    }

    // ---- Desktop toggles
    function loadDesktop() {
        call("GetDesktopPrefs", [], false, (ok, text) => {
            if (ok) {
                desktop = json(text, {});
                desktopLoaded = true;
            }
        });
    }

    function setPanelAutohide(on) {
        call("SetPanelAutohide", [on], true, () => loadDesktop());
    }

    function setDarkTheme(on) {
        call("SetDarkTheme", [on], true, () => loadDesktop());
    }

    Connections {
        target: host.backend
        ignoreUnknownSignals: true
        function onCall_done(tag, ok, result) {
            const cb = host.callbacks[tag];
            if (cb) {
                delete host.callbacks[tag];
                cb(ok, result);
            }
        }
    }
}
