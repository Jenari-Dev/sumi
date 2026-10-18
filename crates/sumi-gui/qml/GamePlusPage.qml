pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2

// GamePlus: on-screen overlays (crosshair, timer, FPS counter, alignment grid)
// and the arrow pad that moves them.
PageBase {
    id: page

    title: i18n("GamePlus")
    stateKey: "game_plus"

    readonly property var functions: [
        { fid: "crosshair", icon: "crosshair", caption: i18n("Crosshair") },
        { fid: "timer", icon: "timer", caption: i18n("Timer") },
        { fid: "fps_counter", icon: "fpscounter", caption: i18n("FPS Counter") },
        { fid: "display_alignment", icon: "alignment", caption: i18n("Display Alignment") }
    ]
    readonly property var shown: store ? functions.filter(f => store.isSupported(f.fid)) : []

    // The function being edited: the one the user picked, else the one that is on.
    property string picked: ""
    readonly property string selected: {
        if (picked !== "" && shown.some(f => f.fid === picked)) {
            return picked;
        }
        for (const f of shown) {
            const v = store.value(f.fid);
            if (v !== undefined && v > 0) {
                return f.fid;
            }
        }
        return shown.length > 0 ? shown[0].fid : "";
    }
    readonly property var selectedDef: shown.find(f => f.fid === selected) || null
    readonly property var choices: store && selected !== "" ? store.options(selected) : []
    readonly property bool movable: selected !== "" && selected !== "display_alignment" && store.isEnabled("gameplus_position")

    function hue(label) {
        return label.indexOf("Blue") >= 0 ? "#2B8CFF" : (label.indexOf("Green") >= 0 ? "#25E06B" : Theme.text);
    }
    function crossKind(opt) {
        if (opt.value === 0) return "off";
        if (opt.label.indexOf("Dot") >= 0) return "dot";
        if (opt.label.indexOf("Mini") >= 0) return "mini";
        if (opt.label.indexOf("Heavy") >= 0) return "heavy";
        return "plain";
    }

    Text {
        x: Theme.pageMargin
        y: 62
        text: i18n("Select Mode")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Row {
        x: Theme.pageMargin
        y: 90
        spacing: 15
        Repeater {
            model: page.shown
            delegate: ModeTile {
                id: fnTile
                required property var modelData
                objectName: "fn_" + modelData.fid
                icon: modelData.icon
                caption: modelData.caption
                checked: page.selected === modelData.fid
                enabled: page.store.isEnabled(modelData.fid)
                onClicked: page.picked = modelData.fid

                // "On" marker
                Rectangle {
                    visible: (page.store.value(fnTile.modelData.fid) || 0) > 0
                    x: parent.width - 16
                    y: 8
                    width: 8
                    height: 8
                    rotation: 45
                    color: Theme.red
                }
            }
        }
    }

    Text {
        x: Theme.pageMargin
        y: 214
        text: i18n("Select Mode Style")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    Card {
        x: Theme.pageMargin
        y: 242
        width: Theme.contentWidth
        height: 470
        title: page.selectedDef ? page.selectedDef.caption : ""

        // Option tiles: two columns
        Flickable {
            id: flick
            x: 20
            y: 16
            width: 620
            height: parent.height - 32
            clip: true
            contentWidth: width
            contentHeight: grid.height
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: RogScrollBar {}

            Grid {
                id: grid
                columns: 2
                spacing: 10
                Repeater {
                    model: page.choices
                    delegate: OptionTile {
                        id: opt
                        required property var modelData
                        objectName: "opt_" + modelData.value
                        width: 298
                        height: 70
                        label: page.store.tr(modelData.label)
                        checked: page.store.value(page.selected) === modelData.value
                        enabled: page.store.isEnabled(page.selected)
                        onClicked: page.store.commit(page.selected, modelData.value)

                        // Glyphs
                        CrosshairGlyph {
                            visible: page.selected === "crosshair"
                            anchors.centerIn: parent
                            kind: page.crossKind(opt.modelData)
                            tint: page.hue(opt.modelData.label)
                        }
                        Icon {
                            visible: page.selected !== "crosshair" && opt.modelData.value === 0
                            anchors.centerIn: parent
                            size: 34
                            lineWidth: 1.6
                            name: "off"
                            color: Theme.textDim
                        }
                        Icon {
                            visible: page.selected === "timer" && opt.modelData.value !== 0
                            anchors.centerIn: parent
                            size: 38
                            lineWidth: 1.6
                            name: "timer"
                        }
                        Text {
                            visible: page.selected === "fps_counter" && opt.modelData.label === "Number"
                            anchors.centerIn: parent
                            text: "144"
                            color: Theme.text
                            font.family: Theme.font
                            font.pixelSize: 22
                            font.bold: true
                        }
                        Row {
                            visible: page.selected === "fps_counter" && opt.modelData.label === "Bar Graph"
                            anchors.centerIn: parent
                            spacing: 4
                            Repeater {
                                model: [10, 16, 12, 22, 18]
                                delegate: Item {
                                    required property int modelData
                                    width: 6
                                    height: 24
                                    Rectangle {
                                        anchors.bottom: parent.bottom
                                        width: parent.width
                                        height: parent.modelData
                                        color: Theme.text
                                    }
                                }
                            }
                        }
                        Icon {
                            visible: page.selected === "display_alignment" && opt.modelData.value !== 0
                            anchors.centerIn: parent
                            size: 40
                            lineWidth: 1.6
                            name: "alignment"
                        }
                    }
                }
            }
        }

        // Position pad
        Item {
            id: pad
            x: 664
            y: 0
            width: 284
            height: parent.height
            opacity: page.movable ? 1 : 0.45

            Text {
                x: 0
                y: 16
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: i18n("Set GamePlus Position")
                color: Theme.text
                font.family: Theme.font
                font.pixelSize: Theme.fontLabel
                font.weight: Font.Medium
            }
            Text {
                x: 0
                y: 42
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: page.selected === "display_alignment" ? i18n("Not available for the alignment grid.") : i18n("Moves the overlay that is on.")
                color: Theme.textDim
                font.family: Theme.font
                font.pixelSize: Theme.fontSmall
            }

            // 1 up, 2 down, 3 right, 4 left
            ArrowButton {
                objectName: "pos_up"
                x: parent.width / 2 - 24
                y: 100
                direction: 0
                enabled: page.movable
                onStep: page.store.write("gameplus_position", 1)
            }
            ArrowButton {
                objectName: "pos_left"
                x: parent.width / 2 - 24 - 58
                y: 158
                direction: 270
                enabled: page.movable
                onStep: page.store.write("gameplus_position", 4)
            }
            ArrowButton {
                objectName: "pos_right"
                x: parent.width / 2 - 24 + 58
                y: 158
                direction: 90
                enabled: page.movable
                onStep: page.store.write("gameplus_position", 3)
            }
            ArrowButton {
                objectName: "pos_down"
                x: parent.width / 2 - 24
                y: 216
                direction: 180
                enabled: page.movable
                onStep: page.store.write("gameplus_position", 2)
            }
            Rectangle {
                x: parent.width / 2 - 5
                y: 158 + 24 - 5
                width: 10
                height: 10
                rotation: 45
                color: "transparent"
                border.width: 1
                border.color: Theme.borderTile
            }
        }

        Rectangle {
            x: 650
            y: 16
            width: 1
            height: parent.height - 32
            color: Theme.separator
            opacity: 0.4
        }
    }
}
