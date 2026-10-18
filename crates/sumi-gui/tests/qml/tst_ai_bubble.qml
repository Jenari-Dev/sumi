import QtQuick
import QtTest

// AI Visual's on-screen bubble: when it shows, when it announces a switch, what it says, where it
// sits, when it polls, and what happens without KDE's layer-shell module. No daemon, no compositor:
// the model is fed AiStatus-shaped objects; the windows (plain ones) open on the offscreen screen.
Item {
    id: root
    width: 400
    height: 300

    // What the app's Backend offers the bubble.
    QtObject {
        id: bk
        property bool readonly: false
        property var prefs: ({})
        property var prefWrites: []
        property var files: ({})
        property var hostCalls: []
        property var responses: ({})
        signal call_done(string tag, bool ok, string result)
        function pref(k) {
            return prefs[k] || "";
        }
        function set_pref(k, v) {
            prefs[k] = v;
            prefWrites.push([k, v]);
            return true;
        }
        function env_var(n) {
            return "";
        }
        function read_text_file(url) {
            return files[url] || "";
        }
        function call(method, argsJson, tag, notify) {
            hostCalls.push(method);
            const r = responses[method];
            call_done(tag, r !== undefined, r !== undefined ? r : "no reply");
        }
        function count(method) {
            return hostCalls.filter(m => m === method).length;
        }
    }

    Host {
        id: host
        backend: bk
    }

    // A model on its own (no daemon): config and monitors come from `fake` data.
    Component {
        id: modelComponent
        AiBubbleModel {
            backend: bk
            pollInterval: 100000
            property var switches: []
            onSwitched: (id, mode) => switches.push([id, mode])
        }
    }

    Component {
        id: bubbleComponent
        AiBubble {
            backend: bk
        }
    }

    readonly property string id1: "PG32UCDM3@i2c-5"
    // Qt's offscreen screen has no name, so the tests tell the model which screen "TEST-1" is.
    readonly property string screenName: "TEST-1"

    function config(over) {
        return Object.assign({ enabled: true, notify: true }, over || {});
    }

    function monitorList(connector) {
        return [{ id: id1, connector: connector === undefined ? "DP-3" : connector }];
    }

    function entry(last, extra) {
        return Object.assign({
            verdict: { scores: [[last || "fps", 61.2], ["office", 22.5]], is_game: 0.8, confidence: 0.7, top: last || "fps", target: last || "fps" },
            last_sent: last || null
        }, extra || {});
    }

    function status(entries, running) {
        return { installed: true, prompts: true, model: {}, state: { running: running !== false, monitors: entries } };
    }

    function one(last, extra) {
        const m = {};
        m[id1] = entry(last, extra);
        return status(m);
    }

    // A model fed by hand; `switches` collects what it announced.
    function standalone(cfg, connector) {
        const m = tc.createTemporaryObject(modelComponent, root, { fake: { config: cfg === undefined ? config() : cfg, monitors: monitorList(connector) } });
        return m;
    }

    TestCase {
        id: tc
        name: "AiBubble"
        when: windowShown

        function init() {
            bk.prefs = {};
            bk.prefWrites = [];
            bk.files = {};
            bk.hostCalls = [];
            bk.responses = {};
            Tr.catalogue = {};
            host.config = {};
            host.aiStatus = {};
        }

        // ---- Visibility ------------------------------------------------------------------
        function test_orb_for_a_monitor_ai_runs_on() {
            const m = standalone();
            verify(m.active);
            compare(m.targetIds.length, 0, "nothing before the first poll");
            m.ingest(one("fps"));
            compare(m.targetIds, [id1]);
            compare(m.stateOf(id1).connector, "DP-3");
            verify(m.shown(id1));
        }

        function test_hidden_when_ai_visual_is_off() {
            const m = standalone(config({ enabled: false }));
            verify(!m.active);
            m.ingest(one("fps"));
            compare(m.targetIds.length, 0);
            verify(!m.shown(id1));
        }

        function test_hidden_when_show_notifications_is_off() {
            const m = standalone(config({ notify: false }));
            verify(!m.active);
            m.ingest(one("fps"));
            compare(m.targetIds.length, 0);
        }

        function test_notifications_default_to_on() {
            const m = standalone({ enabled: true });
            verify(m.active, "ai_visual.notify absent counts as on, like the daemon's default");
        }

        function test_turning_the_config_off_removes_the_orb() {
            const m = standalone();
            m.ingest(one("fps"));
            compare(m.targetIds.length, 1);
            m.fake = { config: config({ notify: false }), monitors: monitorList() };
            compare(m.targetIds.length, 0);
            compare(m.states, {});
        }

        function test_hidden_when_no_monitor_runs_ai() {
            const m = standalone();
            m.ingest(status({}));
            compare(m.targetIds.length, 0);
            m.ingest(one("fps", {}));
            m.ingest(status({}));
            compare(m.targetIds.length, 0, "the daemon dropped the monitor: the orb goes");
        }

        function test_hidden_while_the_daemon_is_not_running_ai() {
            const m = standalone();
            const mons = {};
            mons[id1] = entry("fps");
            m.ingest(status(mons, false));
            compare(m.targetIds.length, 0);
        }

        function test_monitor_without_a_connector_gets_no_orb_until_it_is_known() {
            const m = standalone(config(), "");
            m.ingest(one("fps"));
            compare(m.targetIds.length, 0);
            m.fake = { config: config(), monitors: monitorList("HDMI-A-1") };
            compare(m.targetIds, [id1]);
            compare(m.stateOf(id1).connector, "HDMI-A-1");
        }

        function test_two_monitors_two_orbs() {
            const m = createTemporaryObject(modelComponent, root, { fake: { config: config(), monitors: [{ id: "A", connector: "DP-3" }, { id: "B", connector: "DP-1" }] } });
            m.ingest(status({ A: entry("fps"), B: entry("srgb") }));
            compare(m.targetIds, ["A", "B"]);
            compare(m.stateOf("B").connector, "DP-1");
        }

        function test_paused_dims_and_is_reported() {
            const m = standalone();
            m.ingest(one("fps"));
            compare(m.stateOf(id1).paused, "");
            m.ingest(one("fps", { paused: "HDR is on" }));
            compare(m.stateOf(id1).paused, "HDR is on");
            compare(m.targetIds.length, 1, "a paused monitor keeps its (dimmed) orb");
            m.ingest(one("fps"));
            compare(m.stateOf(id1).paused, "");
        }

        // ---- Switch detection ------------------------------------------------------------
        function test_the_mode_found_at_startup_is_not_announced() {
            const m = standalone();
            m.ingest(one("fps"));
            compare(m.switches.length, 0);
            m.ingest(one("fps"));
            compare(m.switches.length, 0, "unchanged: still nothing");
        }

        function test_a_change_is_announced_once() {
            const m = standalone();
            m.ingest(one("fps"));
            m.ingest(one("racing"));
            compare(m.switches, [[id1, "racing"]]);
            compare(m.stateOf(id1).switchSeq, 1);
            compare(m.stateOf(id1).mode, "racing");
            m.ingest(one("racing"));
            m.ingest(one("racing"));
            compare(m.switches.length, 1, "the same mode again is not a new switch");
            m.ingest(one("fps"));
            compare(m.switches.length, 2);
            compare(m.switches[1], [id1, "fps"]);
        }

        function test_first_mode_after_an_empty_start_is_a_switch() {
            const m = standalone();
            m.ingest(one(null));
            m.ingest(one("scenery"));
            compare(m.switches, [[id1, "scenery"]], "last_sent was empty at the first poll, then AI Visual sent a mode");
        }

        function test_a_gap_in_last_sent_does_not_repeat_the_announcement() {
            const m = standalone();
            m.ingest(one("fps"));
            m.ingest(one(null));
            m.ingest(one("fps"));
            compare(m.switches.length, 0);
        }

        function test_switch_while_paused_is_not_announced() {
            const m = standalone();
            m.ingest(one("fps"));
            m.ingest(one("racing", { paused: "screen locked" }));
            compare(m.switches.length, 0);
            m.ingest(one("racing"));
            compare(m.switches.length, 0, "and not afterwards either");
        }

        function test_a_new_baseline_after_the_config_was_switched_off_and_on() {
            const m = standalone();
            m.ingest(one("fps"));
            m.fake = { config: config({ enabled: false }), monitors: monitorList() };
            m.fake = { config: config(), monitors: monitorList() };
            m.ingest(one("racing"));
            compare(m.switches.length, 0, "the first poll after turning it on is the baseline again");
        }

        function test_only_a_changed_verdict_spins_the_orb() {
            const m = standalone();
            m.ingest(one("fps"));
            compare(m.stateOf(id1).analyseSeq, 0, "first poll: nothing");
            m.ingest(one("fps"));
            compare(m.stateOf(id1).analyseSeq, 0, "same verdict");
            const e = entry("fps");
            e.verdict.scores = [["fps", 40], ["racing", 38]];
            const mons = {};
            mons[id1] = e;
            m.ingest(status(mons));
            compare(m.stateOf(id1).analyseSeq, 1);
        }

        function test_double_click_hides_until_the_next_switch() {
            const m = standalone();
            m.ingest(one("fps"));
            m.dismiss(id1);
            verify(!m.shown(id1));
            m.ingest(one("fps"));
            verify(!m.shown(id1), "still hidden while nothing switches");
            m.ingest(one("moba"));
            verify(m.shown(id1), "a switch brings it back");
        }

        // ---- Message text ----------------------------------------------------------------
        function test_message_text_and_catalogue() {
            const m = standalone();
            compare(m.message("fps"), "Switched to FPS mode");
            compare(m.message("rts_rpg"), "Switched to RTS/RPG mode");
            compare(m.message("srgb"), "Switched to sRGB mode");
            Tr.catalogue = { "Switched to %1 mode": "%1モードに切り替えました", "Racing": "レーシング", "RTS/RPG": "RTS/RPG" };
            compare(m.message("racing"), "レーシングモードに切り替えました");
            compare(m.modeLabel("cinema"), "Cinema", "names without a translation stay English");
            compare(m.modeLabel("rts_rpg"), "RTS/RPG");
            compare(m.modeLabel("mystery"), "mystery", "a mode this app doesn't know is shown as the daemon sent it");
        }

        // ---- Position --------------------------------------------------------------------
        function test_default_position_is_bottom_right() {
            const m = standalone();
            compare(m.position("DP-3"), { right: 32, bottom: 96 });
        }

        function test_position_is_saved_per_connector_in_the_app_prefs() {
            const m = standalone();
            m.setPosition("DP-3", 172.4, 232);
            m.setPosition("HDMI-A-1", 10, 20);
            compare(bk.prefWrites.length, 2);
            compare(bk.prefWrites[1][0], "ai_bubble_pos");
            compare(JSON.parse(bk.prefs.ai_bubble_pos), { "DP-3": { right: 172, bottom: 232 }, "HDMI-A-1": { right: 10, bottom: 20 } });
            compare(m.position("DP-3"), { right: 172, bottom: 232 });
        }

        function test_position_is_restored_by_a_new_model() {
            standalone().setPosition("DP-3", 172, 232);
            const m = standalone();
            compare(m.position("DP-3"), { right: 172, bottom: 232 });
            compare(m.position("DP-1"), { right: 32, bottom: 96 }, "another connector keeps the default");
        }

        function test_bad_saved_positions_are_ignored() {
            bk.prefs.ai_bubble_pos = "{not json";
            compare(standalone().position("DP-3"), { right: 32, bottom: 96 });
            bk.prefs.ai_bubble_pos = JSON.stringify({ "DP-3": { right: "x", bottom: 5 }, "DP-1": { right: -40, bottom: 5 } });
            const m = standalone();
            compare(m.position("DP-3"), { right: 32, bottom: 96 });
            compare(m.position("DP-1"), { right: 0, bottom: 5 }, "negative margins are clamped");
        }

        // ---- Polling ---------------------------------------------------------------------
        function test_polls_only_while_enabled_and_notify() {
            host.config = { ai_visual: { enabled: false, notify: true } };
            bk.responses.AiStatus = JSON.stringify(one("fps"));
            const m = createTemporaryObject(modelComponent, root, { host: host, pollInterval: 50 });
            wait(200);
            compare(bk.count("AiStatus"), 0, "off: no polling at all");
            host.config = { ai_visual: { enabled: true, notify: true } };
            tryVerify(() => bk.count("AiStatus") >= 1, 300, "on: the first poll comes at once");
            wait(260);
            verify(bk.count("AiStatus") >= 4, "on: keeps polling, got " + bk.count("AiStatus"));
            host.config = { ai_visual: { enabled: true, notify: false } };
            const n = bk.count("AiStatus");
            wait(200);
            compare(bk.count("AiStatus"), n, "notify off: polling stops");
            host.config = { ai_visual: { enabled: true, notify: true } };
            wait(120);
            verify(bk.count("AiStatus") > n, "notify on again: polling resumes");
            host.config = { ai_visual: { enabled: false, notify: true } };
            const k = bk.count("AiStatus");
            wait(200);
            compare(bk.count("AiStatus"), k, "AI off: polling stops");
        }

        function test_polling_feeds_the_model_and_uses_the_connector_list() {
            host.config = { ai_visual: { enabled: true, notify: true } };
            bk.responses.AiStatus = JSON.stringify(one("fps"));
            const m = createTemporaryObject(modelComponent, root, { host: host, pollInterval: 50, monitors: monitorList("DP-1") });
            tryCompare(m, "targetIds", [id1]);
            compare(m.stateOf(id1).connector, "DP-1");
            bk.responses.AiStatus = JSON.stringify(one("racing"));
            tryVerify(() => m.stateOf(id1).mode === "racing");
            compare(m.stateOf(id1).switchSeq, 1);
        }

        function test_an_answer_someone_else_asked_for_is_not_asked_for_twice() {
            host.config = { ai_visual: { enabled: true, notify: true } };
            bk.responses.AiStatus = JSON.stringify(one("fps"));
            const m = createTemporaryObject(modelComponent, root, { host: host, pollInterval: 200, monitors: monitorList() });
            wait(20);
            const n = bk.count("AiStatus");
            // The AI panel polls on its own schedule; its answers count.
            for (let i = 0; i < 4; i++) {
                wait(100);
                host.loadAi();
            }
            compare(bk.count("AiStatus"), n + 4, "the bubble itself asked for nothing while the panel was polling");
        }

        function test_fake_file_hook() {
            bk.files["file:///tmp/ai-fake.json"] = JSON.stringify({ config: config(), monitors: monitorList("DP-2"), status: one("fps") });
            const m = createTemporaryObject(modelComponent, root, { fakePath: "/tmp/ai-fake.json", pollInterval: 50 });
            tryCompare(m, "targetIds", [id1]);
            compare(m.stateOf(id1).connector, "DP-2");
            compare(bk.count("AiStatus"), 0, "the daemon is not asked");
            bk.files["file:///tmp/ai-fake.json"] = JSON.stringify({ config: config(), monitors: monitorList("DP-2"), status: one("moba") });
            tryVerify(() => m.stateOf(id1).mode === "moba");
            compare(m.stateOf(id1).switchSeq, 1);
        }

        // ---- Layer-shell or not ----------------------------------------------------------
        function test_without_the_layer_shell_module_it_falls_back_to_a_plain_window() {
            const m = createTemporaryObject(modelComponent, root, { layerSource: Qt.resolvedUrl("NoLayerShell.qml") });
            verify(m.warned, "said once");
            verify(!m.layerShell);
            verify(m.shell !== null && m.shell.status === Component.Ready, "the plain window component is ready");
            const w = m.shell.createObject(root, { content: null });
            verify(w !== null);
            compare(w.placeByPosition, true, "placed by x / y");
            verify((w.flags & Qt.WindowStaysOnTopHint) !== 0);
            verify((w.flags & Qt.FramelessWindowHint) !== 0);
            w.destroy();
        }

        function test_with_the_module_it_uses_the_layer_surface() {
            const available = Qt.createComponent(Qt.resolvedUrl("AiLayerWindow.qml")).status === Component.Ready;
            const m = createTemporaryObject(modelComponent, root, {});
            compare(m.layerShell, available);
            if (!available) {
                skip("org.kde.layershell is not installed here");
            }
            const w = m.shell.createObject(root, {});
            compare(w.placeByPosition, false, "placed by layer-shell margins");
            w.destroy();
        }

        // ---- The windows -----------------------------------------------------------------
        // The whole chain on the plain fallback: model -> overlay -> two windows.
        function test_orb_and_message_windows() {
            const b = createTemporaryObject(bubbleComponent, root, { host: null, monitors: monitorList(screenName) });
            b.model.screenMap = { "TEST-1": Qt.application.screens[0] };
            b.model.layerSource = Qt.resolvedUrl("NoLayerShell.qml");
            b.model.probeShell();
            b.model.fake = { config: config(), monitors: monitorList(screenName) };
            b.model.ingest(one("fps"));
            const ov = findOverlay(b);
            verify(ov !== null, "an overlay for the monitor");
            tryVerify(() => ov.orbItem !== null);
            verify(ov.orbItem.visible);
            const orbWindow = ov.children.filter(c => c.objectName === "orb_window")[0].item;
            const msgWindow = ov.children.filter(c => c.objectName === "message_window")[0].item;
            tryVerify(() => orbWindow.visible, 2000, "the orb window is up");
            verify(!msgWindow.visible, "no message window before a switch");
            compare(orbWindow.width, 96);
            compare(orbWindow.height, 96);
            verify((msgWindow.flags & Qt.WindowTransparentForInput) !== 0, "the message window takes no input");
            verify((orbWindow.flags & Qt.WindowTransparentForInput) === 0, "the orb window takes the mouse");
            // The plain window is placed by x / y from the screen's bottom-right corner.
            const s = Qt.application.screens[0];
            compare(orbWindow.x, s.virtualX + s.width - 96 - 32);
            compare(orbWindow.y, s.virtualY + s.height - 96 - 96);

            // A switch: red pulse and the message.
            Tr.catalogue = { "Switched to %1 mode": "Cambiado a %1" };
            b.model.ingest(one("racing"));
            tryVerify(() => ov.orbItem.glowing, 500, "pulse");
            tryVerify(() => msgWindow.visible, 2000, "the message window opens");
            compare(ov.messageItem.text, "Cambiado a Racing");
            verify(ov.messageItem.shown);
            // It sits left of the orb, vertically centred.
            compare(msgWindow.height, 44);
            compare(ov.messageBottom, 96 + 26);
            compare(ov.messageRight, 32 + 96 + 8);

            // Paused: dimmed, and a switch shows no message.
            ov.messageItem.hideNow();
            b.model.ingest(one("racing", { paused: "HDR is on" }));
            tryCompare(ov.orbItem, "opacity", 0.4);
            ov.announce("fps");
            verify(!ov.messageItem.shown, "no message while paused");

            // Double-click: hidden until the next switch.
            b.model.ingest(one("racing"));
            ov.orbItem.children.filter(c => c.objectName === "orb_drag")[0].hideRequested();
            verify(!b.model.shown(id1));
            tryVerify(() => ov.orbItem === null, 2000, "the orb window is gone");
            b.model.ingest(one("fps"));
            tryVerify(() => ov.orbItem !== null, 2000, "back at the next switch");
        }

        function findOverlay(b) {
            for (let i = 0; i < b.children.length; i++) {
                if (b.children[i].targetId === id1) {
                    return b.children[i];
                }
            }
            return null;
        }

        function test_dragging_moves_the_orb_and_saves_the_place_when_released() {
            const b = createTemporaryObject(bubbleComponent, root, { host: null, monitors: monitorList(screenName) });
            b.model.screenMap = { "TEST-1": Qt.application.screens[0] };
            b.model.layerSource = Qt.resolvedUrl("NoLayerShell.qml");
            b.model.probeShell();
            b.model.fake = { config: config(), monitors: monitorList(screenName) };
            b.model.ingest(one("fps"));
            const ov = findOverlay(b);
            tryVerify(() => ov.orbItem !== null);
            compare(ov.curRight, 32);
            compare(ov.curBottom, 96);
            ov.moveBy(-50, -30);
            compare(ov.curRight, 82, "dragging left grows the right margin");
            compare(ov.curBottom, 126);
            ov.moveBy(100000, 100000);
            compare(ov.curRight, 0, "clamped at the screen edge");
            compare(ov.curBottom, 0);
            compare(bk.prefWrites.length, 0, "nothing is saved while dragging");
            ov.moveBy(-10, -20);
            ov.savePosition();
            compare(JSON.parse(bk.prefs.ai_bubble_pos)[screenName], { right: 10, bottom: 20 });
        }

        // ---- The visual parts ------------------------------------------------------------
        Component {
            id: orbComponent
            AiOrb {}
        }
        Component {
            id: textComponent
            AiBubbleText { holdMs: 120; fadeInMs: 20; fadeOutMs: 20 }
        }
        Component {
            id: dragComponent
            AiDragArea {
                property var log: []
                property int hides: 0
                property int ends: 0
                width: 96
                height: 96
                onMoved: (dx, dy) => log.push([dx, dy])
                onHideRequested: hides++
                onDragFinished: ends++
            }
        }

        function test_orb_dims_when_paused_and_stops_moving() {
            const o = createTemporaryObject(orbComponent, root, {});
            compare(o.width, 96);
            compare(o.opacity, 1);
            verify(o.animated);
            tryVerify(() => o.ringAngle > 0, 1000, "the ring turns while idle");
            o.paused = true;
            tryCompare(o, "opacity", 0.4);
            const a = o.ringAngle;
            wait(150);
            compare(o.ringAngle, a, "nothing moves while paused");
            o.analyse();
            compare(o.boost, 0, "no spin while paused");
            o.paused = false;
            tryCompare(o, "opacity", 1);
        }

        function test_orb_spins_faster_after_analyse_and_pulses_red_on_a_switch() {
            const o = createTemporaryObject(orbComponent, root, {});
            compare(o.busy, false);
            o.analyse();
            verify(o.busy);
            tryVerify(() => !o.busy, 2000, "the spin settles back");
            compare(o.glowing, false);
            o.pulse();
            tryVerify(() => o.glowing, 500);
            tryVerify(() => !o.glowing, 2500, "the glow fades");
        }

        function test_orb_static_when_not_animated() {
            const o = createTemporaryObject(orbComponent, root, { animated: false });
            wait(120);
            compare(o.ringAngle, 0);
        }

        function test_message_fades_in_holds_and_fades_out() {
            const t = createTemporaryObject(textComponent, root, {});
            compare(t.opacity, 0);
            t.flash("Switched to FPS mode");
            compare(t.text, "Switched to FPS mode");
            verify(t.shown);
            tryCompare(t, "opacity", 1, 500);
            tryVerify(() => !t.shown, 1000, "hides itself after holdMs");
            tryCompare(t, "opacity", 0, 500);
            verify(t.implicitWidth > 100);
        }

        function test_message_default_hold_is_four_seconds() {
            const t = createTemporaryObject(Qt.createComponent(Qt.resolvedUrl("AiBubbleText.qml")), root, {});
            compare(t.holdMs, 4000);
        }

        function test_dragging_needs_a_real_movement_and_ignores_stale_positions() {
            const d = createTemporaryObject(dragComponent, root, {});
            mousePress(d, 48, 48);
            mouseMove(d, 50, 49);
            mouseRelease(d, 50, 49);
            compare(d.log.length, 0, "a small wobble is a click");
            compare(d.ends, 0);
            mousePress(d, 48, 48);
            mouseMove(d, 40, 40);
            mouseMove(d, 30, 30);   // inside the settle time: stale, ignored
            compare(d.log, [[-8, -8]]);
            wait(40);
            mouseMove(d, 30, 30);
            compare(d.log, [[-8, -8], [-18, -18]]);
            mouseRelease(d, 30, 30);
            compare(d.ends, 1);
        }

        function test_double_click_asks_to_hide() {
            const d = createTemporaryObject(dragComponent, root, {});
            mouseDoubleClickSequence(d, 48, 48);
            compare(d.hides, 1);
        }
    }
}
