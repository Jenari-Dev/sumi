import QtQuick
import QtTest

// The write gate: slider coalescing, "no write while populating", read-only,
// confirmations and ordered sequences. A fake backend records every call.
Item {
    id: root
    width: 900
    height: 500

    QtObject {
        id: fake
        property bool readonly: false
        property var calls: []
        property var requests: []
        signal applied(string id, string feature, int value, bool ok, string message)
        signal state_ready(string id, string pages, string json)
        signal monitor_changed(string id, string context)
        function apply(id, feature, value) {
            calls.push({ id: id, feature: feature, value: value });
        }
        function request_state(id, pages) {
            requests.push({ id: id, pages: pages });
        }
    }

    Store {
        id: store
        backend: fake
        monitorId: "M1"
        pageKey: "game_visual"
    }

    function feat(id, extra) {
        return Object.assign({ id: id, label: id, page: "game_visual", code: 1, encoding: { type: "range" }, risk: "none",
            vendor_ui: true, value: 0, max: 100, supported: true, enabled: true }, extra || {});
    }
    function state(features) {
        return { id: "M1", model: "X", line: "gaming", oled: true, firmware: { scheme_major: 2, scheme_minor: 18, fw_version: 103, panel_id: 0 },
            bus: 5, context: { mode: 7, hdr: false, power_saving: false, user_color: false, blf_max: false }, features: features };
    }
    function baseState(brightness, vrr) {
        return state([
            feat("brightness", { value: brightness }),
            feat("vrr", { encoding: { type: "bit", mask: 1 }, value: vrr, max: null }),
            feat("reset", { encoding: { type: "command" }, risk: "resets", value: null, max: null }),
            feat("pip_mode", { page: "multi_screen", encoding: { type: "choice" }, risk: "replug", value: 0, max: null,
                options: [{ value: 0, label: "Off" }, { value: 1, label: "PIP" }, { value: 5, label: "PBP" }] }),
            feat("pip_source_a", { page: "multi_screen", encoding: { type: "low_byte" }, value: 0, max: null,
                options: [{ value: 15, label: "DP" }] }),
            feat("gamevisual", { encoding: { type: "low_byte" }, value: 7, max: null, options: [{ value: 7, label: "FPS" }, { value: 1, label: "Cinema" }] })
        ]);
    }

    SliderRow {
        id: row
        width: 600
        store: store
        fid: "brightness"
        title: "Brightness"
    }
    ToggleRow {
        id: toggleRow
        y: 80
        width: 600
        store: store
        fid: "vrr"
        title: "VRR"
    }

    SignalSpy {
        id: movedSpy
        target: slider
        signalName: "moved"
    }
    SignalSpy {
        id: releasedSpy
        target: slider
        signalName: "released"
    }
    RogSlider {
        id: slider
        y: 160
        width: 400
        value: 30
    }

    TestCase {
        name: "WriteGate"
        when: windowShown

        function init() {
            fake.calls = [];
            fake.requests = [];
            fake.readonly = false;
            store.monitorId = "";
            store.monitorId = "M1";
            store.ingest("game_visual", baseState(40, 0));
            fake.calls = [];
        }

        function test_drag_storm_keeps_one_write_in_flight_and_latest_wins() {
            store.write("brightness", 10);
            store.write("brightness", 20);
            store.write("brightness", 30);
            compare(fake.calls.length, 1, "only one write goes out while it is in flight");
            compare(fake.calls[0].value, 10);
            fake.applied("M1", "brightness", 10, true, "");
            compare(fake.calls.length, 2, "the latest value follows when the first returns");
            compare(fake.calls[1].value, 30);
            fake.applied("M1", "brightness", 30, true, "");
            compare(fake.calls.length, 2, "nothing left to send");
            compare(store.value("brightness"), 30);
        }

        function test_release_commits_the_final_value_once() {
            store.write("brightness", 10);
            store.write("brightness", 25);
            store.settle("brightness", 25);
            compare(fake.calls.length, 1);
            fake.applied("M1", "brightness", 10, true, "");
            compare(fake.calls.length, 2);
            compare(fake.calls[1].value, 25);
            fake.applied("M1", "brightness", 25, true, "");
            store.settle("brightness", 25);
            compare(fake.calls.length, 2, "already on the monitor: no duplicate write");
        }

        function test_release_sends_when_nothing_was_sent() {
            store.settle("brightness", 55);
            compare(fake.calls.length, 1);
            compare(fake.calls[0].value, 55);
        }

        function test_moving_back_to_the_value_in_flight_drops_the_pending_one() {
            store.write("brightness", 10);
            store.write("brightness", 20);
            store.write("brightness", 10);
            fake.applied("M1", "brightness", 10, true, "");
            compare(fake.calls.length, 1);
        }

        function test_populating_from_a_state_never_writes() {
            compare(row.current, 40);
            store.ingest("game_visual", baseState(77, 1));
            compare(row.current, 77, "the slider follows the state");
            verify(toggleRow.children.length > 0);
            compare(store.value("vrr"), 1);
            store.ingest("game_visual", baseState(12, 0));
            compare(fake.calls.length, 0, "no write came out of populating controls");
        }

        function test_write_is_refused_while_populating() {
            store.populating = true;
            verify(!store.write("brightness", 5));
            verify(!store.commit("vrr", 1));
            verify(!store.runSequence([{ id: "brightness", value: 5 }]));
            store.populating = false;
            compare(fake.calls.length, 0);
        }

        function test_read_only_daemon_blocks_everything() {
            fake.readonly = true;
            verify(!store.write("brightness", 5));
            verify(!store.commit("reset", 1));
            store.settle("brightness", 6);
            compare(fake.calls.length, 0);
            compare(store.confirm, null, "no confirmation either");
            compare(store.value("brightness"), 40, "value is untouched");
        }

        function test_risky_write_asks_first() {
            verify(store.commit("reset", 1));
            compare(fake.calls.length, 0);
            verify(store.confirm !== null);
            compare(store.confirm.text, "Reset the current GameVisual mode to its defaults?");
            store.answer(false);
            compare(fake.calls.length, 0);
            compare(store.confirm, null);
            store.commit("reset", 1);
            store.answer(true);
            compare(fake.calls.length, 1);
            compare(fake.calls[0].feature, "reset");
            compare(fake.calls[0].value, 1);
        }

        function test_only_pbp_layouts_ask_before_replugging() {
            store.commit("pip_mode", 1);
            compare(store.confirm, null);
            compare(fake.calls.length, 1);
            fake.applied("M1", "pip_mode", 1, true, "");
            fake.calls = [];
            store.commit("pip_mode", 5);
            verify(store.confirm !== null);
            compare(store.confirm.text, "The monitor will briefly disconnect while the layout changes.");
            compare(fake.calls.length, 0);
            store.answer(true);
            compare(fake.calls.length, 1);
        }

        function test_hdr_mode_asks_only_when_the_family_changes() {
            const st = baseState(40, 0);
            st.features.push(feat("hdr_mode", { encoding: { type: "choice" }, risk: "replug", value: 258, max: null,
                options: [{ value: 257, label: "Cinema HDR" }, { value: 258, label: "Gaming HDR" }, { value: 517, label: "Dolby Vision Bright" }, { value: 518, label: "Dolby Vision Dark" }] }));
            store.ingest("game_visual", st);
            store.commit("hdr_mode", 257);
            compare(store.confirm, null, "same family");
            compare(fake.calls.length, 1);
            fake.applied("M1", "hdr_mode", 257, true, "");
            fake.calls = [];
            store.commit("hdr_mode", 517);
            verify(store.confirm !== null, "other family");
            compare(store.confirm.text, "Switching between HDR10 and Dolby Vision makes the monitor reconnect; the screen goes dark for a moment.");
            compare(fake.calls.length, 0);
            store.answer(true);
            compare(fake.calls.length, 1);
            compare(fake.calls[0].value, 517);
        }

        // ---- Compare
        function test_compare_shows_the_values_from_before_the_edit_then_puts_the_current_ones_back() {
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            fake.calls = [];
            verify(store.snapshot !== null, "an edit started a snapshot");
            compare(store.snapshot.brightness, 40);
            verify(store.compareStart());
            compare(fake.calls.length, 1);
            compare(fake.calls[0].feature, "brightness");
            compare(fake.calls[0].value, 40);
            fake.applied("M1", "brightness", 40, true, "");
            compare(store.value("brightness"), 40);
            store.compareEnd();
            compare(fake.calls.length, 2);
            compare(fake.calls[1].value, 70, "released: the current value again");
            fake.applied("M1", "brightness", 70, true, "");
            compare(store.compareState, "idle");
            compare(store.snapshot.brightness, 40, "the snapshot is still there for the next press");
        }

        function test_releasing_compare_early_waits_for_the_write_in_flight() {
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            fake.calls = [];
            store.compareStart();
            store.compareEnd();
            compare(fake.calls.length, 1, "the restore waits for the first write");
            fake.applied("M1", "brightness", 40, true, "");
            compare(fake.calls.length, 2);
            compare(fake.calls[1].value, 70);
        }

        function test_compare_edits_do_not_replace_the_snapshot() {
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            store.compareStart();
            fake.applied("M1", "brightness", 40, true, "");
            store.compareEnd();
            fake.applied("M1", "brightness", 70, true, "");
            compare(store.snapshot.brightness, 40);
        }

        function test_a_new_snapshot_starts_with_the_next_burst_of_edits() {
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            store.write("brightness", 80);
            fake.applied("M1", "brightness", 80, true, "");
            compare(store.snapshot.brightness, 40, "edits in one go share a snapshot");
            store.lastEditAt = 0;   // the user paused
            store.write("brightness", 90);
            compare(store.snapshot.brightness, 80);
        }

        function test_changing_the_mode_drops_the_snapshot() {
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            verify(store.snapshot !== null);
            store.commit("gamevisual", 1);
            compare(store.snapshot, null);
        }

        function test_compare_needs_a_snapshot_a_difference_and_a_writable_daemon() {
            verify(!store.compareStart(), "nothing edited yet");
            store.write("brightness", 70);
            fake.applied("M1", "brightness", 70, true, "");
            fake.readonly = true;
            verify(!store.compareStart());
            fake.readonly = false;
            store.write("brightness", 40);   // back to the snapshot value: nothing differs
            fake.applied("M1", "brightness", 40, true, "");
            verify(!store.compareStart());
        }

        function test_choosing_the_current_value_is_a_no_op() {
            store.commit("gamevisual", 7);
            compare(fake.calls.length, 0);
        }

        function test_a_failed_write_reverts_and_rereads() {
            store.write("brightness", 10);
            compare(store.value("brightness"), 10);
            fake.applied("M1", "brightness", 10, false, "sumid is running read-only");
            compare(store.value("brightness"), 40);
            compare(fake.requests.length, 1);
            compare(fake.requests[0].pages, "game_visual");
        }

        function test_features_that_change_other_rules_reread_the_page() {
            store.commit("gamevisual", 1);
            compare(fake.requests.length, 0);
            fake.applied("M1", "gamevisual", 1, true, "");
            compare(fake.requests.length, 1);
            store.write("brightness", 5);
            fake.applied("M1", "brightness", 5, true, "");
            compare(fake.requests.length, 1, "brightness needs no re-read");
        }

        function test_sequence_waits_for_each_write() {
            store.runSequence([{ id: "pip_source_a", value: 15 }, { id: "pip_mode", value: 1 }]);
            compare(fake.calls.length, 1);
            compare(fake.calls[0].feature, "pip_source_a");
            fake.applied("M1", "pip_source_a", 15, true, "");
            compare(fake.calls.length, 2);
            compare(fake.calls[1].feature, "pip_mode");
        }

        function test_sequence_stops_at_the_first_error() {
            store.runSequence([{ id: "pip_source_a", value: 15 }, { id: "pip_mode", value: 1 }]);
            fake.applied("M1", "pip_source_a", 15, false, "nope");
            compare(fake.calls.length, 1);
        }

        function test_state_reply_marks_the_page_loaded() {
            store.reset();
            verify(store.loaded["game_visual"] !== true);
            fake.state_ready("M1", "game_visual", JSON.stringify(baseState(50, 1)));
            verify(store.loaded["game_visual"] === true);
            compare(store.value("brightness"), 50);
            compare(fake.calls.length, 0);
        }

        function test_brightness_only_change_is_patched_without_a_page_read() {
            const ctx = { mode: 7, hdr: false, power_saving: false, user_color: false, blf_max: false };
            fake.monitor_changed("M1", JSON.stringify({ context: ctx, brightness: 61 }));
            compare(store.value("brightness"), 61);
            wait(600);
            compare(fake.requests.length, 0, "no page read for a brightness-only change");
            compare(fake.calls.length, 0);
        }

        function test_a_context_change_rereads_the_visible_page() {
            const ctx = { mode: 7, hdr: true, power_saving: false, user_color: false, blf_max: false };
            fake.monitor_changed("M1", JSON.stringify({ context: ctx, brightness: 61 }));
            tryVerify(() => fake.requests.length === 1);
            compare(fake.requests[0].pages, "game_visual");
        }

        function test_changes_on_another_monitor_are_ignored() {
            fake.monitor_changed("OTHER", "{}");
            wait(600);
            compare(fake.requests.length, 0);
        }

        function test_an_edit_in_flight_survives_a_state_refresh() {
            store.write("brightness", 66);
            fake.state_ready("M1", "game_visual", JSON.stringify(baseState(40, 0)));
            compare(store.value("brightness"), 66, "the user's value is not overwritten by a stale read");
        }
    }

    TestCase {
        name: "Slider"
        when: windowShown

        function init() {
            slider.value = 30;
            movedSpy.clear();
            releasedSpy.clear();
        }

        function test_setting_value_from_code_emits_nothing() {
            slider.value = 70;
            compare(slider.shown, 70);
            compare(movedSpy.count, 0);
            compare(releasedSpy.count, 0);
        }

        function test_click_moves_and_releases_once() {
            mouseClick(slider, slider.trackWidth * 0.5, slider.height / 2);
            verify(movedSpy.count >= 1);
            compare(releasedSpy.count, 1);
            compare(releasedSpy.signalArguments[0][0], 50);
        }

        function test_drag_emits_each_new_value() {
            mousePress(slider, 0, slider.height / 2);
            mouseMove(slider, slider.trackWidth * 0.4, slider.height / 2);
            mouseMove(slider, slider.trackWidth * 0.8, slider.height / 2);
            mouseRelease(slider, slider.trackWidth * 0.8, slider.height / 2);
            verify(movedSpy.count >= 3);
            compare(releasedSpy.count, 1);
            compare(releasedSpy.signalArguments[0][0], 80);
        }
    }
}
