pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2

// System Settings: power mode, OSD language and shortcuts, plus a few OSD
// settings Sumi adds (input, volume, mute, auto input detection, Adaptive-Sync).
PageBase {
    id: page

    title: i18n("System Settings")
    stateKey: "system"
    flick: scroller

    readonly property var hostState: host && store ? host.hostState(store.monitorId) : null
    readonly property bool hdrKnown: hostState !== null && hostState.hdr !== null && hostState.hdr !== undefined
    readonly property var desktop: host ? host.desktop : ({})
    readonly property bool panelKnown: desktop.panel_autohide !== null && desktop.panel_autohide !== undefined
    readonly property bool darkKnown: desktop.dark_theme !== null && desktop.dark_theme !== undefined

    Component.onCompleted: {
        host.loadHostState(store.monitorId);
        host.loadDesktop();
    }

    Connections {
        target: page.store
        function onMonitorIdChanged() {
            page.host.loadHostState(page.store.monitorId);
        }
    }

    function powerBlurb(v) {
        return v === 1 ? i18n("Lowers power use; most picture settings are locked.") : i18n("Full picture controls and performance.");
    }

    Flickable {
        id: scroller
        x: Theme.pageMargin
        y: 70
        width: Theme.contentWidth + 18
        height: parent.height - 70 - 8
        clip: true
        contentWidth: Theme.contentWidth
        contentHeight: rows.height
        boundsBehavior: Flickable.StopAtBounds
        QQC2.ScrollBar.vertical: RogScrollBar {}

        Column {
            id: rows
            width: Theme.contentWidth

            // Display HDR: KDE's HDR switch for this monitor's output
            SettingRow {
                width: parent.width
                title: i18n("Display HDR")
                description: i18n("Turns HDR on in KDE for this monitor.")
                dim: !page.hdrKnown
                RogToggle {
                    objectName: "toggle_display_hdr"
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.hdrKnown
                    checked: page.hdrKnown && page.hostState.hdr === true
                    onToggled: on => page.host.setHdr(page.store.monitorId, on)
                }
            }

            // Power Settings: two radios with a line of explanation each
            Item {
                width: parent.width
                height: Math.max(134, powerColumn.y + powerColumn.height + 2)
                Text {
                    y: 8
                    text: i18n("Power Settings")
                    color: page.store.isEnabled("power_saving") ? Theme.text : Theme.disabledText
                    font.family: Theme.font
                    font.pixelSize: Theme.fontLabel
                    font.weight: Font.Medium
                }
                Column {
                    id: powerColumn
                    y: 38
                    spacing: 6
                    Repeater {
                        model: page.store.options("power_saving")
                        delegate: Item {
                            id: pw
                            required property var modelData
                            width: 600
                            height: Math.max(44, 24 + blurb.height)
                            RogRadio {
                                objectName: "power_" + pw.modelData.value
                                text: page.store.tr(pw.modelData.label)
                                checked: page.store.value("power_saving") === pw.modelData.value
                                enabled: page.store.isEnabled("power_saving")
                                onClicked: page.store.commit("power_saving", pw.modelData.value)
                            }
                            Text {
                                id: blurb
                                x: 28
                                y: 24
                                width: pw.width - x
                                wrapMode: Text.WordWrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                text: page.powerBlurb(pw.modelData.value)
                                color: Theme.textDim
                                font.family: Theme.font
                                font.pixelSize: Theme.fontBody
                            }
                        }
                    }
                }
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: Theme.separator
                    opacity: 0.4
                }
            }

            ComboRow { width: parent.width; store: page.store; fid: "language"; title: i18n("OSD Language"); description: i18n("Language of the monitor's on-screen menu.") }
            ComboRow { width: parent.width; store: page.store; fid: "shortcut_1"; title: i18n("OSD Shortcut 1"); description: i18n("What the first shortcut key on the monitor opens.") }
            ComboRow { width: parent.width; store: page.store; fid: "shortcut_2"; title: i18n("OSD Shortcut 2"); description: i18n("What the second shortcut key on the monitor opens.") }
            ComboRow { width: parent.width; store: page.store; fid: "input"; title: i18n("Input Select"); description: i18n("The video input the monitor shows.") }
            SliderRow { width: parent.width; store: page.store; fid: "volume"; title: i18n("Volume"); description: i18n("Speaker and headphone output level.") }
            ToggleRow { width: parent.width; store: page.store; fid: "mute"; title: i18n("Mute"); description: i18n("Silences the monitor's audio output."); onValue: 1; offValue: 2 }
            ToggleRow { width: parent.width; store: page.store; fid: "auto_input"; title: i18n("Auto Input Detection"); description: i18n("Switches to a source when it starts sending a signal.") }
            ToggleRow { width: parent.width; store: page.store; fid: "vrr"; title: i18n("Adaptive-Sync (VRR)"); description: i18n("Matches the refresh rate to the frame rate of the game.") }

            // Desktop: two KDE settings that spare an OLED panel
            Item {
                width: parent.width
                height: 44
                Text {
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 6
                    text: i18n("Desktop")
                    color: Theme.text
                    font.family: Theme.font
                    font.pixelSize: Theme.fontLabel
                    font.weight: Font.Medium
                }
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: Theme.separator
                    opacity: 0.4
                }
            }
            SettingRow {
                width: parent.width
                title: i18n("Auto-hide panel")
                description: i18n("Hides the Plasma panel when you aren't using it, so it doesn't burn into an OLED panel.")
                dim: !page.panelKnown
                RogToggle {
                    objectName: "toggle_panel_autohide"
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.panelKnown
                    checked: page.panelKnown && page.desktop.panel_autohide === true
                    onToggled: on => page.host.setPanelAutohide(on)
                }
            }
            SettingRow {
                width: parent.width
                title: i18n("Dark theme")
                description: i18n("Uses a dark colour scheme: fewer bright pixels, less wear on an OLED panel.")
                divider: false
                dim: !page.darkKnown
                RogToggle {
                    objectName: "toggle_dark_theme"
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.darkKnown
                    checked: page.darkKnown && page.desktop.dark_theme === true
                    onToggled: on => page.host.setDarkTheme(on)
                }
            }

            // Room under the last row for the status toast
            Item {
                width: parent.width
                height: 44
            }
        }
    }
}
