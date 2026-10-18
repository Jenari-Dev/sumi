pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2

// HotKey: global shortcuts for the monitor. KDE owns key capture and conflicts,
// so the keys are set in System Settings; this page shows what is bound, lets
// you try an action, and holds the host-side options.
PageBase {
    id: page

    title: i18n("HotKey")

    property var bindings: []
    property bool bindingsLoaded: false
    property string bindingsError: ""
    property var gamepads: []
    property bool infoDismissed: true

    readonly property var cfg: host ? host.config : ({})
    readonly property bool enabledKeys: !!(cfg.hotkeys && cfg.hotkeys.enabled)
    readonly property int step: cfg.hotkeys && cfg.hotkeys.step ? cfg.hotkeys.step : 5
    readonly property var steps: [1, 2, 5, 10]

    function reload() {
        host.loadConfig();
        host.call("Hotkeys", [], false, (ok, text) => {
            bindingsLoaded = true;
            if (ok) {
                bindings = host.json(text, []);
                bindingsError = "";
            } else {
                bindings = [];
                bindingsError = text;
            }
        });
        host.call("Gamepads", [], false, (ok, text) => {
            gamepads = ok ? host.json(text, []) : [];
        });
    }

    function keysText(keys) {
        return keys && keys.length > 0 ? keys.join(", ") : i18n("Not set");
    }

    // Run an action now. The ones with side effects ask first, like their buttons elsewhere.
    function test(action) {
        const risk = store ? store.actionRisk(action) : null;
        const run = () => host.call("RunAction", [action], true);
        if (risk) {
            store.ask(risk.title, risk.text, risk.ok, run);
        } else {
            run();
        }
    }

    function dismissInfo() {
        infoDismissed = true;
        host.backend.set_pref("hotkey_info_dismissed", "1");
    }

    Component.onCompleted: {
        infoDismissed = host.backend.pref("hotkey_info_dismissed") === "1";
        reload();
    }

    // ---- Header toggle
    Row {
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 26
        spacing: 14
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: i18n("Enable hot keys")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
        RogToggle {
            objectName: "toggle_hotkeys"
            anchors.verticalCenter: parent.verticalCenter
            checked: page.enabledKeys
            onToggled: on => page.host.updateConfig(c => {
                c.hotkeys.enabled = on;
            })
        }
    }

    Column {
        id: head
        x: Theme.pageMargin
        y: 70
        width: Theme.contentWidth
        spacing: 12

        SettingRow {
            width: parent.width
            title: i18n("Step")
            description: i18n("How far brightness, contrast and saturation move for each key press.")
            dim: !page.enabledKeys
            RogCombo {
                objectName: "combo_step"
                anchors.verticalCenter: parent.verticalCenter
                width: 160
                model: page.steps.map(v => String(v))
                currentIndex: page.steps.indexOf(page.step)
                enabled: page.enabledKeys
                onActivated: i => page.host.updateConfig(c => {
                    c.hotkeys.step = page.steps[i];
                })
            }
        }

        // First visit: what EZ OSD is
        Rectangle {
            id: info
            objectName: "info_card"
            visible: !page.infoDismissed
            width: parent.width
            height: visible ? 96 : 0
            color: Theme.alpha(Theme.black, 0.3)
            border.width: 1
            border.color: Theme.hoverFill

            Brackets {
                anchors.fill: parent
                size: 8
                color: Theme.hoverEdge
            }
            Text {
                x: 20
                y: 14
                text: i18n("EZ OSD")
                color: Theme.text
                font.family: Theme.font
                font.pixelSize: Theme.fontLabel
                font.weight: Font.Medium
            }
            Text {
                x: 20
                y: 40
                width: parent.width - 200
                text: i18n("Drive the monitor's own menu from the keyboard: bind “EZ OSD: open the monitor menu” and the arrow, enter and back actions below, then use them like the monitor's joystick.")
                wrapMode: Text.WordWrap
                color: Theme.textSoft
                font.family: Theme.font
                font.pixelSize: Theme.fontBody
            }
            RogButton {
                objectName: "dismiss_info"
                x: parent.width - width - 20
                y: (parent.height - height) / 2
                text: i18n("Got it")
                secondary: true
                onClicked: page.dismissInfo()
            }
        }

        // Game controller
        SettingRow {
            objectName: "gamepad_row"
            visible: page.gamepads.length > 0
            width: parent.width
            height: visible ? 62 : 0
            title: i18n("Gamepad EZ OSD")
            description: i18n("Hold LB + RB + LT + RT to start, then D-pad to move, A to select, B to go back.")
            RogToggle {
                objectName: "toggle_gamepad"
                anchors.verticalCenter: parent.verticalCenter
                checked: !!(page.cfg.gamepad && page.cfg.gamepad.enabled)
                onToggled: on => page.host.updateConfig(c => {
                    c.gamepad = c.gamepad || {};
                    c.gamepad.enabled = on;
                })
            }
        }
    }

    // ---- The actions
    Card {
        id: actions
        x: Theme.pageMargin
        y: head.y + head.height + 12
        width: Theme.contentWidth
        height: 690 - y

        // Column titles
        Text {
            x: 20
            y: 0
            height: 36
            verticalAlignment: Text.AlignVCenter
            text: i18n("Action")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        Text {
            x: 440
            y: 0
            height: 36
            verticalAlignment: Text.AlignVCenter
            text: i18n("Keys")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        Rectangle {
            x: 1
            y: 36
            width: parent.width - 2
            height: 1
            color: Theme.separator
            opacity: 0.5
        }

        ListView {
            id: list
            x: 1
            y: 37
            width: parent.width - 2
            height: parent.height - 38
            clip: true
            model: page.bindings
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: RogScrollBar {}

            delegate: Item {
                id: row
                required property var modelData
                required property int index
                width: list.width - 8
                height: 34
                Rectangle {
                    anchors.fill: parent
                    color: Theme.alpha(Theme.text, 0.04)
                    visible: row.index % 2 === 1
                }
                Text {
                    x: 19
                    anchors.verticalCenter: parent.verticalCenter
                    text: i18n(row.modelData.label)
                    color: Theme.text
                    font.family: Theme.font
                    font.pixelSize: Theme.fontBody
                }
                Text {
                    x: 439
                    anchors.verticalCenter: parent.verticalCenter
                    width: 340
                    elide: Text.ElideRight
                    text: page.keysText(row.modelData.keys)
                    color: row.modelData.keys && row.modelData.keys.length > 0 ? Theme.text : Theme.disabledText
                    font.family: Theme.font
                    font.pixelSize: Theme.fontBody
                }
                RogButton {
                    objectName: "test_" + row.modelData.action
                    x: parent.width - width - 20
                    anchors.verticalCenter: parent.verticalCenter
                    minWidth: 76
                    height: 26
                    text: i18n("Test")
                    secondary: true
                    onClicked: page.test(row.modelData.action)
                }
            }
        }

        Column {
            visible: page.bindingsLoaded && page.bindings.length === 0
            anchors.centerIn: parent
            spacing: 8
            width: parent.width - 80
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: i18n("KDE global shortcuts aren't available.")
                color: Theme.text
                font.family: Theme.font
                font.pixelSize: Theme.fontLabel
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                text: page.bindingsError
                color: Theme.textDim
                font.family: Theme.font
                font.pixelSize: Theme.fontSmall
            }
        }
    }

    // ---- Footer
    RogButton {
        id: openKeys
        objectName: "open_keys"
        x: Theme.pageMargin
        y: 702
        minWidth: 250
        text: i18n("Set keys in System Settings")
        onClicked: page.host.backend.open_system_settings("kcm_keys")
    }
    Text {
        x: openKeys.x + openKeys.width + 20
        y: 702
        width: Theme.pageMargin + Theme.contentWidth - x
        height: 32
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
        fontSizeMode: Text.Fit
        minimumPixelSize: 11
        elide: Text.ElideRight
        text: i18n("Keys are set under Shortcuts › Sumi; nothing is bound by default.")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
}
