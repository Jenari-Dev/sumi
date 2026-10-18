import QtQuick
import QtTest

// Page-level behaviour against the real PG32UCDM3 daemon state (fixtures/state-pg32ucdm3.json):
// what a click writes, what asks for confirmation, what is greyed out.
Item {
    id: root
    width: 1100
    height: 800

    QtObject {
        id: fake
        property bool readonly: false
        property bool connected: true
        property string daemon_version: "9.9"
        property var calls: []
        property var requests: []
        property var copied: []
        property var autostart: false
        // Host-side: every `call` is recorded; replies come from `responses` (method -> text),
        // an Error entry fails the call. SetConfig stores what it is given as the next GetConfig.
        property var hostCalls: []
        property var responses: ({})
        property var prefs: ({})
        property var files: ({})
        property var written: ({})
        property var opened: []
        signal applied(string id, string feature, int value, bool ok, string message)
        signal state_ready(string id, string pages, string json)
        signal monitor_changed(string id, string context)
        signal call_done(string tag, bool ok, string result)
        function apply(id, feature, value) {
            calls.push({ id: id, feature: feature, value: value });
        }
        function request_state(id, pages) {
            requests.push({ id: id, pages: pages });
        }
        function call(method, argsJson, tag, notify) {
            const args = JSON.parse(argsJson);
            hostCalls.push({ method: method, args: args, notify: notify });
            let r = responses[method];
            if (method === "SetConfig" && !(typeof r === "object" && r.error)) {
                responses.GetConfig = args[0];
                r = "";
            }
            if (r === undefined) {
                call_done(tag, false, "no reply for " + method);
            } else if (typeof r === "object" && r.error) {
                call_done(tag, false, r.error);
            } else {
                call_done(tag, true, r);
            }
        }
        function called(method) {
            return hostCalls.filter(c => c.method === method);
        }
        function copy_text(t) {
            copied.push(t);
        }
        function app_version() {
            return "9.9.9";
        }
        function autostart_enabled() {
            return autostart;
        }
        function set_autostart(on) {
            autostart = on;
            return true;
        }
        function pref(k) {
            return prefs[k] || "";
        }
        property var prefWrites: []
        function set_pref(k, v) {
            prefs[k] = v;
            prefWrites.push([k, v]);
            return true;
        }
        function read_text_file(url) {
            return files[url] || "";
        }
        function write_text_file(url, text) {
            written[url] = text;
            return true;
        }
        function open_system_settings(m) {
            opened.push(m);
            return true;
        }
        property int restarts: 0
        property string manual: "file:///home/test/.local/share/doc/sumi/README.md"
        function restart() {
            restarts++;
        }
        function manual_url() {
            return manual;
        }
        function env_var(n) {
            return "";
        }
    }

    // Stands in for the file dialog: picks a fixed file at once.
    QtObject {
        id: chooser
        property string url: "file:///tmp/sumi-test.json"
        property string suggested: ""
        function save(name, cb) {
            suggested = name;
            cb(url);
        }
        function open(cb) {
            cb(url);
        }
    }

    Host {
        id: host
        backend: fake
    }

    Store {
        id: store
        backend: fake
        monitorId: "PG32UCDM3@i2c-5"
    }

    Loader {
        id: loader
    }

    TextCheck {
        id: textCheck
    }

    function fixture() {
        const x = new XMLHttpRequest();
        x.open("GET", Qt.resolvedUrl("state-pg32ucdm3.json"), false);
        x.send();
        return JSON.parse(x.responseText);
    }

    function show(file, props) {
        loader.source = "";
        loader.setSource(file, Object.assign({ store: store, host: host }, props || {}));
        return loader.item;
    }

    function baseConfig() {
        return {
            app_sync: { enabled: true, restore_on_unfocus: false, rule: [{ app: "dolphin", name: "Dolphin", mode: 1 }] },
            hotkeys: { enabled: true, step: 5 },
            target_mode: { strength: 44 },
            gamepad: { enabled: false },
            mode_sync: false,
            ai_visual: { enabled: false, notify: true, modes: ["rts_rpg", "fps", "scenery", "racing", "srgb", "moba", "cinema"], interval_ms: 1000 },
            desktop_saved: { panel_hiding: ["none"], color_scheme: "BreezeLight" }
        };
    }

    // What AiStatus says: installed or not, plus the daemon's state (download, monitors, error).
    function aiStatus(o) {
        return {
            installed: o.installed === true, prompts: true,
            model: { name: "vision_model.onnx", url: "https://huggingface.co/x", size: 352000000, sha256: "00" },
            state: Object.assign({ running: o.installed === true, monitors: {} }, o.state || {})
        };
    }

    function seeing(extra) {
        return Object.assign({
            verdict: { scores: [["fps", 61.2], ["racing", 22.5], ["office", 9.1], ["rpg", 5], ["moba", 1.2], ["scenery", 1]], is_game: 0.834, confidence: 0.719, top: "fps", target: "fps" },
            last_sent: "fps"
        }, extra || {});
    }

    function lastCall() {
        return fake.calls[fake.calls.length - 1];
    }

    TestCase {
        name: "Pages"
        when: windowShown

        function init() {
            fake.calls = [];
            fake.requests = [];
            fake.copied = [];
            fake.readonly = false;
            fake.autostart = false;
            fake.hostCalls = [];
            fake.prefs = {};
            fake.prefWrites = [];
            fake.files = {};
            fake.written = {};
            fake.opened = [];
            fake.restarts = 0;
            fake.manual = "file:///home/test/.local/share/doc/sumi/README.md";
            Tr.catalogue = {};
            fake.responses = {
                GetConfig: JSON.stringify(baseConfig()),
                GetHostState: JSON.stringify({ connector: "DP-3", hdr: false, target_mode: false }),
                GetDesktopPrefs: JSON.stringify({ panel_autohide: false, dark_theme: true }),
                SetHdr: "", SetTargetMode: "", SetPanelAutohide: "", SetDarkTheme: "", RunAction: "",
                Gamepads: "[]",
                Hotkeys: JSON.stringify([{ action: "osd_show", label: "EZ OSD: open the monitor menu", keys: ["Meta+F1"] }, { action: "brightness_up", label: "Brightness +", keys: [] }]),
                ListApps: JSON.stringify([{ id: "org.kde.dolphin", name: "Dolphin", icon: "", key: "dolphin" }, { id: "steam_app_1172470", name: "Apex Legends", icon: "", key: "steam_app_1172470" }]),
                RecentWindows: JSON.stringify([{ class: "firefox", desktop_file: "firefox", exe: "firefox", output: "DP-3" }]),
                TileLayoutNames: JSON.stringify([[1, "Top / bottom"], [2, "Left / right"], [13, "Four quadrants"]]),
                GetTileLayouts: JSON.stringify([{ output: "DP-3", aspect: 1.78, layout: 2, tiles: 2 }, { output: "HDMI-A-1", aspect: 1.78, layout: 0, tiles: 0 }]),
                SetTileLayout: JSON.stringify([{ output: "DP-3", aspect: 1.78, layout: 13, tiles: 4 }, { output: "HDMI-A-1", aspect: 1.78, layout: 0, tiles: 0 }]),
                ExportSettings: JSON.stringify({ ModelName: "PG32UCDM3", DisplayMode: 7, vcp: { "10": "100" } }),
                ImportSettings: "0x10 Brightness -> 100\n0x12 Contrast -> 80",
                ListMonitors: JSON.stringify([{ id: "PG32UCDM3@i2c-5", model: "PG32UCDM3", oled: true }]),
                Problems: "[]", Capabilities: "(caps)", GetState: "{\"state\":1}",
                AiStatus: JSON.stringify(aiStatus({ installed: true })), DownloadAiModel: "", RemoveAiModel: ""
            };
            host.config = baseConfig();
            host.hostStates = {};
            host.desktop = {};
            host.aiStatus = {};
            host.aiPreview = {};
            host.aiLoaded = false;
            host.aiError = "";
            store.monitors = [{ id: "PG32UCDM3@i2c-5", model: "PG32UCDM3", oled: true }];
            store.monitorId = "";
            store.monitorId = "PG32UCDM3@i2c-5";
            store.ingest("game_visual,game_plus,oled_care,multi_screen,system,device_info", fixture());
            store.confirm = null;
        }

        // Show a page and let Qt Quick lay it out (positioners polish on the next frame).
        function page(file, props) {
            const p = show(file, props);
            wait(30);
            return p;
        }

        function cleanup() {
            loader.source = "";
        }

        // ---- GameVisual
        function test_gamevisual_tiles_follow_the_daemon() {
            const p = page("GameVisualPage.qml");
            verify(findChild(p, "mode_FPS").checked, "FPS is the current mode");
            verify(!findChild(p, "mode_Cinema").checked);
            verify(findChild(p, "mode_Night Vision") !== null);
            verify(findChild(p, "mode_hdr").locked);
            verify(!findChild(p, "mode_hdr").checked, "HDR tile only checks while HDR is on");
            verify(findChild(p, "mode_hdr_dv") !== null, "the fixture has Dolby Vision options");
            verify(!findChild(p, "mode_hdr").enabled && !findChild(p, "mode_hdr_dv").enabled, "both HDR tiles look disabled in SDR");
            verify(!findChild(p, "mode_hdr_dv").checked);
        }

        function hdrState(value, withDv) {
            const st = fixture();
            st.context.hdr = value !== null;
            const hm = st.features.find(f => f.id === "hdr_mode");
            hm.value = value === null ? 0 : value;
            hm.enabled = true;
            if (withDv === false) {
                hm.options = hm.options.filter(o => (o.value >> 8) !== 2);
            }
            store.ingest("game_visual", st);
            return page("GameVisualPage.qml");
        }

        function test_dolby_vision_tile_only_when_there_are_dolby_options() {
            verify(findChild(hdrState(258, false), "mode_hdr_dv") === null);
            verify(findChild(hdrState(258, true), "mode_hdr_dv") !== null);
            compare(findChild(hdrState(258, true), "mode_hdr_dv").caption, "HDR Dolby Vision");
        }

        function test_selected_tile_follows_the_family_of_the_current_value() {
            let p = hdrState(0x0102);
            verify(findChild(p, "mode_hdr").checked);
            verify(!findChild(p, "mode_hdr_dv").checked);
            compare(p.selectedTile, p.stripTiles.findIndex(t => t.kind === "hdr" && t.family === 1));
            p = hdrState(0x0206);
            verify(!findChild(p, "mode_hdr").checked);
            verify(findChild(p, "mode_hdr_dv").checked);
            compare(p.selectedTile, p.stripTiles.findIndex(t => t.kind === "hdr" && t.family === 2));
        }

        function test_each_tile_has_its_own_pills() {
            const p = hdrState(0x0102);
            for (const v of [257, 258, 259, 264]) {
                verify(findChild(p, "hdr_" + v) !== null, "HDR10 pill " + v);
            }
            for (const v of [517, 518, 519, 520]) {
                verify(findChild(p, "hdr_" + v) === null, "no Dolby Vision pill " + v + " under the HDR tile");
            }
            mouseClick(findChild(p, "mode_hdr_dv"));
            for (const v of [517, 518, 519, 520]) {
                verify(findChild(p, "hdr_" + v) !== null, "Dolby Vision pill " + v);
            }
            verify(findChild(p, "hdr_258") === null);
            verify(findChild(p, "mode_hdr").checked, "the current family stays the checked tile until a pill is picked");
            compare(findChild(p, "hdr_520").text, "Source only");
            mouseClick(findChild(p, "mode_hdr"));
            verify(findChild(p, "hdr_258") !== null);
        }

        function test_hdr_off_tiles_do_nothing() {
            const p = hdrState(null);
            mouseClick(findChild(p, "mode_hdr_dv"));
            compare(fake.calls.length, 0);
            compare(p.pickedFamily, 0);
            verify(!findChild(p, "mode_hdr_dv").checked);
        }

        function test_crossing_families_asks_and_staying_does_not() {
            const p = hdrState(258);
            verify(findChild(p, "hdr_258").checked);
            // Same family: a plain write, no dialog.
            mouseClick(findChild(p, "hdr_259"));
            compare(fake.calls.length, 1);
            compare(lastCall().value, 259);
            compare(store.confirm, null);
            fake.applied("PG32UCDM3@i2c-5", "hdr_mode", 259, true, "");
            fake.calls = [];
            // Other family: its tile, then a sub-mode, asks first.
            mouseClick(findChild(p, "mode_hdr_dv"));
            compare(fake.calls.length, 0, "the tile itself writes nothing");
            mouseClick(findChild(p, "hdr_518"));
            compare(fake.calls.length, 0);
            verify(store.confirm !== null);
            compare(store.confirm.text, "Switching between HDR10 and Dolby Vision makes the monitor reconnect; the screen goes dark for a moment.");
            store.answer(false);
            compare(fake.calls.length, 0);
            mouseClick(findChild(p, "hdr_518"));
            store.answer(true);
            compare(fake.calls.length, 1, "one plain write of the whole value");
            compare(lastCall().value, 518);
            fake.applied("PG32UCDM3@i2c-5", "hdr_mode", 518, true, "");
            verify(findChild(p, "mode_hdr_dv").checked);
            verify(findChild(p, "hdr_518").checked);
            // Within Dolby Vision it does not ask; back to HDR10 does.
            fake.calls = [];
            mouseClick(findChild(p, "hdr_517"));
            compare(store.confirm, null);
            compare(lastCall().value, 517);
            fake.applied("PG32UCDM3@i2c-5", "hdr_mode", 517, true, "");
            fake.calls = [];
            mouseClick(findChild(p, "mode_hdr"));
            mouseClick(findChild(p, "hdr_257"));
            compare(fake.calls.length, 0);
            verify(store.confirm !== null);
        }

        function test_advertised_hdr_format_is_shown_next_to_the_hdr_tile() {
            const p = page("GameVisualPage.qml");
            const st = fixture();
            compare(st.hdr_format, "hdr10", "the fixture is a real state: this monitor advertises HDR10");
            compare(findChild(p, "hdr_advertised").text, "Advertised: HDR10");
            st.hdr_format = "dolby_vision";
            store.ingest("game_visual", st);
            compare(findChild(p, "hdr_advertised").text, "Advertised: Dolby Vision");
            verify(findChild(p, "hdr_advertised").visible);
            st.hdr_format = "hdr10";
            store.ingest("game_visual", st);
            compare(findChild(p, "hdr_advertised").text, "Advertised: HDR10");
            st.hdr_format = null;
            store.ingest("game_visual", st);
            verify(!findChild(p, "hdr_advertised").visible);
        }

        function test_hdr_pills_are_not_shown_in_sdr() {
            const p = page("GameVisualPage.qml");
            verify(!findChild(p, "hdr_258").visible, "the sub-mode rows only show while HDR is on");
        }

        function test_choosing_a_mode_writes_it_and_rereads() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_Cinema"));
            compare(fake.calls.length, 1);
            compare(lastCall().feature, "gamevisual");
            compare(lastCall().value, 1);
            fake.applied("PG32UCDM3@i2c-5", "gamevisual", 1, true, "");
            compare(fake.requests.length, 1, "enable rules depend on the mode");
        }

        function test_clicking_the_current_mode_does_nothing() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_FPS"));
            compare(fake.calls.length, 0);
        }

        function test_hdr_status_tile_is_not_clickable() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_hdr"));
            compare(fake.calls.length, 0);
        }

        function test_reset_asks_before_writing() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "reset"));
            compare(fake.calls.length, 0);
            verify(store.confirm !== null);
            compare(store.confirm.text, "Reset the current GameVisual mode to its defaults?");
            store.answer(true);
            compare(lastCall().feature, "reset");
            compare(lastCall().value, 1);
        }

        function test_unsupported_and_disabled_rows_are_greyed() {
            const p = page("GameVisualPage.qml");
            verify(!findChild(p, "slider_overdrive").enabled, "not supported on this OLED");
            verify(!findChild(p, "slider_hue").enabled);
            verify(!findChild(p, "slider_red_gain").enabled, "RGB gains need colour temperature User");
            verify(!findChild(p, "slider_blue_light").enabled, "Blue Light Filter is Racing only");
            verify(findChild(p, "slider_brightness").enabled);
            compare(findChild(p, "slider_brightness").to, 100);
            compare(findChild(p, "slider_brightness").value, 100);
            compare(findChild(p, "slider_contrast").value, 80);
        }

        function test_slider_drag_through_the_page_coalesces() {
            const p = page("GameVisualPage.qml");
            const s = findChild(p, "slider_contrast");
            mousePress(s, 0, s.height / 2);
            mouseMove(s, s.trackWidth * 0.3, s.height / 2);
            mouseMove(s, s.trackWidth * 0.6, s.height / 2);
            mouseRelease(s, s.trackWidth * 0.6, s.height / 2);
            compare(fake.calls.length, 1, "one write in flight");
            compare(lastCall().feature, "contrast");
            fake.applied("PG32UCDM3@i2c-5", "contrast", lastCall().value, true, "");
            compare(fake.calls.length, 2);
            compare(lastCall().value, 60, "the final position is what ends up on the monitor");
        }

        function test_read_only_daemon_sends_nothing_from_the_page() {
            fake.readonly = true;
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_Cinema"));
            const s = findChild(p, "slider_contrast");
            mouseClick(s, s.trackWidth * 0.2, s.height / 2);
            compare(fake.calls.length, 0);
            compare(findChild(p, "slider_contrast").value, 80, "the slider snaps back");
        }

        // ---- GamePlus
        function test_gameplus_option_writes_the_style() {
            const p = page("GamePlusPage.qml");
            mouseClick(findChild(p, "opt_7"));
            compare(lastCall().feature, "crosshair");
            compare(lastCall().value, 7);
        }

        function test_gameplus_arrow_sends_one_step_then_repeats_while_held() {
            const p = page("GamePlusPage.qml");
            const up = findChild(p, "pos_up");
            mousePress(up);
            compare(fake.calls.length, 1);
            compare(lastCall().feature, "gameplus_position");
            compare(lastCall().value, 1);
            wait(500);
            compare(fake.calls.length, 1, "no second write while the first is in flight");
            fake.applied("PG32UCDM3@i2c-5", "gameplus_position", 1, true, "");
            wait(200);
            verify(fake.calls.length >= 2, "held key repeats");
            mouseRelease(up);
            const n = fake.calls.length;
            fake.applied("PG32UCDM3@i2c-5", "gameplus_position", 1, true, "");
            wait(300);
            compare(fake.calls.length, n, "released: no more steps");
        }

        function test_gameplus_arrows_map_to_the_four_commands() {
            const p = page("GamePlusPage.qml");
            const want = { pos_up: 1, pos_down: 2, pos_right: 3, pos_left: 4 };
            for (const name in want) {
                fake.calls = [];
                store.inflight = {};
                const b = findChild(p, name);
                mousePress(b);
                mouseRelease(b);
                compare(lastCall().value, want[name], name);
            }
        }

        // ---- OLED Care
        function test_oled_toggle_and_pixel_cleaning() {
            const p = page("OledCarePage.qml");
            mouseClick(findChild(p, "toggle_screen_dimming"));
            compare(lastCall().feature, "screen_dimming");
            compare(lastCall().value, 0);
            fake.calls = [];
            mouseClick(findChild(p, "pixel_cleaning"));
            compare(fake.calls.length, 0, "pixel cleaning asks first");
            compare(store.confirm.text, "Pixel cleaning takes about 6 minutes. The screen turns off while it runs. Don't unplug the monitor.");
            store.answer(true);
            compare(lastCall().feature, "pixel_cleaning");
            compare(lastCall().value, 1);
        }

        // ---- MultiScreen
        function test_pbp_apply_confirms_then_writes_sources_modes_then_layout() {
            const p = page("MultiScreenPage.qml");
            mouseClick(findChild(p, "layout_5"));
            p.pick("srcA", 0);
            p.pick("srcB", 1);
            p.pick("gvA", 1);
            p.pick("gvB", 2);
            mouseClick(findChild(p, "pip_apply"));
            compare(fake.calls.length, 0);
            compare(store.confirm.text, "The monitor will briefly disconnect while the layout changes.");
            store.answer(true);
            const order = ["pip_source_a", "pip_source_b", "pip_gamevisual_a", "pip_gamevisual_b", "pip_mode"];
            for (let i = 0; i < order.length; i++) {
                compare(fake.calls.length, i + 1);
                compare(lastCall().feature, order[i]);
                fake.applied("PG32UCDM3@i2c-5", lastCall().feature, lastCall().value, true, "");
            }
            compare(lastCall().value, 5);
        }

        function test_pip_defaults_to_the_first_source_and_mode_when_the_monitor_reports_none() {
            const p = page("MultiScreenPage.qml");
            compare(findChild(p, "combo_pip_source_a").currentIndex, 0, "0xF5 reads 0: first source, not a dash");
            compare(findChild(p, "combo_pip_gamevisual_b").currentIndex, 0);
            verify(findChild(p, "combo_pip_source_a").currentText !== "—");
        }

        function test_pip_apply_needs_no_confirmation_and_writes_the_defaults_first() {
            const p = page("MultiScreenPage.qml");
            mouseClick(findChild(p, "layout_3"));
            mouseClick(findChild(p, "pip_apply"));
            compare(store.confirm, null);
            compare(lastCall().feature, "pip_source_a", "the defaults the page shows are what gets written");
            const order = ["pip_source_a", "pip_source_b", "pip_gamevisual_a", "pip_gamevisual_b", "pip_mode"];
            for (let i = 0; i < order.length; i++) {
                compare(fake.calls.length, i + 1);
                compare(lastCall().feature, order[i]);
                fake.applied("PG32UCDM3@i2c-5", lastCall().feature, lastCall().value, true, "");
            }
            compare(lastCall().value, 3);
        }

        function test_kvm_confirms() {
            const p = page("MultiScreenPage.qml");
            p.fn = "kvm";
            mouseClick(findChild(p, "kvm_3"));
            compare(fake.calls.length, 0);
            compare(store.confirm.text, "Peripherals on the monitor's USB hub (keyboard, mouse…) will move to the other computer.");
            store.answer(true);
            compare(lastCall().feature, "kvm");
            compare(lastCall().value, 3);
        }

        // ---- System
        function test_power_saving_asks_first_standard_does_not() {
            const p = page("SystemPage.qml");
            mouseClick(findChild(p, "power_1"));
            compare(fake.calls.length, 0);
            compare(store.confirm.text, "Power saving mode locks most picture settings; only brightness and contrast stay adjustable.");
            store.answer(true);
            compare(lastCall().feature, "power_saving");
            compare(lastCall().value, 1);
            fake.applied("PG32UCDM3@i2c-5", "power_saving", 1, true, "");
            compare(fake.requests.length, 1, "power saving changes which controls are usable");
        }

        function test_mute_maps_to_the_two_value_register() {
            const p = page("SystemPage.qml");
            mouseClick(findChild(p, "toggle_mute"));
            compare(lastCall().feature, "mute");
            compare(lastCall().value, 1, "on = muted");
        }

        function test_input_select_confirms() {
            const p = page("SystemPage.qml");
            store.commit("input", 17);
            compare(fake.calls.length, 0);
            compare(store.confirm.text, "This PC will lose the picture until you switch back on the monitor.");
        }

        // ---- User Center / Application Settings
        // ---- Firmware update check
        function fwPage() {
            fake.responses.CheckFirmware = JSON.stringify({ current: "MCM103", latest: null, url: null, checked_at: 1 });
            return page("UserCenterPage.qml", { backend: fake });
        }
        function fwText(p) {
            return findChild(p, "fw_status").text;
        }

        function test_firmware_unchecked_shows_the_note_and_asks_nothing() {
            const p = fwPage();
            compare(p.fwState, "unchecked");
            compare(fwText(p), "Asks ASUS's download server whether newer firmware exists; only the model name is sent.");
            verify(findChild(p, "fw_check").visible);
            verify(!findChild(p, "fw_download").visible);
            compare(fake.called("CheckFirmware").length, 0, "no network on opening the page");
        }

        function test_firmware_checking_disables_the_button() {
            const p = fwPage();
            p.fwState = "checking";
            compare(fwText(p), "Checking…");
            verify(!findChild(p, "fw_check").enabled);
        }

        function test_firmware_up_to_date() {
            const p = fwPage();
            mouseClick(findChild(p, "fw_check"));
            compare(fake.called("CheckFirmware")[0].args[0], "PG32UCDM3@i2c-5");
            compare(p.fwState, "uptodate");
            compare(fwText(p), "Up to date");
            verify(!findChild(p, "fw_download").visible);
        }

        function test_firmware_available_then_download_progress_then_saved() {
            const p = fwPage();
            fake.responses.CheckFirmware = JSON.stringify({ current: "MCM103", latest: "MCM105", url: "https://x/ASUS_PG32UCDM3_MCM105.zip", checked_at: 1 });
            fake.responses.DownloadFirmware = "";
            mouseClick(findChild(p, "fw_check"));
            compare(p.fwState, "available");
            compare(fwText(p), "MCM105 is available");
            mouseClick(findChild(p, "fw_download"));
            compare(p.fwState, "downloading");
            compare(fake.called("DownloadFirmware")[0].args[0], "PG32UCDM3@i2c-5");
            verify(findChild(p, "fw_progress").visible);
            fake.responses.FirmwareStatus = JSON.stringify({ download: { state: "running", done: 5000000, total: 20000000 } });
            tryVerify(() => p.fwDone === 5000000, 2000);
            compare(findChild(p, "fw_progress").valueText, "5.0 of 20.0 MB");
            fake.responses.FirmwareStatus = JSON.stringify({ download: { state: "done", done: 20000000, total: 20000000, path: "/home/u/Downloads/ASUS_PG32UCDM3_MCM105 (1).zip" } });
            tryCompare(p, "fwState", "saved", 2000);
            compare(fwText(p), "Saved to /home/u/Downloads/ASUS_PG32UCDM3_MCM105 (1).zip");
            verify(findChild(p, "fw_note").visible);
            compare(findChild(p, "fw_note").text, "Run ASUS's update tool from this file on Windows; Sumi doesn't flash firmware.");
            const opened = [];
            p.openUrl = u => opened.push(u);
            mouseClick(findChild(p, "fw_open"));
            compare(opened, ["file:///home/u/Downloads"]);
        }

        function test_firmware_errors_are_shown_inline() {
            const p = fwPage();
            fake.responses.CheckFirmware = { error: "Couldn't reach ASUS's download server (timeout)" };
            mouseClick(findChild(p, "fw_check"));
            compare(p.fwState, "error");
            compare(fwText(p), "Couldn't reach ASUS's download server (timeout)");
            verify(findChild(p, "fw_check").enabled, "and the user can try again");
            // A failed download too.
            fake.responses.CheckFirmware = JSON.stringify({ current: "MCM103", latest: "MCM105", url: "u", checked_at: 1 });
            fake.responses.DownloadFirmware = "";
            mouseClick(findChild(p, "fw_check"));
            mouseClick(findChild(p, "fw_download"));
            fake.responses.FirmwareStatus = JSON.stringify({ download: { state: "error", error: "disk full" } });
            tryCompare(p, "fwState", "error", 2000);
            compare(fwText(p), "disk full");
        }

        function test_user_center_copy_has_no_serial() {
            const st = fixture();
            compare(st.connector, "DP-3");
            store.ingest("device_info,system,game_visual", st);
            const p = page("UserCenterPage.qml", { backend: fake });
            mouseClick(findChild(p, "copy"));
            compare(fake.copied.length, 1);
            const t = fake.copied[0];
            verify(t.indexOf("Model: PG32UCDM3") >= 0, t);
            verify(t.indexOf("Firmware: MCM103") >= 0, t);
            verify(t.indexOf("VCP scheme: 2.18") >= 0, t);
            verify(t.indexOf("Refresh rate: 240 Hz") >= 0, t);
            verify(/Display usage time: \d+ h/.test(t), t);
            verify(t.indexOf("Input: DisplayPort") >= 0, t);
            verify(t.indexOf("GameVisual mode: FPS") >= 0, t);
            verify(t.indexOf("I2C bus: i2c-5") >= 0, t);
            verify(t.indexOf("HDR format: HDR10") >= 0, t);
            verify(t.indexOf("Connector: DP-3") >= 0, t);
            verify(!/serial|S\/N|SN:/i.test(t), "never a serial number");
        }

        function test_start_on_login_toggle_writes_through_the_backend() {
            const p = page("AppSettingsPage.qml", { backend: fake });
            const t = findChild(p, "toggle_autostart");
            verify(!t.checked);
            mouseClick(t);
            verify(fake.autostart);
            verify(t.checked);
        }

        // ---- Host configuration
        function test_config_is_read_modified_and_written_whole() {
            host.updateConfig(c => {
                c.hotkeys.step = 10;
            });
            const sets = fake.called("SetConfig");
            compare(sets.length, 1);
            const sent = JSON.parse(sets[0].args[0]);
            compare(sent.hotkeys.step, 10);
            compare(sent.hotkeys.enabled, true, "the rest of the object is kept");
            compare(sent.desktop_saved.color_scheme, "BreezeLight", "fields the app does not edit survive");
            compare(host.config.hotkeys.step, 10);
            compare(fake.called("GetConfig").length, 1, "the current config is read first");
        }

        function test_config_edits_are_applied_to_what_the_daemon_has_now() {
            // Someone else (the daemon, the CLI) changed the config since the page last read it.
            const theirs = baseConfig();
            theirs.desktop_saved.panel_hiding = ["autohide"];
            theirs.target_mode.strength = 60;
            fake.responses.GetConfig = JSON.stringify(theirs);
            host.updateConfig(c => {
                c.mode_sync = true;
            });
            const sent = JSON.parse(fake.called("SetConfig")[0].args[0]);
            compare(sent.mode_sync, true);
            compare(sent.desktop_saved.panel_hiding[0], "autohide", "their change is not clobbered by a stale copy");
            compare(sent.target_mode.strength, 60);
        }

        function test_a_refused_config_write_shows_the_error_and_reloads() {
            fake.responses.SetConfig = { error: "sumid is running read-only" };
            host.updateConfig(c => {
                c.hotkeys.enabled = false;
            });
            compare(fake.called("SetConfig")[0].notify, true, "errors go to the toast");
            compare(host.config.hotkeys.enabled, true, "the UI shows what the daemon really has");
        }

        // ---- HotKey
        function test_hotkey_page_lists_actions_and_tests_one() {
            const p = page("HotKeyPage.qml");
            compare(p.bindings.length, 2);
            compare(p.keysText(p.bindings[0].keys), "Meta+F1");
            compare(p.keysText(p.bindings[1].keys), "Not set");
            mouseClick(findChild(p, "test_brightness_up"));
            const c = fake.called("RunAction");
            compare(c.length, 1);
            compare(c[0].args[0], "brightness_up");
            compare(c[0].notify, true, "a refusal is shown in the toast");
        }

        function test_hotkey_test_button_confirms_actions_with_side_effects() {
            fake.responses.Hotkeys = JSON.stringify([{ action: "kvm_toggle", label: "KVM: switch USB upstream", keys: [] }, { action: "hdr_toggle", label: "HDR on/off", keys: [] }]);
            const p = page("HotKeyPage.qml");
            mouseClick(findChild(p, "test_kvm_toggle"));
            compare(fake.called("RunAction").length, 0, "moving the USB hub asks first");
            compare(store.confirm.text, "Peripherals on the monitor's USB hub (keyboard, mouse…) will move to the other computer.");
            store.answer(false);
            compare(fake.called("RunAction").length, 0);
            mouseClick(findChild(p, "test_hdr_toggle"));
            verify(store.confirm !== null);
            store.answer(true);
            compare(fake.called("RunAction").length, 1);
            compare(fake.called("RunAction")[0].args[0], "hdr_toggle");
        }

        function test_hotkey_toggle_step_and_system_settings() {
            const p = page("HotKeyPage.qml");
            mouseClick(findChild(p, "toggle_hotkeys"));
            compare(JSON.parse(fake.called("SetConfig")[0].args[0]).hotkeys.enabled, false);
            fake.hostCalls = [];
            const combo = findChild(p, "combo_step");
            combo.enabled = true;
            combo.activated(3);
            compare(JSON.parse(fake.called("SetConfig")[0].args[0]).hotkeys.step, 10);
            mouseClick(findChild(p, "open_keys"));
            compare(fake.opened[0], "kcm_keys");
        }

        function test_hotkey_info_card_is_dismissed_for_good() {
            let p = page("HotKeyPage.qml");
            verify(findChild(p, "info_card").visible);
            mouseClick(findChild(p, "dismiss_info"));
            verify(!findChild(p, "info_card").visible);
            compare(fake.prefs.hotkey_info_dismissed, "1");
            p = page("HotKeyPage.qml");
            verify(!findChild(p, "info_card").visible, "stays dismissed");
        }

        function test_gamepad_row_only_with_a_gamepad() {
            let p = page("HotKeyPage.qml");
            verify(!findChild(p, "gamepad_row").visible);
            fake.responses.Gamepads = JSON.stringify(["DualSense Wireless Controller"]);
            p = page("HotKeyPage.qml");
            verify(findChild(p, "gamepad_row").visible);
            mouseClick(findChild(p, "toggle_gamepad"));
            compare(JSON.parse(fake.called("SetConfig")[0].args[0]).gamepad.enabled, true);
        }

        function test_hotkey_page_explains_when_kde_shortcuts_are_missing() {
            fake.responses.Hotkeys = { error: "The name org.kde.kglobalaccel was not provided" };
            const p = page("HotKeyPage.qml");
            compare(p.bindings.length, 0);
            verify(p.bindingsError.indexOf("kglobalaccel") >= 0);
        }

        // ---- App Tweaker
        function test_app_tweaker_shows_the_rules() {
            const p = page("AppTweakerPage.qml");
            verify(p.appsLoaded);
            compare(p.rules.length, 1);
            compare(findChild(p, "app_0").currentText, "Dolphin");
            compare(findChild(p, "mode_0").currentText, "Cinema");
            compare(findChild(p, "mode_default").currentText, "No change");
            const items = p.appItems;
            verify(items[0].header && items[0].label === "Recently used", "recently used apps come first");
            verify(items.some(i => i.header && i.label === "Installed apps"));
        }

        function test_app_tweaker_add_change_delete_save_every_time() {
            const p = page("AppTweakerPage.qml");
            mouseClick(findChild(p, "add_rule"));
            wait(30);
            let sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            compare(sent.app_sync.rule.length, 2);
            compare(sent.app_sync.rule[1].app, "");
            // pick an app for the new row (index into the combo model, past the section header)
            const i = p.appItems.findIndex(x => x.key === "steam_app_1172470");
            findChild(p, "app_1").activated(i);
            wait(30);
            sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            compare(sent.app_sync.rule[1].app, "steam_app_1172470");
            compare(sent.app_sync.rule[1].name, "Apex Legends");
            findChild(p, "mode_1").activated(1);
            wait(30);
            sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            compare(sent.app_sync.rule[1].mode, p.modes[1].value);
            mouseClick(findChild(p, "delete_0"));
            sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            compare(sent.app_sync.rule.length, 1);
            compare(sent.app_sync.rule[0].app, "steam_app_1172470");
            compare(sent.desktop_saved.color_scheme, "BreezeLight");
        }

        function test_app_tweaker_default_mode_and_toggles() {
            const p = page("AppTweakerPage.qml");
            findChild(p, "mode_default").activated(2);
            let sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            compare(sent.app_sync.default_mode, p.modes[1].value);
            findChild(p, "mode_default").activated(0);
            sent = JSON.parse(fake.called("SetConfig").pop().args[0]);
            verify(!("default_mode" in sent.app_sync), "No change = no default mode");
            mouseClick(findChild(p, "toggle_restore"));
            compare(JSON.parse(fake.called("SetConfig").pop().args[0]).app_sync.restore_on_unfocus, true);
            verify(findChild(p, "toggle_hdr_off").checked, "HDR is turned off for SDR modes unless told otherwise");
            mouseClick(findChild(p, "toggle_hdr_off"));
            compare(JSON.parse(fake.called("SetConfig").pop().args[0]).app_sync.hdr_off_for_mode, false);
            mouseClick(findChild(p, "toggle_app_sync"));
            compare(JSON.parse(fake.called("SetConfig").pop().args[0]).app_sync.enabled, false);
        }

        function test_app_tweaker_conflict_hides_add() {
            const c = baseConfig();
            c.app_sync.rule = [{ app: "dolphin", name: "Dolphin", mode: 1 }, { app: "Dolphin", name: "Dolphin", mode: 7 }];
            host.config = c;
            fake.responses.GetConfig = JSON.stringify(c);
            const p = page("AppTweakerPage.qml");
            verify(findChild(p, "conflict").visible);
            compare(findChild(p, "conflict").text, "Settings conflict: Dolphin");
            verify(!findChild(p, "add_rule").visible);
        }

        function test_app_tweaker_allows_ten_rules() {
            const c = baseConfig();
            c.app_sync.rule = [];
            for (let i = 0; i < 10; i++) {
                c.app_sync.rule.push({ app: "app" + i, name: "App " + i, mode: 1 });
            }
            host.config = c;
            fake.responses.GetConfig = JSON.stringify(c);
            const p = page("AppTweakerPage.qml");
            verify(!findChild(p, "add_rule").visible, "at most ten rows");
        }

        // ---- GameVisual: Mode Sync, HDR off, Import / Export, Compare
        function test_mode_sync_shows_only_with_two_monitors_of_the_model() {
            let p = page("GameVisualPage.qml");
            verify(!findChild(p, "mode_sync").visible);
            store.monitors = [{ id: "PG32UCDM3@i2c-5", model: "PG32UCDM3" }, { id: "PG32UCDM3@i2c-6", model: "PG32UCDM3" }];
            verify(findChild(p, "mode_sync").visible);
            mouseClick(findChild(p, "mode_sync"));
            compare(JSON.parse(fake.called("SetConfig").pop().args[0]).mode_sync, true);
            store.monitors = [{ id: "PG32UCDM3@i2c-5", model: "PG32UCDM3" }, { id: "PG27AQN@i2c-6", model: "PG27AQN" }];
            verify(!findChild(p, "mode_sync").visible, "different models do not count");
        }

        function test_choosing_an_sdr_mode_in_hdr_asks_first() {
            const st = fixture();
            st.context.hdr = true;
            st.features.find(f => f.id === "gamevisual").enabled = true;
            store.ingest("game_visual", st);
            const p = page("GameVisualPage.qml");
            verify(!findChild(p, "mode_Cinema").visible, "the strip shows the HDR tile (last page), not the first tiles");
            mouseClick(findChild(p, "strip_left"));
            verify(findChild(p, "mode_Cinema").visible);
            verify(findChild(p, "mode_Cinema").enabled, "the tiles stay clickable in HDR");
            mouseClick(findChild(p, "mode_Cinema"));
            compare(fake.calls.length, 0);
            compare(store.confirm.text, "Switching to Cinema turns HDR off for this monitor.");
            store.answer(true);
            compare(lastCall().feature, "gamevisual");
            compare(lastCall().value, 1);
        }

        function test_compare_button_flips_while_held() {
            const p = page("GameVisualPage.qml");
            const b = findChild(p, "compare");
            verify(!b.enabled, "nothing to compare before the first edit");
            store.write("brightness", 70);
            fake.applied("PG32UCDM3@i2c-5", "brightness", 70, true, "");
            fake.calls = [];
            verify(b.enabled);
            mousePress(b);
            compare(lastCall().value, 100, "held: the value from before the edit");
            fake.applied("PG32UCDM3@i2c-5", "brightness", 100, true, "");
            mouseRelease(b);
            compare(lastCall().value, 70, "released: back to the current value");
        }

        function test_export_writes_the_daemon_file_with_a_suggested_name() {
            const p = page("GameVisualPage.qml", { chooser: chooser });
            mouseClick(findChild(p, "export"));
            compare(chooser.suggested, "PG32UCDM3_FPS.json");
            compare(fake.called("ExportSettings")[0].args[0], "PG32UCDM3@i2c-5");
            verify(fake.written["file:///tmp/sumi-test.json"].indexOf("PG32UCDM3") >= 0);
        }

        function test_import_applies_the_file_and_shows_the_log() {
            fake.files["file:///tmp/sumi-test.json"] = JSON.stringify({ ModelName: "PG32UCDM3", DisplayMode: 7, vcp: {} });
            const p = page("GameVisualPage.qml", { chooser: chooser });
            mouseClick(findChild(p, "import"));
            const c = fake.called("ImportSettings");
            compare(c.length, 1);
            compare(c[0].args[2], false, "first try without switching the mode");
            compare(store.confirm.title, "Settings imported");
            verify(store.confirm.text.indexOf("0x10 Brightness") >= 0);
            verify(store.confirm.large && !store.confirm.showCancel);
        }

        function test_import_for_another_mode_asks_before_switching() {
            fake.files["file:///tmp/sumi-test.json"] = JSON.stringify({ ModelName: "PG32UCDM3", DisplayMode: 1, vcp: {} });
            fake.responses.ImportSettings = { error: "the file is for GameVisual mode 1, the monitor is in mode 7; switch first" };
            const p = page("GameVisualPage.qml", { chooser: chooser });
            mouseClick(findChild(p, "import"));
            compare(store.confirm.text, "This file is for Cinema. Switch to it and apply?");
            fake.responses.ImportSettings = "0xDC GameVisual -> 1";
            store.answer(true);
            const c = fake.called("ImportSettings");
            compare(c.length, 2);
            compare(c[1].args[2], true);
            verify(store.confirm.text.indexOf("0xDC") >= 0, "the log is shown");
        }

        function test_import_of_a_wrong_file_reports_the_error() {
            fake.files["file:///tmp/sumi-test.json"] = JSON.stringify({ ModelName: "OTHER", vcp: {} });
            fake.responses.ImportSettings = { error: "this file is for \"OTHER\", not PG32UCDM3" };
            const p = page("GameVisualPage.qml", { chooser: chooser });
            mouseClick(findChild(p, "import"));
            compare(store.confirm.title, "Import failed");
            verify(store.confirm.text.indexOf("OTHER") >= 0);
        }

        function test_import_is_not_offered_to_a_read_only_daemon() {
            fake.readonly = true;
            const p = page("GameVisualPage.qml");
            verify(!findChild(p, "import").enabled);
            verify(findChild(p, "export").enabled, "reading is always fine");
        }

        // ---- OLED Care, System: host-side toggles
        function test_target_mode_row_follows_the_host_state() {
            const p = page("OledCarePage.qml");
            const t = findChild(p, "toggle_target_mode");
            verify(t.enabled);
            verify(!t.checked);
            fake.responses.GetHostState = JSON.stringify({ connector: "DP-3", hdr: false, target_mode: false });
            mouseClick(t);
            const c = fake.called("SetTargetMode");
            compare(c.length, 1);
            compare(c[0].args[0], true);
            compare(c[0].notify, true);
            compare(fake.called("GetHostState").length, 2, "the state is re-read after the write");
        }

        function test_target_mode_is_greyed_when_kwin_does_not_say() {
            fake.responses.GetHostState = JSON.stringify({ connector: "DP-3", hdr: null, target_mode: null });
            const p = page("OledCarePage.qml");
            verify(!findChild(p, "toggle_target_mode").enabled);
        }

        function test_display_hdr_and_desktop_toggles() {
            fake.responses.GetHostState = JSON.stringify({ connector: "DP-3", hdr: true, target_mode: false });
            const p = page("SystemPage.qml");
            const hdr = findChild(p, "toggle_display_hdr");
            verify(hdr.checked, "HDR is on in KDE");
            mouseClick(hdr);
            compare(fake.called("SetHdr")[0].args[0], "PG32UCDM3@i2c-5");
            compare(fake.called("SetHdr")[0].args[1], false);
            verify(findChild(p, "toggle_dark_theme").checked);
            verify(!findChild(p, "toggle_panel_autohide").checked);
            p.flick.contentY = p.flick.contentHeight - p.flick.height;   // the Desktop section is below the fold
            wait(30);
            mouseClick(findChild(p, "toggle_panel_autohide"));
            compare(fake.called("SetPanelAutohide")[0].args[0], true);
            mouseClick(findChild(p, "toggle_dark_theme"));
            compare(fake.called("SetDarkTheme")[0].args[0], false);
        }

        function test_desktop_toggles_are_greyed_without_plasma() {
            fake.responses.GetDesktopPrefs = JSON.stringify({ panel_autohide: null, dark_theme: null });
            fake.responses.GetHostState = JSON.stringify({ connector: "DP-3", hdr: null, target_mode: null });
            const p = page("SystemPage.qml");
            verify(!findChild(p, "toggle_panel_autohide").enabled);
            verify(!findChild(p, "toggle_dark_theme").enabled);
            verify(!findChild(p, "toggle_display_hdr").enabled);
        }

        // ---- MultiFrame
        function test_multiframe_lists_screens_and_applies_a_layout() {
            const p = page("MultiScreenPage.qml");
            p.fn = "multiframe";
            compare(p.tiles.length, 2);
            compare(p.selectedOutput, "DP-3", "the monitor's own screen is selected");
            verify(findChild(p, "zone_2").current);
            compare(p.layoutName(2), "Left / right");
            compare(p.layoutName(null), "Custom");
            compare(p.layoutName(0), "None");
            mouseClick(findChild(p, "zone_13"));
            const c = fake.called("SetTileLayout");
            compare(c[0].args[0], "DP-3");
            compare(c[0].args[1], 13);
            verify(findChild(p, "zone_13").current, "the page shows the reply");
            wait(30);
            mouseClick(findChild(p, "screen_HDMI-A-1"));
            wait(30);
            mouseClick(findChild(p, "zone_0"));
            compare(fake.called("SetTileLayout")[1].args[0], "HDMI-A-1");
            compare(fake.called("SetTileLayout")[1].args[1], 0);
        }

        function test_multiframe_without_kwin_says_so() {
            fake.responses.GetTileLayouts = { error: "KWin scripting (is this a Plasma 6 session?)" };
            const p = page("MultiScreenPage.qml");
            p.fn = "multiframe";
            verify(p.tilesError.indexOf("KWin") >= 0);
        }

        // ---- Diagnostics
        function test_diagnostics_collect_everything_without_serials() {
            const p = page("AppSettingsPage.qml", { backend: fake, chooser: chooser });
            fake.connected = true;
            mouseClick(findChild(p, "export_diagnostics"));
            const url = "file:///tmp/sumi-test.json";
            const t = fake.written[url];
            verify(t !== undefined, "a file was written");
            verify(t.indexOf("App version: 9.9.9") >= 0);
            verify(t.indexOf("== ListMonitors ==") >= 0);
            verify(t.indexOf("== Problems ==") >= 0);
            verify(t.indexOf("PG32UCDM3@i2c-5 Capabilities") >= 0);
            verify(t.indexOf("PG32UCDM3@i2c-5 GetState") >= 0);
            verify(t.indexOf("PG32UCDM3@i2c-5 GetHostState") >= 0);
            compare(fake.called("GetState")[0].args[1].length, 0, "the full state, every page");
            verify(!/serial|S\/N/i.test(t));
        }

        // ---- AI Visual
        function openAi() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_ai"));
            wait(30);
            return p;
        }

        function setAi(fields) {
            const c = baseConfig();
            Object.assign(c.ai_visual, fields);
            host.config = c;
            fake.responses.GetConfig = JSON.stringify(c);
        }

        function lastConfig() {
            return JSON.parse(fake.called("SetConfig").pop().args[0]);
        }

        function test_the_ai_tile_comes_first_and_is_checked_while_ai_is_on() {
            const p = page("GameVisualPage.qml");
            const ai = findChild(p, "mode_ai");
            const first = findChild(p, "mode_RTS/RPG");
            verify(ai.mapToItem(p, 0, 0).x < first.mapToItem(p, 0, 0).x, "AI Visual leads the strip");
            verify(!ai.checked);
            verify(findChild(p, "mode_FPS").checked);
            setAi({ enabled: true });
            verify(ai.checked);
            verify(!findChild(p, "mode_FPS").checked, "the tile that is on is the AI tile");
        }

        function test_clicking_the_ai_tile_opens_the_panel_and_enables_nothing() {
            const p = openAi();
            verify(p.aiOpen);
            compare(fake.called("SetConfig").length, 0);
            verify(!findChild(p, "slider_brightness").visible, "the panel replaces the cards");
            verify(!findChild(p, "reset").visible);
            verify(findChild(p, "toggle_ai") !== null);
            verify(findChild(p, "mode_ai").checked, "the AI tile shows it is the open one");
            mouseClick(findChild(p, "ai_close"));
            verify(!p.aiOpen);
            verify(findChild(p, "slider_brightness").visible, "Close brings the cards back");
        }

        function test_another_tile_ends_ai_visual_with_one_config_write_and_applies_the_mode() {
            setAi({ enabled: true });
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "mode_Cinema"));
            compare(fake.called("SetConfig").length, 1);
            compare(lastConfig().ai_visual.enabled, false);
            compare(lastCall().feature, "gamevisual");
            compare(lastCall().value, 1);
            verify(!p.aiOpen);
        }

        function test_another_tile_with_ai_off_writes_no_config() {
            const p = openAi();
            mouseClick(findChild(p, "mode_Cinema"));
            compare(fake.called("SetConfig").length, 0);
            verify(!p.aiOpen, "picking a mode leaves the AI panel");
            compare(lastCall().value, 1);
        }

        function test_the_consent_dialog_comes_once() {
            const p = openAi();
            const t = findChild(p, "toggle_ai");
            verify(t.enabled);
            mouseClick(t);
            compare(fake.called("SetConfig").length, 0, "nothing is enabled before the answer");
            compare(store.confirm.text, "AI Visual looks at what's on this monitor about once a second to pick a GameVisual mode. The picture stays in memory on this PC; nothing is saved or sent anywhere.");
            store.answer(false);
            compare(fake.called("SetConfig").length, 0);
            mouseClick(t);
            store.answer(true);
            compare(lastConfig().ai_visual.enabled, true);
            compare(fake.prefs.ai_visual_consent, "1");
            mouseClick(t);
            compare(store.confirm, null);
            compare(lastConfig().ai_visual.enabled, false);
            mouseClick(t);
            compare(store.confirm, null, "no second explanation");
            compare(lastConfig().ai_visual.enabled, true);
        }

        function test_without_the_model_the_switch_is_off_and_the_download_row_is_first() {
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: false }));
            const p = openAi();
            verify(!findChild(p, "toggle_ai").enabled);
            const d = findChild(p, "download_ai");
            verify(d.visible);
            compare(d.text, "Download (352 MB)");
            verify(!findChild(p, "ai_installed").visible);
            mouseClick(d);
            compare(fake.called("DownloadAiModel").length, 0, "asks first");
            compare(store.confirm.text, "Download the AI Visual model? vision_model.onnx, 352 MB, from huggingface.co (OpenAI CLIP ViT-B/32, MIT licence). It is checked against its published SHA-256 and kept in ~/.local/share/sumi/models.");
            store.answer(false);
            compare(fake.called("DownloadAiModel").length, 0);
            mouseClick(d);
            store.answer(true);
            compare(fake.called("DownloadAiModel").length, 1);
            compare(fake.called("DownloadAiModel")[0].notify, true, "a refusal is shown");
        }

        function test_download_progress_then_installed() {
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: false, state: { download: [100000000, 352000000] } }));
            const p = openAi();
            const bar = findChild(p, "download_progress");
            verify(bar.visible);
            compare(bar.valueText, "100 of 352 MB");
            verify(!findChild(p, "download_ai").visible);
            verify(!findChild(p, "toggle_ai").enabled);
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: true }));
            p.aiPanel.host.loadAi();
            verify(!bar.visible);
            verify(findChild(p, "ai_installed").visible);
            compare(findChild(p, "ai_installed").text, "AI model installed (352 MB)");
            verify(findChild(p, "toggle_ai").enabled);
            verify(findChild(p, "remove_ai").visible);
        }

        function test_remove_the_model_asks_and_turns_ai_off() {
            setAi({ enabled: true });
            const p = openAi();
            mouseClick(findChild(p, "remove_ai"));
            compare(fake.called("RemoveAiModel").length, 0);
            store.answer(true);
            compare(fake.called("RemoveAiModel").length, 1);
            compare(lastConfig().ai_visual.enabled, false);
        }

        function test_status_is_polled_only_while_the_panel_is_open() {
            const p = openAi();
            p.aiPanel.pollInterval = 60;
            const before = fake.called("AiStatus").length;
            wait(300);
            verify(fake.called("AiStatus").length >= before + 3, "polled while visible");
            mouseClick(findChild(p, "ai_close"));
            wait(30);
            const after = fake.called("AiStatus").length;
            wait(300);
            compare(fake.called("AiStatus").length, after, "no polling once it is closed");
        }

        function test_mode_checkboxes_edit_the_modes_and_keep_unknown_ones() {
            const p = openAi();
            verify(findChild(p, "ai_mode_fps").checked);
            mouseClick(findChild(p, "ai_mode_fps"));
            let m = lastConfig().ai_visual.modes;
            verify(m.indexOf("fps") < 0);
            verify(m.indexOf("cinema") >= 0, "a mode the list does not show is kept");
            wait(30);
            mouseClick(findChild(p, "ai_mode_fps"));
            verify(lastConfig().ai_visual.modes.indexOf("fps") >= 0);
        }

        function test_notifications_checkbox() {
            const p = openAi();
            verify(findChild(p, "ai_notify").checked);
            mouseClick(findChild(p, "ai_notify"));
            compare(lastConfig().ai_visual.notify, false);
        }

        function test_fallback_radios_follow_the_config() {
            const p = openAi();
            verify(findChild(p, "ai_keep").checked);
            verify(!findChild(p, "ai_assign").checked);
            verify(!findChild(p, "ai_fallback").enabled);
            mouseClick(findChild(p, "ai_assign"));
            compare(lastConfig().ai_visual.fallback, "user");
            verify(findChild(p, "ai_assign").checked);
            verify(findChild(p, "ai_fallback").enabled);
            findChild(p, "ai_fallback").activated(1);
            compare(lastConfig().ai_visual.fallback, "fps");
            wait(30);
            mouseClick(findChild(p, "ai_keep"));
            verify(!("fallback" in lastConfig().ai_visual), "Remain unchanged = no fallback");
        }

        function test_ai_reset_restores_the_defaults_but_not_the_switch() {
            setAi({ enabled: true, notify: false, modes: ["fps"], fallback: "racing" });
            const p = openAi();
            mouseClick(findChild(p, "ai_reset"));
            const a = lastConfig().ai_visual;
            compare(a.notify, true);
            compare(a.modes.join(","), "rts_rpg,fps,scenery,racing,srgb,moba");
            verify(!("fallback" in a));
            compare(a.enabled, true, "Reset leaves AI Visual on");
        }

        function test_now_seeing_shows_the_selected_monitors_verdict() {
            const st = {};
            st["PG32UCDM3@i2c-5"] = seeing();
            st["OTHER@i2c-9"] = seeing({ verdict: { scores: [["office", 99]], is_game: 0.01, confidence: 0.99, target: "srgb" } });
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: true, state: { monitors: st } }));
            setAi({ enabled: true });
            const p = openAi();
            compare(findChild(p, "scene_0").label, "FPS");
            compare(Math.round(findChild(p, "scene_0").value), 61);
            compare(findChild(p, "scene_1").label, "Racing");
            compare(findChild(p, "scene_2").label, "Office");
            verify(findChild(p, "scene_3") === null, "only the top three");
            compare(Math.round(findChild(p, "gauge_game").value), 83);
            compare(Math.round(findChild(p, "gauge_confidence").value), 72);
            compare(findChild(p, "ai_target").text, "Mode: FPS");
            verify(!findChild(p, "ai_paused").visible);
            verify(!findChild(p, "ai_error").visible);
        }

        function test_preview_while_ai_is_off() {
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: true }));
            fake.responses.AiClassifyNow = JSON.stringify({
                observation: {}, frame: [2560, 1440], ms: {},
                verdict: { scores: [["office", 98], ["rpg", 1]], is_game: 0.02, confidence: 0.98, top: "office", target: "srgb" }
            });
            setAi({ enabled: false });
            const p = openAi();
            tryVerify(() => fake.called("AiClassifyNow").length > 0, 3000, "the panel previews");
            const c = fake.called("AiClassifyNow")[0];
            compare(c.args[0], "PG32UCDM3@i2c-5");
            tryCompare(findChild(p, "ai_target"), "text", "Mode: sRGB");
            compare(Math.round(findChild(p, "gauge_confidence").value), 98);
            verify(findChild(p, "ai_idle").text.indexOf("Preview") === 0);
        }

        function test_no_preview_while_ai_is_on() {
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: true }));
            setAi({ enabled: true });
            openAi();
            wait(300);
            verify(fake.called("AiClassifyNow").length === 0, "AI on: the daemon's verdict is shown, no preview captures");
        }

        function test_now_seeing_says_when_nothing_changes_is_paused_or_failing() {
            const st = {};
            st["PG32UCDM3@i2c-5"] = seeing({ paused: "HDR is on", verdict: { scores: [["fps", 50]], is_game: 0.5, confidence: 0.4, target: null } });
            fake.responses.AiStatus = JSON.stringify(aiStatus({ installed: true, state: { monitors: st, error: "capture failed: no screen cast" } }));
            setAi({ enabled: true });
            const p = openAi();
            compare(findChild(p, "ai_target").text, "Mode: unchanged");
            verify(findChild(p, "ai_paused").visible);
            compare(findChild(p, "ai_paused").text, "Paused: HDR is on");
            verify(findChild(p, "ai_error").visible);
            compare(findChild(p, "ai_error").text, "capture failed: no screen cast");
        }

        function test_an_older_daemon_without_ai_is_explained() {
            fake.responses.AiStatus = { error: "No such method 'AiStatus'" };
            const p = openAi();
            verify(!findChild(p, "toggle_ai").enabled);
            verify(!findChild(p, "download_ai").visible);
        }

        function test_app_tweaker_default_row_follows_ai_visual() {
            let p = page("AppTweakerPage.qml");
            verify(findChild(p, "mode_default").visible);
            verify(!findChild(p, "mode_default_ai").visible);
            verify(!findChild(p, "ai_note").visible);
            setAi({ enabled: true });
            p = page("AppTweakerPage.qml");
            verify(!findChild(p, "mode_default").visible, "the mode combo is replaced");
            verify(findChild(p, "mode_default_ai").visible);
            verify(!findChild(p, "mode_default_ai").enabled);
            compare(findChild(p, "mode_default_ai").currentText, "AI Visual");
            compare(findChild(p, "ai_note").text, "Unlisted apps follow AI Visual");
        }

        // ---- The mode strip
        function visibleTiles(p) {
            let n = 0;
            for (const name of ["mode_ai", "mode_RTS/RPG", "mode_FPS", "mode_Cinema", "mode_Scenery", "mode_Racing", "mode_sRGB", "mode_MOBA", "mode_User", "mode_Night Vision", "mode_hdr", "mode_hdr_dv"]) {
                const t = findChild(p, name);
                if (t !== null && t.visible) {
                    n++;
                }
            }
            return n;
        }

        function test_strip_tiles_are_vendor_sized_and_arrows_step_a_page_at_a_time() {
            const p = page("GameVisualPage.qml");
            const ai = findChild(p, "mode_ai");
            const rts = findChild(p, "mode_RTS/RPG");
            compare(ai.width, 108);
            compare(ai.height, 104);
            compare(rts.x - ai.x, 123, "tiles are on a 123 px pitch");
            verify(findChild(p, "strip_left").visible, "eleven tiles do not fit in the strip");
            verify(findChild(p, "strip_right").visible);
            verify(!findChild(p, "strip_left").enabled, "nothing to the left of the first page");
            verify(findChild(p, "strip_right").enabled);
            compare(findChild(p, "strip_left").width, 34);
            compare(findChild(p, "strip_left").height, 56);
            compare(visibleTiles(p), 7, "whole tiles only, a page of seven");
            verify(!findChild(p, "mode_hdr").visible);
            mouseClick(findChild(p, "strip_right"));
            compare(p.first, 5, "the last page ends at the last tile");
            verify(findChild(p, "mode_hdr").visible);
            verify(!findChild(p, "mode_ai").visible);
            verify(findChild(p, "strip_left").enabled);
            verify(!findChild(p, "strip_right").enabled, "nothing to the right of the last page");
            compare(visibleTiles(p), 7);
            mouseClick(findChild(p, "strip_left"));
            compare(p.first, 0);
            verify(findChild(p, "mode_ai").visible);
        }

        function test_strip_wheel_steps_one_tile_at_a_time() {
            const p = page("GameVisualPage.qml");
            const wheel = findChild(p, "strip_wheel");
            mouseWheel(wheel, 100, 50, 0, -120);
            compare(p.first, 1);
            mouseWheel(wheel, 100, 50, 0, -120);
            compare(p.first, 2);
            mouseWheel(wheel, 100, 50, 0, 120);
            compare(p.first, 1);
            for (let i = 0; i < 10; i++) {
                mouseWheel(wheel, 100, 50, 0, -120);
            }
            compare(p.first, 5, "stops at the end");
        }

        function test_strip_keeps_the_selected_tile_in_view() {
            const st = fixture();
            st.context.mode = 4;
            st.features.find(f => f.id === "gamevisual").value = 4;   // User: the ninth tile
            store.ingest("game_visual", st);
            let p = page("GameVisualPage.qml");
            verify(findChild(p, "mode_User").visible);
            verify(findChild(p, "mode_User").checked);
            compare(p.first, 2);
            // HDR on: the HDR tile is the selected one
            const hdr = JSON.parse(JSON.stringify(st));   // a fresh state, as the daemon sends it
            hdr.context.hdr = true;
            hdr.features.find(f => f.id === "hdr_mode").value = 258;
            store.ingest("game_visual", hdr);
            wait(30);
            verify(findChild(p, "mode_hdr").visible);
            compare(p.first, 5);
        }

        function test_strip_has_no_arrows_when_the_tiles_fit() {
            const st = fixture();
            const gv = st.features.find(f => f.id === "gamevisual");
            gv.options = gv.options.slice(0, 4);
            store.ingest("game_visual", st);
            const p = page("GameVisualPage.qml");
            verify(!findChild(p, "strip_left").visible);
            verify(!findChild(p, "strip_right").visible);
            compare(visibleTiles(p), 7, "AI Visual, four modes, HDR and Dolby Vision: all of them");
            compare(findChild(p, "mode_ai").x, findChild(p, "mode_RTS/RPG").x - 123);
        }

        function test_strip_arrow_input_does_not_write_anything() {
            const p = page("GameVisualPage.qml");
            mouseClick(findChild(p, "strip_right"));
            mouseClick(findChild(p, "strip_left"));
            compare(fake.calls.length, 0);
            compare(fake.called("SetConfig").length, 0);
        }

        // ---- Daemon labels go through i18n
        function test_daemon_labels_are_translated_for_display_but_logic_keeps_the_raw_ones() {
            Tr.catalogue = { "Cinema": "シネマ", "FPS": "エフピーエス", "%1 mode settings": "%1 モード設定", "Level 3": "レベル3", "Shadow Boost": "シャドウ" };
            const p = page("GameVisualPage.qml");
            const cinema = findChild(p, "mode_Cinema");
            compare(cinema.caption, "シネマ");
            compare(cinema.icon, "cinema", "icons are chosen from the raw label");
            verify(findChild(p, "mode_FPS").checked, "the current mode is still found");
            compare(findChild(p, "mode_FPS").caption, "エフピーエス");
            verify(p.currentLabel === "FPS", "export names keep the raw label");
            compare(findChild(p, "combo_shadow_boost").currentText, "レベル3");
            mouseClick(cinema);
            compare(lastCall().value, 1, "clicking still writes the daemon's value");
        }

        function test_hdr_pills_and_dolby_prefix() {
            Tr.catalogue = { "Gaming HDR": "ゲーミングHDR", "Dolby Vision Bright": "Dolby Vision ブライト", "Dolby Vision": "Dolby Vision" };
            const st = fixture();
            st.context.hdr = true;
            st.features.find(f => f.id === "hdr_mode").value = 258;
            store.ingest("game_visual", st);
            const p = page("GameVisualPage.qml");
            compare(findChild(p, "hdr_258").text, "ゲーミングHDR");
            mouseClick(findChild(p, "mode_hdr_dv"));
            compare(findChild(p, "hdr_517").text, "ブライト", "the family name the row already shows is dropped");
        }

        function test_language_names_user_data_and_errors_are_not_translated() {
            Tr.catalogue = { "English": "Anglais", "Dolphin": "Dauphin", "DisplayPort": "DP-fr" };
            const sys = page("SystemPage.qml");
            compare(findChild(sys, "combo_language").currentText, "English", "native language names stay as they are");
            compare(findChild(sys, "combo_input").currentText, "DP-fr", "other options are translated");
            const tweaker = page("AppTweakerPage.qml");
            compare(findChild(tweaker, "app_0").currentText, "Dolphin", "app names are the user's own data");
        }

        function test_other_pages_translate_their_daemon_labels() {
            Tr.catalogue = { "Blue Dot": "ブルードット", "PIP top right": "PIP 右上", "USB upstream B": "USB 上流B", "Standard": "標準", "EZ OSD: up": "EZ OSD: 上" };
            const gp = page("GamePlusPage.qml");
            compare(findChild(gp, "opt_7").label, "ブルードット");
            const ms = page("MultiScreenPage.qml");
            compare(findChild(ms, "layout_1").label, "PIP 右上");
            ms.fn = "kvm";
            wait(30);
            compare(findChild(ms, "kvm_2").text, "USB 上流B");
            const sys = page("SystemPage.qml");
            compare(findChild(sys, "power_0").text, "標準");
            fake.responses.Hotkeys = JSON.stringify([{ action: "osd_up", label: "EZ OSD: up", keys: [] }]);
            const hk = page("HotKeyPage.qml");
            compare(hk.bindings[0].label, "EZ OSD: up", "the data is raw");
            verify(findChild(hk, "test_osd_up") !== null);
        }

        // ---- Language
        function test_the_language_table_matches_the_monitor_menu_codes() {
            const want = { 1: "zh_TW", 2: "en", 3: "fr", 4: "de", 5: "it", 6: "ja", 7: "ko", 8: "pt", 9: "ru", 10: "es", 12: "tr", 13: "zh_CN",
                17: "hr", 18: "cs", 20: "nl", 26: "hu", 30: "pl", 31: "ro", 35: "th", 36: "uk", 37: "vi", 38: "fa", 39: "id" };
            compare(Languages.list.length, 23);
            for (const v in want) {
                compare(Languages.localeFor(Number(v)), want[v], "0xCC = " + v);
            }
            compare(Languages.localeFor(0), "en", "0 is English");
            compare(Languages.localeFor(11), "", "an unknown code maps to nothing");
        }

        function test_language_combo_lists_system_osd_and_every_language() {
            const p = page("AppSettingsPage.qml", { backend: fake });
            const c = findChild(p, "combo_language");
            compare(c.model.length, 25);
            compare(c.model[0], "System language");
            compare(c.model[1], "Same as the monitor menu");
            verify(c.model.indexOf("日本語") >= 0);
            verify(c.model.indexOf("Deutsch") >= 0);
            compare(c.currentIndex, 0, "the default is the system language");
            verify(!findChild(p, "restart_row").visible);
        }

        function test_choosing_a_language_stores_it_and_asks_for_a_restart() {
            const p = page("AppSettingsPage.qml", { backend: fake });
            const c = findChild(p, "combo_language");
            c.activated(c.model.indexOf("日本語"));
            compare(fake.prefs.language, "ja");
            verify(findChild(p, "restart_row").visible);
            compare(findChild(p, "restart_row").title, "Restart Sumi to change the language");
            compare(c.currentIndex, c.model.indexOf("日本語"));
            compare(fake.restarts, 0, "nothing restarts by itself");
            wait(30);
            mouseClick(findChild(p, "restart"));
            compare(fake.restarts, 1);
            c.activated(1);
            compare(fake.prefs.language, "osd");
            c.activated(0);
            compare(fake.prefs.language, "");
            c.activated(c.model.indexOf("简体中文"));
            compare(fake.prefs.language, "zh_CN");
        }

        function test_the_saved_language_is_shown() {
            fake.prefs.language = "de";
            const p = page("AppSettingsPage.qml", { backend: fake });
            compare(findChild(p, "combo_language").currentText, "Deutsch");
            fake.prefs.language = "osd";
            const q = page("AppSettingsPage.qml", { backend: fake });
            compare(findChild(q, "combo_language").currentText, "Same as the monitor menu");
        }

        function test_the_monitor_menu_language_is_remembered_when_seen() {
            const st = fixture();
            const lang = st.features.find(f => f.id === "language");
            compare(lang.value, 2);
            store.ingest("system", st);
            compare(fake.prefs.osd_locale, "en");
            const n = fake.prefWrites.length;
            store.ingest("system", st);
            compare(fake.prefWrites.length, n, "unchanged: no second write");
            lang.value = 6;
            store.ingest("system", st);
            compare(fake.prefs.osd_locale, "ja");
            lang.value = 0;
            store.ingest("system", st);
            compare(fake.prefs.osd_locale, "en", "0 reads as English");
            lang.value = 11;
            store.ingest("system", st);
            compare(fake.prefs.osd_locale, "en", "an unknown code leaves it alone");
        }

        function test_host_reads_the_menu_language_with_one_vcp_read() {
            fake.responses.GetVcp = "[13,40]";
            host.cacheOsdLanguage("PG32UCDM3@i2c-5");
            compare(fake.called("GetVcp").length, 1);
            compare(fake.called("GetVcp")[0].args[1], 204);
            compare(fake.prefs.osd_locale, "zh_CN");
            fake.responses.GetVcp = { error: "DDC busy" };
            host.cacheOsdLanguage("PG32UCDM3@i2c-5");
            compare(fake.prefs.osd_locale, "zh_CN", "a failed read changes nothing");
        }

        // ---- Help
        property var opened: []
        function test_user_manual_opens_the_installed_readme_or_the_support_page() {
            const urls = [];
            const p = page("AppSettingsPage.qml", { backend: fake, openUrl: u => urls.push(u) });
            mouseClick(findChild(p, "open_manual"));
            compare(urls[0], "file:///home/test/.local/share/doc/sumi/README.md");
            fake.manual = "https://www.asus.com/support/";
            mouseClick(findChild(p, "open_manual"));
            compare(urls[1], "https://www.asus.com/support/");
            verify(findChild(p, "open_manual").parent !== null);
        }

        // ---- Text fitting (see TextCheck.qml) ------------------------------------------------
        // Every page with a language that needs 45 % more room than English: nothing may be cut,
        // overflow its control or be drawn over another text.
        function checkFits(file, props, label) {
            const p = page(file, props);
            wait(80);
            const n = textCheck.run(p, label);
            verify(n === 0, label + ": " + textCheck.findings.map(f => f.kind + " \"" + f.text + "\" " + f.detail).join("; "));
        }

        function test_the_check_reports_text_that_does_not_fit() {
            const probe = createTemporaryObject(probeComponent, root, {});
            wait(30);
            textCheck.run(probe, "probe");
            const kinds = textCheck.findings.map(f => f.kind + ":" + f.text).sort();
            compare(kinds, ["elided:A very long label", "outside:Pushed out", "overlap:Overlapping one", "taller:wrapped words wrapped words wrapped words wrapped"].sort());
            textCheck.run(createTemporaryObject(fineComponent, root, {}), "fine");
            compare(textCheck.findings.length, 0, "short texts in roomy boxes are fine");
        }

        function test_every_page_fits_english() {
            Tr.stretch = 1;
            for (const f of ["GameVisualPage.qml", "GamePlusPage.qml", "OledCarePage.qml", "MultiScreenPage.qml", "SystemPage.qml", "UserCenterPage.qml", "AppSettingsPage.qml", "HotKeyPage.qml", "AppTweakerPage.qml"]) {
                checkFits(f, f === "AppSettingsPage.qml" ? { backend: fake } : (f === "UserCenterPage.qml" ? { backend: fake } : {}), f + " (English)");
            }
            for (const st of ["unchecked", "checking", "uptodate", "available", "downloading", "saved", "error"]) {
                const up = fwPage();
                up.fwInfo = { latest: "MCM105" };
                up.fwPath = "/home/u/Downloads/ASUS_PG32UCDM3_MCM105.zip";
                up.fwError = "Couldn't reach ASUS's download server";
                up.fwTotal = 20000000;
                up.fwDone = 5000000;
                up.fwState = st;
                wait(60);
                verify(textCheck.run(up, "firmware " + st) === 0, "firmware " + st + ": " + textCheck.findings.map(f => f.kind + " \"" + f.text + "\" " + f.detail).join("; "));
            }
        }

        function test_every_page_fits_a_language_that_needs_room() {
            Tr.stretch = 1.45;
            try {
                for (const f of ["GameVisualPage.qml", "GamePlusPage.qml", "OledCarePage.qml", "MultiScreenPage.qml", "SystemPage.qml", "UserCenterPage.qml", "AppSettingsPage.qml", "HotKeyPage.qml", "AppTweakerPage.qml"]) {
                    checkFits(f, f === "AppSettingsPage.qml" || f === "UserCenterPage.qml" ? { backend: fake } : {}, f + " (+45 %)");
                }
                for (const st of ["unchecked", "available", "downloading", "saved", "error"]) {
                    const up = fwPage();
                    up.fwInfo = { latest: "MCM105" };
                    up.fwPath = "/home/u/Downloads/ASUS_PG32UCDM3_MCM105.zip";
                    up.fwError = "Couldn't reach ASUS's download server";
                    up.fwTotal = 20000000;
                    up.fwDone = 5000000;
                    up.fwState = st;
                    wait(60);
                    verify(textCheck.run(up, "firmware " + st + " (+45 %)") === 0, "firmware " + st + ": " + textCheck.findings.map(f => f.kind + " \"" + f.text + "\" " + f.detail).join("; "));
                }
                for (const v of [0x0102, 0x0206]) {
                    const h = hdrState(v);
                    h.pickedFamily = v >> 8;
                    wait(80);
                    verify(textCheck.run(h, "hdr " + v + " (+45 %)") === 0, "HDR strip: " + textCheck.findings.map(f => f.kind + " \"" + f.text + "\" " + f.detail).join("; "));
                }
                const gv = page("GameVisualPage.qml");
                gv.aiOpen = true;
                wait(80);
                verify(textCheck.run(gv, "ai panel (+45 %)") === 0, "AI panel: " + textCheck.findings.map(f => f.kind + " \"" + f.text + "\" " + f.detail).join("; "));
            } finally {
                Tr.stretch = 1;
            }
        }

        function test_buttons_grow_with_their_label_and_keep_their_minimum_width() {
            const a = createTemporaryObject(buttonComponent, root, { text: "OK" });
            compare(a.width, 136, "short labels keep the usual width");
            const b = createTemporaryObject(buttonComponent, root, { text: "Tasten in den Systemeinstellungen festlegen" });
            verify(b.width > 250, "a long label widens the button: " + b.width);
            const c = createTemporaryObject(buttonComponent, root, { text: "Tasten in den Systemeinstellungen festlegen", minWidth: 250 });
            compare(c.width, b.width, "minWidth is only a minimum");
            const d = createTemporaryObject(buttonComponent, root, { text: "OK", minWidth: 250 });
            compare(d.width, 250);
        }

        function test_toggles_widen_for_long_on_off_words() {
            const t = createTemporaryObject(toggleComponent, root, {});
            compare(t.width, 52, "ON / OFF keep the usual size");
            Tr.catalogue = { "ON": "AKTIF", "OFF": "NONAKTIF" };
            const u = createTemporaryObject(toggleComponent, root, {});
            verify(u.width > 52, "NONAKTIF needs more room: " + u.width);
        }

        Component {
            id: probeComponent
            Item {
                width: 300
                height: 200
                Text { x: 0; y: 0; width: 80; text: "A very long label"; elide: Text.ElideRight }
                Item {
                    x: 0
                    y: 30
                    width: 60
                    height: 20
                    Text { x: 20; text: "Pushed out" }
                }
                Text { x: 0; y: 60; width: 60; height: 18; wrapMode: Text.WordWrap; text: "wrapped words wrapped words wrapped words wrapped" }
                Text { x: 0; y: 150; text: "Overlapping one" }
                Text { x: 10; y: 152; text: "Overlapping two" }
            }
        }
        Component {
            id: fineComponent
            Item {
                width: 300
                height: 100
                Text { x: 0; y: 0; width: 200; text: "Short"; elide: Text.ElideRight }
                Text { x: 0; y: 30; text: "Also short" }
            }
        }
        Component {
            id: buttonComponent
            RogButton {}
        }
        Component {
            id: toggleComponent
            RogToggle {}
        }
    }
}
