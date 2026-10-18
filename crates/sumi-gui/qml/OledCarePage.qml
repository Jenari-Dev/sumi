import QtQuick
import QtQuick.Controls as QQC2

// OLED Care: burn-in protection switches, pixel cleaning, reminder and screen move.
PageBase {
    id: page

    title: i18n("OLED Care")
    stateKey: "oled_care"
    flick: scroller

    readonly property var hostState: host && store ? host.hostState(store.monitorId) : null
    readonly property bool targetKnown: hostState !== null && hostState.target_mode !== null && hostState.target_mode !== undefined
    readonly property bool targetOn: targetKnown && hostState.target_mode === true

    Component.onCompleted: host.loadHostState(store.monitorId)

    Connections {
        target: page.store
        function onMonitorIdChanged() {
            page.host.loadHostState(page.store.monitorId);
        }
    }

    Flickable {
        id: scroller
        x: Theme.pageMargin
        y: 76
        width: Theme.contentWidth + 18
        height: parent.height - 76 - 12
        clip: true
        contentWidth: Theme.contentWidth
        contentHeight: rows.height
        boundsBehavior: Flickable.StopAtBounds
        QQC2.ScrollBar.vertical: RogScrollBar {}

        Column {
            id: rows
            width: Theme.contentWidth

            ToggleRow {
                width: parent.width
                store: page.store
                fid: "screen_dimming"
                title: i18n("Screen Dimming Control")
                description: i18n("Slowly dims a picture that stays still, so no pixel burns at full brightness.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "logo_detection"
                title: i18n("Logo Detection")
                description: i18n("Spots bright logos and HUD elements that don't move and lowers their brightness.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "taskbar_detection"
                title: i18n("Taskbar Detection")
                description: i18n("Recognises a fixed taskbar at the screen edge and dims it.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "boundary_detection"
                title: i18n("Boundary Detection")
                description: i18n("Finds the borders of static windows and softens the light along them.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "outer_dimming"
                title: i18n("Outer Dimming Control")
                description: i18n("Lowers brightness toward the outer area of the panel.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "global_dimming"
                title: i18n("Global Dimming Control")
                description: i18n("Lets the monitor adjust brightness across the whole panel for the content.")
            }
            ToggleRow {
                width: parent.width
                store: page.store
                fid: "uniform_brightness"
                title: i18n("Uniform Brightness")
                description: i18n("Keeps the brightness level steady from scene to scene.")
            }
            SettingRow {
                id: targetMode
                width: parent.width
                title: i18n("Target Mode")
                description: i18n("Dims every window except the one you are using.")
                dim: !page.targetKnown
                RogToggle {
                    objectName: "toggle_target_mode"
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.targetKnown
                    checked: page.targetOn
                    onToggled: on => page.host.setTargetMode(page.store.monitorId, on)
                }
            }
            SettingRow {
                id: cleaning
                width: parent.width
                title: i18n("Pixel Cleaning")
                description: i18n("A panel refresh cycle of about six minutes; the screen goes dark while it runs.")
                dim: !page.store.isEnabled("pixel_cleaning")
                RogButton {
                    objectName: "pixel_cleaning"
                    anchors.verticalCenter: parent.verticalCenter
                    text: i18n("Apply")
                    enabled: page.store.isEnabled("pixel_cleaning")
                    onClicked: page.store.commit("pixel_cleaning", 1)
                }
            }
            ComboRow {
                width: parent.width
                store: page.store
                fid: "pixel_cleaning_reminder"
                title: i18n("Pixel Cleaning Reminder")
                description: i18n("How often the monitor reminds you to run pixel cleaning.")
            }
            ComboRow {
                width: parent.width
                store: page.store
                fid: "screen_move"
                title: i18n("Screen Move")
                description: i18n("Shifts the image by a few pixels now and then so no pixel stays lit in one place.")
                divider: false
            }
        }
    }
}
