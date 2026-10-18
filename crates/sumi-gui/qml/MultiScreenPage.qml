pragma ComponentBehavior: Bound

import QtQuick

// MultiScreen: MultiFrame (planned), PIP / PBP layouts and USB KVM.
PageBase {
    id: page

    title: i18n("MultiScreen")
    stateKey: "multi_screen"

    property string fn: "pip"   // "multiframe" | "pip" | "kvm"
    property var tiles: []          // [{output, aspect, layout, tiles}] from KWin
    property var tileNames: []      // [[n, name]]
    property string tilesError: ""
    property bool tilesLoaded: false
    property string selectedOutput: ""
    property bool outputPickedByUser: false
    // The monitor's own info can arrive after the screen list: prefer its
    // connector until the user picks a screen themselves.
    readonly property string myConnector: store.info.connector || ""
    onMyConnectorChanged: {
        if (!outputPickedByUser && tiles.some(t => t.output === myConnector))
            selectedOutput = myConnector;
    }
    property string hoverName: ""
    property int pickedMode: -1
    property var picks: ({})    // combo selections made by the user: key -> index

    readonly property var layouts: store ? store.options("pip_mode") : []
    readonly property int shownMode: pickedMode >= 0 ? pickedMode : (store && store.value("pip_mode") !== undefined ? store.value("pip_mode") : 0)

    // The user's pick, else what the monitor reports; when it reports nothing (0), the first
    // option, as the vendor app does.
    function indexFor(key, fid) {
        if (picks[key] !== undefined) {
            return picks[key];
        }
        const i = store.indexOfValue(fid);
        return i >= 0 ? i : (store.options(fid).length > 0 ? 0 : -1);
    }
    function pick(key, i) {
        const p = Object.assign({}, picks);
        p[key] = i;
        picks = p;
    }

    function layoutName(n) {
        if (n === null || n === undefined) return i18n("Custom");
        if (n === 0) return i18n("None");
        const e = tileNames.find(x => x[0] === n);
        return e ? e[1] : i18n("Layout %1", n);
    }

    readonly property var currentTile: tiles.find(t => t.output === selectedOutput) || null

    function loadTiles() {
        host.call("TileLayoutNames", [], false, (ok, text) => {
            tileNames = ok ? host.json(text, []) : [];
        });
        host.call("GetTileLayouts", [], false, (ok, text) => {
            tilesLoaded = true;
            if (ok) {
                tiles = host.json(text, []);
                tilesError = "";
                if (!tiles.some(t => t.output === selectedOutput)) {
                    const mine = store.info.connector;
                    selectedOutput = tiles.some(t => t.output === mine) ? mine : (tiles.length > 0 ? tiles[0].output : "");
                }
            } else {
                tiles = [];
                tilesError = text;
            }
        });
    }

    function setLayout(n) {
        if (selectedOutput === "") {
            return;
        }
        host.call("SetTileLayout", [selectedOutput, n], true, (ok, text) => {
            if (ok) {
                tiles = host.json(text, tiles);
            }
        });
    }

    onFnChanged: if (fn === "multiframe" && !tilesLoaded) loadTiles()

    Component.onCompleted: {
        const wanted = host ? host.backend.env_var("SUMI_TAB") : "";
        if (wanted === "multiframe" || wanted === "kvm" || wanted === "pip") {
            fn = wanted;
        }
    }

    // Write sources and GameVisual modes for both windows, then the layout.
    function applyPip() {
        if (!store || store.readOnly) {
            return;
        }
        const steps = [];
        const mode = page.shownMode;
        if (mode !== 0) {
            const wanted = [["srcA", "pip_source_a"], ["srcB", "pip_source_b"], ["gvA", "pip_gamevisual_a"], ["gvB", "pip_gamevisual_b"]];
            for (const w of wanted) {
                const i = indexFor(w[0], w[1]);
                const opts = store.options(w[1]);
                if (i >= 0 && i < opts.length) {
                    steps.push({ id: w[1], value: opts[i].value });
                }
            }
        }
        steps.push({ id: "pip_mode", value: mode });
        const go = () => store.runSequence(steps);
        const r = store.riskInfo("pip_mode", mode);
        if (r) {
            store.ask(r.title, r.text, r.ok, go);
        } else {
            go();
        }
    }

    Text {
        x: Theme.pageMargin
        y: 62
        text: i18n("Select Function")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Row {
        x: Theme.pageMargin
        y: 90
        spacing: 15
        ModeTile {
            objectName: "fn_multiframe"
            icon: "multiframe"
            caption: i18n("MultiFrame")
            checked: page.fn === "multiframe"
            onClicked: page.fn = "multiframe"
        }
        ModeTile {
            objectName: "fn_pip"
            icon: "pippbp"
            caption: i18n("PIP/PBP")
            checked: page.fn === "pip"
            enabled: page.store.isSupported("pip_mode")
            onClicked: page.fn = "pip"
        }
        ModeTile {
            objectName: "fn_kvm"
            icon: "kvm"
            caption: i18n("KVM")
            checked: page.fn === "kvm"
            enabled: page.store.isSupported("kvm")
            onClicked: page.fn = "kvm"
        }
    }

    // ---- MultiFrame: KWin tile layouts per screen
    Card {
        visible: page.fn === "multiframe"
        x: Theme.pageMargin
        y: 226
        width: Theme.contentWidth
        height: 396
        title: i18n("MultiFrame")

        // Screens
        Text {
            x: 20
            y: 14
            text: i18n("Screen")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        Column {
            x: 20
            y: 42
            width: 290
            spacing: 6
            Repeater {
                model: page.tiles
                delegate: Rectangle {
                    id: screenRow
                    required property var modelData
                    objectName: "screen_" + modelData.output
                    readonly property bool current: page.selectedOutput === modelData.output
                    width: parent.width
                    height: 46
                    color: current ? Theme.alpha(Theme.pressed, 0.3) : (screenHover.hovered ? Theme.alpha(Theme.hoverFill, 0.25) : "transparent")
                    border.width: 1
                    border.color: current ? Theme.pressed : Theme.borderTile
                    Rectangle {
                        visible: screenRow.current
                        width: 3
                        height: parent.height
                        color: Theme.red
                    }
                    Text {
                        x: 16
                        y: 6
                        text: screenRow.modelData.output
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: Theme.fontBody
                        font.weight: Font.Medium
                    }
                    Text {
                        x: 16
                        y: 25
                        width: parent.width - 28
                        elide: Text.ElideRight
                        text: page.layoutName(screenRow.modelData.layout)
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontSmall
                    }
                    HoverHandler {
                        id: screenHover
                        cursorShape: Qt.PointingHandCursor
                    }
                    TapHandler {
                        onTapped: {
                            page.outputPickedByUser = true;
                            page.selectedOutput = screenRow.modelData.output;
                        }
                    }
                }
            }
        }

        // Layouts: None and 1..13
        Grid {
            id: layoutGrid
            x: 334
            y: 14
            columns: 7
            spacing: 10
            enabled: page.currentTile !== null
            Repeater {
                model: 14
                delegate: Item {
                    id: zoneTile
                    required property int index
                    readonly property int n: index === 0 ? 0 : index
                    readonly property bool current: page.currentTile !== null && page.currentTile.layout === n
                    objectName: "zone_" + n
                    width: 78
                    height: 82
                    Rectangle {
                        anchors.fill: parent
                        color: zoneTile.current ? Theme.alpha(Theme.pressed, 0.25) : (zoneHover.hovered ? Theme.alpha(Theme.hoverFill, 0.25) : Theme.alpha(Theme.black, 0.12))
                        border.width: 1
                        border.color: zoneTile.current ? Theme.pressed : (zoneHover.hovered ? Theme.hoverFill : Theme.borderTile)
                    }
                    ZoneThumb {
                        x: 11
                        y: 12
                        layout: zoneTile.n
                        checked: zoneTile.current
                        hovered: zoneHover.hovered
                    }
                    Text {
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 8
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: zoneTile.n === 0 ? i18n("None") : String(zoneTile.n)
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: Theme.fontSmall
                    }
                    HoverHandler {
                        id: zoneHover
                        cursorShape: Qt.PointingHandCursor
                        onHoveredChanged: page.hoverName = hovered ? page.layoutName(zoneTile.n) : ""
                    }
                    TapHandler {
                        onTapped: page.setLayout(zoneTile.n)
                    }
                }
            }
        }
        Text {
            x: 334
            y: 14 + 2 * 82 + 10 + 14
            width: 606
            height: 20
            text: page.hoverName !== "" ? page.hoverName : (page.currentTile ? page.layoutName(page.currentTile.layout) : "")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
        Text {
            x: 334
            y: 14 + 2 * 82 + 10 + 48
            width: 606
            wrapMode: Text.WordWrap
            text: i18n("Hold Shift while dragging a window to drop it into a zone (KDE tiling).")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }

        // KWin not available
        Rectangle {
            visible: page.tilesError !== ""
            x: 1
            y: 0
            width: parent.width - 2
            height: parent.height
            color: "#E6050505"
            Column {
                anchors.centerIn: parent
                spacing: 8
                width: parent.width - 120
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: i18n("MultiFrame needs KDE Plasma 6.")
                    color: Theme.text
                    font.family: Theme.font
                    font.pixelSize: Theme.fontLabel
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    text: page.tilesError
                    color: Theme.textDim
                    font.family: Theme.font
                    font.pixelSize: Theme.fontSmall
                }
            }
        }
    }

    // ---- PIP / PBP
    Card {
        visible: page.fn === "pip"
        x: Theme.pageMargin
        y: 226
        width: Theme.contentWidth
        height: 458
        title: i18n("PIP/PBP")

        Grid {
            x: 20
            y: 16
            columns: 3
            spacing: 12
            Repeater {
                model: page.layouts
                delegate: OptionTile {
                    id: lay
                    required property var modelData
                    objectName: "layout_" + modelData.value
                    width: 298
                    height: 84
                    glyphWidth: 92
                    label: page.store.tr(modelData.label)
                    checked: page.shownMode === modelData.value
                    enabled: page.store.isEnabled("pip_mode")
                    onClicked: page.pickedMode = lay.modelData.value
                    LayoutThumb {
                        anchors.centerIn: parent
                        mode: lay.modelData.value
                    }
                }
            }
        }

        // Window rows
        Item {
            x: 20
            y: 224
            width: parent.width - 40
            height: 150
            Rectangle {
                width: parent.width
                height: 1
                color: Theme.separator
                opacity: 0.4
            }

            Repeater {
                model: [
                    { name: i18n("Window A"), src: "pip_source_a", gv: "pip_gamevisual_a", ks: "srcA", kg: "gvA" },
                    { name: i18n("Window B"), src: "pip_source_b", gv: "pip_gamevisual_b", ks: "srcB", kg: "gvB" }
                ]
                delegate: Item {
                    id: winRow
                    required property var modelData
                    required property int index
                    y: 14 + index * 46
                    width: parent.width
                    height: 36
                    Text {
                        id: winName
                        anchors.verticalCenter: parent.verticalCenter
                        text: winRow.modelData.name
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: Theme.fontLabel
                        font.weight: Font.Medium
                    }
                    // The labels keep their usual places; a longer translation pushes what follows.
                    Text {
                        id: srcLabel
                        x: Math.max(120, winName.width + 16)
                        anchors.verticalCenter: parent.verticalCenter
                        text: i18n("Source")
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontBody
                    }
                    RogCombo {
                        id: srcCombo
                        objectName: "combo_" + winRow.modelData.src
                        x: Math.max(184, srcLabel.x + srcLabel.width + 10)
                        width: 220
                        anchors.verticalCenter: parent.verticalCenter
                        enabled: page.store.isEnabled(winRow.modelData.src)
                        model: page.store.displayOptions(winRow.modelData.src)
                        currentIndex: page.indexFor(winRow.modelData.ks, winRow.modelData.src)
                        onActivated: i => page.pick(winRow.modelData.ks, i)
                    }
                    Text {
                        id: gvLabel
                        x: Math.max(440, srcCombo.x + srcCombo.width + 16)
                        anchors.verticalCenter: parent.verticalCenter
                        text: i18n("GameVisual")
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontBody
                    }
                    RogCombo {
                        objectName: "combo_" + winRow.modelData.gv
                        x: Math.max(540, gvLabel.x + gvLabel.width + 10)
                        width: 220
                        anchors.verticalCenter: parent.verticalCenter
                        enabled: page.store.isEnabled(winRow.modelData.gv)
                        model: page.store.displayOptions(winRow.modelData.gv)
                        currentIndex: page.indexFor(winRow.modelData.kg, winRow.modelData.gv)
                        onActivated: i => page.pick(winRow.modelData.kg, i)
                    }
                }
            }

            RogButton {
                objectName: "pip_apply"
                anchors.right: parent.right
                y: 108
                text: i18n("Apply")
                enabled: page.store.isEnabled("pip_mode")
                onClicked: page.applyPip()
            }
        }
    }

    // ---- KVM
    Card {
        visible: page.fn === "kvm"
        x: Theme.pageMargin
        y: 226
        width: Theme.contentWidth
        height: 300
        title: i18n("KVM")

        Text {
            x: 20
            y: 16
            text: i18n("Choose which computer the monitor's USB hub belongs to.")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        Column {
            x: 20
            y: 56
            spacing: 14
            Repeater {
                model: page.store ? page.store.options("kvm") : []
                delegate: RogRadio {
                    required property var modelData
                    objectName: "kvm_" + modelData.value
                    text: page.store.tr(modelData.label)
                    checked: page.store.value("kvm") === modelData.value
                    enabled: page.store.isEnabled("kvm")
                    onClicked: page.store.commit("kvm", modelData.value)
                }
            }
        }
    }
}
