pragma ComponentBehavior: Bound

import QtQuick

// GameVisual: mode strip, HDR status, Image and Color cards.
PageBase {
    id: page

    title: i18n("GameVisual")
    stateKey: "game_visual"

    readonly property var modes: store ? store.options("gamevisual") : []
    readonly property bool hdr: store !== null && store.context.hdr === true
    readonly property var cur: store ? store.value("gamevisual") : undefined
    readonly property bool hasHdr: store !== null && store.isSupported("hdr_mode")
    readonly property bool hasAi: host !== null

    // ---- The mode strip, as in the vendor app: 108 x 104 tiles on a 123 px pitch. When they do
    // not all fit in the 968 px window, arrows at both ends step through them a page at a time
    // (the mouse wheel steps one tile). Tiles are only ever shown whole, never cut.
    readonly property int tilePitch: 123
    readonly property real arrowSpace: 40   // an arrow (34) and its gap
    readonly property var stripTiles: {
        const t = [];
        if (hasAi) {
            t.push({ kind: "ai" });
        }
        for (const m of modes) {
            t.push({ kind: "mode", opt: m });
        }
        if (hasHdr && hdrOptions(1).length > 0) {
            t.push({ kind: "hdr", family: 1 });
        }
        if (hasHdr && hdrOptions(2).length > 0) {
            t.push({ kind: "hdr", family: 2 });
        }
        return t;
    }
    readonly property bool overflowing: stripTiles.length * tilePitch - 15 > Theme.contentWidth
    readonly property real windowWidth: overflowing ? Theme.contentWidth - 2 * arrowSpace : Theme.contentWidth
    readonly property int perPage: Math.max(1, Math.floor((windowWidth + 15) / tilePitch))
    readonly property int maxFirst: Math.max(0, stripTiles.length - perPage)
    // Where the first whole tile starts inside the strip (centred between the arrows).
    readonly property real tileBase: overflowing ? arrowSpace + (windowWidth - (perPage * tilePitch - 15)) / 2 : 0
    property int first: 0        // index of the first tile shown
    property real wheelAcc: 0

    function scrollBy(n) {
        first = Math.max(0, Math.min(maxFirst, first + n));
    }
    function scrollEnd() {
        first = maxFirst;
    }
    function ensureVisible(i) {
        if (i < 0) {
            return;
        }
        if (i < first) {
            first = i;
        } else if (i >= first + perPage) {
            first = Math.min(maxFirst, i - perPage + 1);
        }
    }
    onMaxFirstChanged: first = Math.min(first, maxFirst)

    // The tile that is "on": AI Visual, the HDR tile, or the current mode. It is kept in view.
    readonly property int selectedTile: {
        if (aiOn || aiOpen) {
            return hasAi ? 0 : -1;
        }
        if (hdr) {
            return stripTiles.findIndex(t => t.kind === "hdr" && t.family === hdrFamily);
        }
        return stripTiles.findIndex(t => t.kind === "mode" && isCurrent(t.opt));
    }
    // The HDR tiles are the last two: with one selected, keep both in view.
    function ensureSelected() {
        ensureVisible(selectedTile);
        if (hdr && selectedTile >= 0 && stripTiles[selectedTile].kind === "hdr") {
            ensureVisible(stripTiles.length - 1);
            ensureVisible(selectedTile);
        }
    }
    onSelectedTileChanged: ensureSelected()
    onStripTilesChanged: ensureSelected()

    // ---- AI Visual: its tile is checked while it is on; clicking it opens its panel.
    property bool aiOpen: false
    readonly property bool aiOn: host !== null && host.config.ai_visual !== undefined && host.config.ai_visual.enabled === true

    // Picking an ordinary mode ends AI Visual (one config write), then applies the mode.
    function pickMode(value) {
        aiOpen = false;
        if (aiOn) {
            host.updateConfig(c => {
                c.ai_visual = c.ai_visual || {};
                c.ai_visual.enabled = false;
            });
        }
        store.commit("gamevisual", value);
    }

    Component.onCompleted: {
        ensureSelected();
        if (host !== null) {
            host.loadConfig();
            if (host.backend.env_var("SUMI_TAB") === "ai") {
                aiOpen = true;
            }
        }
    }

    // An HDR sub-mode pill: the translated label without the family name the row already has.
    function pillText(label) {
        const text = store.tr(label);
        const family = i18n("Dolby Vision");
        // Translations join them as "Dolby Vision X", "Dolby Vision, X", "Dolby Vision: X"…
        const rest = text.indexOf(family) === 0 ? text.substring(family.length).replace(/^[\s,:;\-–—、，：]+/, "") : "";
        return rest.length > 0 ? rest : text;
    }

    function modeIcon(label) {
        switch (label) {
        case "RTS/RPG": return "rts";
        case "FPS": return "fps";
        case "Cinema": return "cinema";
        case "Scenery": return "scenery";
        case "Racing": return "racing";
        case "sRGB": return "srgb";
        case "MOBA": return "moba";
        case "User": return "user";
        case "Night Vision": return "nightvision";
        default: return "unknown";
        }
    }

    function isCurrent(opt) {
        if (page.hdr || page.cur === undefined) {
            return false;
        }
        return opt.value === page.cur || (opt.label === "sRGB" && (page.cur === 3 || page.cur === 10));
    }

    readonly property string currentLabel: {
        for (const o of page.modes) {
            if (page.isCurrent(o)) {
                return o.label;
            }
        }
        return "";
    }

    // The HDR sub-modes come from the `hdr_mode` options; the high byte of a value is its
    // family: 1 = HDR10 (tile "HDR"), 2 = Dolby Vision (tile "HDR Dolby Vision", only when the
    // monitor has such options). The tiles are usable while HDR is on; the tile of the current
    // value's family is checked, clicking the other one shows its sub-modes, and picking one
    // of those writes the whole value (the family switch, which asks first).
    function hdrOptions(family) {
        return store ? store.options("hdr_mode").filter(o => (o.value >> 8) === family) : [];
    }
    readonly property int hdrFamily: {
        const v = store ? store.value("hdr_mode") : undefined;
        if (v !== undefined && (v >> 8) > 0) {
            return v >> 8;
        }
        return store && store.info.hdr_format === "dolby_vision" ? 2 : 1;
    }
    property int pickedFamily: 0       // the family whose pills are shown (0 = the current one)
    onHdrFamilyChanged: pickedFamily = 0
    readonly property int shownFamily: pickedFamily > 0 && hdrOptions(pickedFamily).length > 0 ? pickedFamily : hdrFamily
    readonly property var hdrRows: {
        const o = hdrOptions(shownFamily);
        return o.length > 0 ? [{ name: shownFamily === 2 ? "Dolby Vision" : "HDR10", family: shownFamily, options: o }] : [];
    }
    readonly property int cardsY: 256

    // ---- Mode Sync, Import / Export
    readonly property bool canModeSync: store !== null && store.monitors.filter(m => m.model === store.info.model).length >= 2
    readonly property bool modeSync: host !== null && host.config.mode_sync === true

    function currentModeLabel() {
        // Raw labels: this becomes part of a file name that other apps may read.
        return page.hdr ? "HDR" : (page.currentLabel !== "" ? page.currentLabel : "Settings");
    }

    function exportSettings() {
        const name = store.info.model + "_" + currentModeLabel().replace(/[^A-Za-z0-9]+/g, "") + ".json";
        page.chooser.save(name, url => {
            host.call("ExportSettings", [store.monitorId], true, (ok, text) => {
                if (ok) {
                    host.backend.write_text_file(url, text);
                }
            });
        });
    }

    function importSettings() {
        page.chooser.open(url => {
            const text = host.backend.read_text_file(url);
            if (text === "") {
                return;
            }
            const id = store.monitorId;
            const finished = log => {
                store.notice(i18n("Settings imported"), log !== "" ? log : i18n("Nothing needed to change."), true);
                store.request();
            };
            host.call("ImportSettings", [id, text, false], false, (ok, res) => {
                if (ok) {
                    finished(res);
                    return;
                }
                const doc = host.json(text, {});
                const fileMode = doc.DisplayMode !== undefined ? doc.DisplayMode : undefined;
                if (/switch first/.test(res) && fileMode !== undefined) {
                    const label = store.optionText("gamevisual", fileMode) || i18n("mode %1", fileMode);
                    store.ask(i18n("Switch mode"), i18n("This file is for %1. Switch to it and apply?", label), i18n("Switch and apply"), () => {
                        host.call("ImportSettings", [id, text, true], true, (ok2, res2) => {
                            if (ok2) {
                                finished(res2);
                            }
                        });
                    });
                } else {
                    store.notice(i18n("Import failed"), res, false);
                }
            });
        });
    }

    // The file dialog (tests hand in a stand-in).
    property var chooser: defaultChooser
    FileChooser {
        id: defaultChooser
    }

    // Mode Sync (header, top right): mirror this page's changes to monitors of the same model
    RogCheckBox {
        objectName: "mode_sync"
        visible: page.canModeSync
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 28
        text: i18n("Mode Sync")
        checked: page.modeSync
        onToggled: on => page.host.updateConfig(c => {
            c.mode_sync = on;
        })
    }

    Text {
        x: Theme.pageMargin
        y: 62
        text: i18n("Select Mode")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    // The HDR format the monitor advertises, above the HDR tile (the last tile).
    Text {
        objectName: "hdr_advertised"
        visible: page.hasHdr && page.store.hdrFormatName() !== ""
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 64
        text: i18n("Advertised: %1", page.store.hdrFormatName())
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontSmall
    }

    // Wheel over the strip steps through the tiles.
    Item {
        objectName: "strip_wheel"
        x: Theme.pageMargin
        y: 90
        width: Theme.contentWidth
        height: 104
        WheelHandler {
            onWheel: event => {
                page.wheelAcc += event.angleDelta.y !== 0 ? event.angleDelta.y : event.angleDelta.x;
                if (Math.abs(page.wheelAcc) >= 120) {
                    page.scrollBy(page.wheelAcc < 0 ? 1 : -1);
                    page.wheelAcc = 0;
                }
            }
        }
    }
    Repeater {
        model: page.stripTiles
        delegate: ModeTile {
            id: tile
            required property var modelData
            required property int index
            readonly property bool isAi: modelData.kind === "ai"
            readonly property bool isHdr: modelData.kind === "hdr"
            readonly property bool isDv: isHdr && modelData.family === 2
            objectName: isAi ? "mode_ai" : (isDv ? "mode_hdr_dv" : (isHdr ? "mode_hdr" : "mode_" + modelData.opt.label))
            x: Theme.pageMargin + page.tileBase + (index - page.first) * page.tilePitch
            y: 90
            visible: index >= page.first && index < page.first + page.perPage
            width: 108
            icon: isAi ? "aivisual" : (isDv ? "hdrdv" : (isHdr ? "hdr" : page.modeIcon(modelData.opt.label)))
            caption: isAi ? i18n("AI Visual") : (isDv ? i18n("HDR Dolby Vision") : (isHdr ? i18n("HDR") : page.store.tr(modelData.opt.label)))
            checked: isAi ? (page.aiOn || page.aiOpen) : (isHdr ? (page.hdr && page.hdrFamily === modelData.family) : (page.isCurrent(modelData.opt) && !page.aiOn && !page.aiOpen))
            // The HDR tiles are status tiles: usable (to pick a family's sub-modes) only while HDR is on.
            locked: isHdr && !page.hdr
            enabled: isAi || (isHdr ? page.hdr : page.store.isEnabled("gamevisual"))
            onClicked: {
                if (isAi) {
                    page.aiOpen = true;
                } else if (isHdr) {
                    page.aiOpen = false;
                    page.pickedFamily = modelData.family;
                } else {
                    page.pickMode(modelData.opt.value);
                }
            }
        }
    }
    StripArrow {
        objectName: "strip_left"
        visible: page.overflowing
        x: Theme.pageMargin
        y: 90 + 24
        direction: -1
        enabled: page.first > 0
        onClicked: page.scrollBy(-page.perPage)
    }
    StripArrow {
        objectName: "strip_right"
        visible: page.overflowing
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 90 + 24
        direction: 1
        enabled: page.first < page.maxFirst
        onClicked: page.scrollBy(page.perPage)
    }

    // Caption (SDR) or the HDR sub-mode pills
    Text {
        visible: !page.hdr && !page.aiOpen
        x: Theme.pageMargin
        y: 212
        height: 32
        verticalAlignment: Text.AlignVCenter
        text: page.currentLabel !== "" ? i18n("%1 mode settings", page.store.tr(page.currentLabel)) : i18n("Mode settings")
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontLabel
        font.weight: Font.Medium
    }
    Column {
        visible: page.hdr && !page.aiOpen
        x: Theme.pageMargin
        y: 206
        spacing: 10
        Repeater {
            model: page.hdrRows
            delegate: Row {
                id: hdrRow
                required property var modelData
                spacing: 20
                Text {
                    width: 110
                    anchors.verticalCenter: parent.verticalCenter
                    text: hdrRow.modelData.name
                    color: Theme.textDim
                    font.family: Theme.font
                    font.pixelSize: Theme.fontBody
                }
                Repeater {
                    model: hdrRow.modelData.options
                    delegate: RogPill {
                        required property var modelData
                        objectName: "hdr_" + modelData.value
                        text: page.pillText(modelData.label)
                        checked: page.store.value("hdr_mode") === modelData.value
                        enabled: page.store.isEnabled("hdr_mode")
                        onClicked: page.store.commit("hdr_mode", modelData.value)
                    }
                }
            }
        }
    }
    RogButton {
        id: resetButton
        objectName: "reset"
        visible: !page.aiOpen
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 212
        text: i18n("Reset")
        enabled: page.store !== null && page.store.isEnabled("reset")
        onClicked: page.store.commit("reset", 1)
    }
    // Compare: while held, the values from before your last change; on release, the current ones again.
    RogButton {
        objectName: "compare"
        visible: !page.aiOpen
        x: resetButton.x - 14 - width
        y: 212
        text: i18n("Compare")
        secondary: true
        enabled: page.store !== null && page.store.snapshot !== null && !page.hdr && !page.store.readOnly
        onDownChanged: {
            if (down) {
                page.store.compareStart();
            } else {
                page.store.compareEnd();
            }
        }
    }

    // ---- Footer
    Row {
        visible: !page.aiOpen
        x: Theme.pageMargin + Theme.contentWidth - width
        y: page.cardsY + 396 + 12
        spacing: 14
        RogButton {
            objectName: "import"
            text: i18n("Import")
            enabled: page.store !== null && !page.store.readOnly
            onClicked: page.importSettings()
        }
        RogButton {
            objectName: "export"
            text: i18n("Export")
            onClicked: page.exportSettings()
        }
    }

    // ---- Image
    Card {
        visible: !page.aiOpen
        x: Theme.pageMargin
        y: page.cardsY
        width: 478
        height: 396
        title: i18n("Image")

        Column {
            x: 20
            width: parent.width - 40
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "brightness"; title: i18n("Brightness") }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "contrast"; title: i18n("Contrast") }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "overdrive"; title: i18n("Variable OD") }
            SliderRow {
                width: parent.width
                compact: true
                divider: false
                store: page.store
                fid: "blue_light"
                title: i18n("Blue Light Filter")
                format: v => v === 0 ? i18n("Off") : String(Math.round(v))
            }
            ComboRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "shadow_boost"; title: i18n("Shadow Boost") }
            ToggleRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "ascr"; title: i18n("ASCR") }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "sharpness"; title: i18n("Sharpness") }
        }
    }

    // ---- Color
    Card {
        visible: !page.aiOpen
        x: Theme.pageMargin + 478 + 12
        y: page.cardsY
        width: 478
        height: 396
        title: i18n("Color")

        Column {
            x: 20
            width: parent.width - 40
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "saturation"; title: i18n("Saturation") }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "hue"; title: i18n("Hue") }
            ComboRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "color_temp"; title: i18n("Color Temp.") }
            ComboRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "gamma"; title: i18n("Gamma") }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "red_gain"; title: "R"; tint: ["#3A0A10", "#FF2A3A"] }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "green_gain"; title: "G"; tint: ["#0A2A12", "#2AE066"] }
            SliderRow { width: parent.width; compact: true; divider: false; store: page.store; fid: "blue_gain"; title: "B"; tint: ["#0A1638", "#2B7CFF"] }
        }
    }

    // The AI Visual panel while it is open (null otherwise).
    readonly property var aiPanel: aiLoader.item

    // ---- AI Visual panel (in place of the cards)
    Loader {
        id: aiLoader
        active: page.aiOpen && page.host !== null
        x: Theme.pageMargin
        y: 206
        width: Theme.contentWidth
        height: 510
        sourceComponent: AiVisualPanel {
            width: aiLoader.width
            height: aiLoader.height
            store: page.store
            host: page.host
            onCloseRequested: page.aiOpen = false
        }
    }
}
