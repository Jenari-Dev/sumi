
    // ---- text-fit tour (harness only): visit every page / tab / dialog and run the check
    property int tourI: 0
    property string tourPhase: "start"
    property int tourTicks: 0
    property var tourItems: []
    property int tourSub: 0
    function tourDialog(label, d) { return { label: "dialog/" + label, dialog: d }; }
    property var tourSteps: tourPages()
    property bool tourDialogsAdded: false
    function tourPages() {
        const only = sumiBackend.env_var("SUMI_TOUR_ONLY");
        const all = tourPagesAll();
        return only === "" ? all : all.filter(x => x.label === only || x.label.indexOf(only + "/") === 0);
    }
    function tourPagesAll() {
        return [
            { label: "game_visual", page: "game_visual", combos: true },
            { label: "game_visual/dolby_vision", page: "game_visual", prep: p => { p.pickedFamily = 2; } },
            { label: "game_visual/ai", page: "game_visual", prep: p => { p.aiOpen = true; }, combos: true },
            { label: "game_visual/ai_nomodel", page: "game_visual", prep: p => { p.aiOpen = true; tourAi({ installed: false }); } },
            { label: "game_visual/ai_downloading", page: "game_visual", prep: p => { p.aiOpen = true; tourAi({ installed: false, state: { running: false, monitors: {}, download: [123000000, 352000000] } }); } },
            { label: "game_visual/ai_live", page: "game_visual", prep: p => { p.aiOpen = true; sumiHost.config = Object.assign({}, sumiHost.config, { ai_visual: Object.assign({}, sumiHost.config.ai_visual, { enabled: true, fallback: "user" }) }); const m = {}; m[sumiStore.monitorId] = { verdict: { scores: [["rpg", 61.2], ["scenery", 22.5], ["office", 9.1], ["racing", 4], ["moba", 2], ["fps", 1.2]], is_game: 0.834, confidence: 0.719, top: "rpg", target: "rts_rpg" }, last_sent: "rts_rpg", paused: "an App Tweaker rule covers the focused app" }; tourAi({ installed: true, state: { running: true, monitors: m, error: "capture failed: no screen cast" } }); }, undo: p => { sumiHost.config = Object.assign({}, sumiHost.config, { ai_visual: Object.assign({}, sumiHost.config.ai_visual, { enabled: false }) }); } },
            { label: "game_plus/crosshair", page: "game_plus", prep: p => { p.picked = "crosshair"; } },
            { label: "game_plus/timer", page: "game_plus", prep: p => { p.picked = "timer"; } },
            { label: "game_plus/fps_counter", page: "game_plus", prep: p => { p.picked = "fps_counter"; } },
            { label: "game_plus/display_alignment", page: "game_plus", prep: p => { p.picked = "display_alignment"; } },
            { label: "oled_care", page: "oled_care", combos: true },
            { label: "multi_screen/pip", page: "multi_screen", prep: p => { p.fn = "pip"; }, combos: true },
            { label: "multi_screen/multiframe", page: "multi_screen", prep: p => { p.fn = "multiframe"; } },
            { label: "multi_screen/kvm", page: "multi_screen", prep: p => { p.fn = "kvm"; } },
            { label: "hotkey", page: "hotkey", combos: true },
            { label: "hotkey/info", page: "hotkey", prep: p => { p.infoDismissed = false; } },
            { label: "app_tweaker", page: "app_tweaker", combos: true },
            { label: "app_tweaker/ai", page: "app_tweaker", prep: p => { sumiHost.config = Object.assign({}, sumiHost.config, { ai_visual: Object.assign({}, sumiHost.config.ai_visual, { enabled: true }) }); }, undo: p => { sumiHost.config = Object.assign({}, sumiHost.config, { ai_visual: Object.assign({}, sumiHost.config.ai_visual, { enabled: false }) }); } },
            { label: "app_tweaker/conflict", page: "app_tweaker", prep: p => { const c = JSON.parse(JSON.stringify(sumiHost.config)); c.app_sync.rule.push({ app: "dolphin", name: "Dolphin", mode: 3 }); sumiHost.config = c; } },
            { label: "system", page: "system", combos: true },
            { label: "user_center", page: "user_center" },
            { label: "app_settings", page: "app_settings", combos: true },
            { label: "app_settings/changed", page: "app_settings", prep: p => { p.languageChanged = true; } },
            { label: "main/readonly", page: "game_visual", prep: p => { sumiBackend.readonly = true; }, undo: p => { sumiBackend.readonly = false; } },
            { label: "main/offline", page: "game_visual", prep: p => { sumiBackend.connected = false; pageLoader.visible = false; }, undo: p => { sumiBackend.connected = true; pageLoader.visible = true; } },
            { label: "main/no_monitor", page: "game_visual", prep: p => { win.monitors = []; sumiBackend.monitors = "[]"; }, },
        ];
    }
    function tourAi(over) {
        const st = { installed: true, prompts: true, model: { name: "vision_model.onnx", url: "https://huggingface.co/x", size: 352000000, sha256: "00" }, state: { running: true, monitors: {} } };
        Object.assign(st, over);
        const a = Object.assign({}, sumiBackend.api);
        a.AiStatus = st;
        sumiBackend.api = a;
    }
    function tourDialogs() {
        const t = Tr.i18n;
        const dialogs = [];
        for (const id of Object.keys(sumiStore.features)) {
            for (const v of [1, 5, 514, 258]) {
                const r = sumiStore.riskInfo(id, v);
                if (r) dialogs.push(tourDialog(id + "=" + v, r));
            }
        }
        for (const a of ["kvm_toggle", "power_saving_toggle", "pip_toggle", "hdr_toggle"]) {
            const r = sumiStore.actionRisk(a);
            if (r) dialogs.push(tourDialog(a, r));
        }
        dialogs.push(tourDialog("hdr_off", { title: t("Turn HDR off"), ok: t("Switch"), text: t("Switching to %1 turns HDR off for this monitor.", "Dolby Vision Bright") }));
        dialogs.push(tourDialog("ai_consent", { title: t("AI Visual"), ok: t("Turn on"), text: t("AI Visual looks at what's on this monitor about once a second to pick a GameVisual mode. The picture stays in memory on this PC; nothing is saved or sent anywhere.") }));
        dialogs.push(tourDialog("ai_download", { title: t("Download the AI model"), ok: t("Download"), text: t("Download the AI Visual model? %1, %2 MB, from huggingface.co (OpenAI CLIP ViT-B/32, MIT licence). It is checked against its published SHA-256 and kept in ~/.local/share/sumi/models.", "vision_model.onnx", 352) }));
        dialogs.push(tourDialog("ai_remove", { title: t("Remove the AI model"), ok: t("Remove"), text: t("Remove the AI Visual model from this PC? You can download it again later.") }));
        dialogs.push(tourDialog("switch_mode", { title: t("Switch mode"), ok: t("Switch and apply"), text: t("This file is for %1. Switch to it and apply?", "Dolby Vision Bright") }));
        dialogs.push(tourDialog("not_responding", { title: t("Monitor not responding"), ok: t("OK"), notice: true, showCancel: false, text: t("%1 was found but doesn't answer DDC/CI. Turn on DDC/CI in the monitor's menu (System Setup) and check that /dev/i2c-* is accessible.", "PG32UCDM3") }));
        dialogs.push(tourDialog("import_done", { title: t("Settings imported"), ok: t("OK"), notice: true, showCancel: false, large: true, text: t("Nothing needed to change.") }));
        dialogs.push(tourDialog("import_failed", { title: t("Import failed"), ok: t("OK"), notice: true, showCancel: false, text: t("The file is not a settings file.") }));
        return dialogs;
    }

    Timer {
        id: tourTimer
        interval: 250
        repeat: true
        running: win.refreshed && sumiBackend.env_var("SUMI_TOUR") !== ""
        onTriggered: win.tourTick()
    }

    function tourComboList(p) {
        const out = [];
        const walk = it => {
            if (!it) return;
            if (it.model !== undefined && it.labelAt !== undefined && it.open !== undefined && it.visible) out.push(it);
            const k = it.children || [];
            for (let i = 0; i < k.length; i++) walk(k[i]);
        };
        walk(p);
        return out;
    }

    function tourTick() {
        if (tourI >= tourSteps.length && !tourDialogsAdded && sumiBackend.env_var("SUMI_TOUR_ONLY") === "") {
            tourDialogsAdded = true;
            tourSteps = tourSteps.concat(tourDialogs());
        }
        const step = tourSteps[tourI];
        if (!step) {
            console.log("TOUR end at", tourI, tourSteps.length);
            Qt.quit();
            return;
        }
        switch (tourPhase) {
        case "start":
            if (step.dialog) {
                sumiStore.confirm = Object.assign({ fn: null, showCancel: true, large: false, notice: false }, step.dialog);
                tourTicks = 3;
                tourPhase = "dialog";
            } else {
                if (win.page === step.page) win.requestPage(); else win.page = step.page;
                tourPhase = "load";
                tourTicks = 0;
            }
            break;
        case "dialog":
            if (--tourTicks <= 0) {
                textCheck.run(confirmDialog, step.label);
                sumiStore.confirm = null;
                tourI++;
                tourPhase = "start";
            }
            break;
        case "load": {
            const ready = pageInfo.key === "" ? refreshed : sumiStore.isLoaded(pageInfo.key);
            if (ready && pageLoader.item) {
                tourPhase = "settle";
                tourTicks = 5;
            } else if (++tourTicks > 80) {
                console.log("TOUR timeout loading " + step.label);
                tourI++;
                tourPhase = "start";
            }
            break;
        }
        case "settle":
            if (--tourTicks <= 0) {
                if (step.prep) step.prep(pageLoader.item);
                tourPhase = "prep";
                tourTicks = 4;
            }
            break;
        case "prep":
            if (--tourTicks <= 0) {
                textCheck.run(scaler, step.label);
                tourItems = step.combos ? tourComboList(pageLoader.item) : [];
                tourSub = 0;
                tourPhase = tourItems.length > 0 ? "combo_open" : "done";
            }
            break;
        case "combo_open":
            if (tourSub >= tourItems.length) {
                tourPhase = "done";
                break;
            }
            tourItems[tourSub].open();
            tourTicks = 3;
            tourPhase = "combo_check";
            break;
        case "combo_check":
            if (--tourTicks <= 0) {
                textCheck.run(overlayLayer, step.label + "/combo:" + (tourItems[tourSub].objectName || tourSub));
                tourItems[tourSub].close();
                tourSub++;
                tourPhase = "combo_open";
            }
            break;
        case "done":
            if (step.undo) step.undo(pageLoader.item);
            tourI++;
            tourPhase = "start";
            break;
        }
    }
