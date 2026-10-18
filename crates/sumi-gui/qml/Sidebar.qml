pragma ComponentBehavior: Bound

import QtQuick

// 240 px sidebar: wordmark, monitor selector, page tabs.
Item {
    id: root

    property var store: null
    property var backend: null
    property var monitors: []
    property string page: ""
    property bool oled: false
    property bool powerSaving: false
    signal pageSelected(string id)
    signal monitorSelected(int index)

    width: Theme.sidebarWidth
    height: Theme.pageHeight

    readonly property var deviceTabs: {
        const t = [
            { id: "game_visual", label: i18n("GameVisual"), icon: "gamevisual" },
            { id: "game_plus", label: i18n("GamePlus"), icon: "gameplus", lockedByPower: true }
        ];
        if (oled) {
            t.push({ id: "oled_care", label: i18n("OLED Care"), icon: "oled", lockedByPower: true });
        }
        t.push({ id: "multi_screen", label: i18n("MultiScreen"), icon: "multiscreen" });
        t.push({ id: "hotkey", label: i18n("HotKey"), icon: "hotkey" });
        t.push({ id: "app_tweaker", label: i18n("App Tweaker"), icon: "apptweaker" });
        t.push({ id: "system", label: i18n("System Settings"), icon: "system" });
        return t;
    }

    // ---- Wordmark
    Item {
        x: 22
        y: 16
        width: 200
        height: 60

        Icon {
            id: mark
            size: 56
            lineWidth: 1.6
            name: "sumimark"
            color: Theme.text
        }
        Icon {
            size: 56
            lineWidth: 0
            name: "sumidrop"
            color: "transparent"
            fill: Theme.red
        }
        Text {
            x: 66
            anchors.verticalCenter: mark.verticalCenter
            anchors.verticalCenterOffset: -4
            text: i18n("Sumi")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: 30
            font.bold: true
            font.letterSpacing: 1
        }
        Text {
            x: 67
            // Up to the sidebar's right margin; long translations lose letter spacing, then size.
            width: parent.width - x
            anchors.verticalCenter: mark.verticalCenter
            anchors.verticalCenterOffset: 20
            text: i18n("Monitor settings")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: 9
            font.letterSpacing: text.length > 24 ? 0 : (text.length > 16 ? 1 : 2)
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: 7
            elide: Text.ElideRight
            font.capitalization: Font.AllUppercase
        }
    }

    // ---- Monitor selector ("1.PG32UCDM3")
    RogCombo {
        objectName: "monitor_combo"
        x: 20
        y: 88
        width: 200
        enabled: root.monitors.length >= 2
        model: root.monitors.map((m, i) => (i + 1) + "." + m.model)
        currentIndex: root.store ? root.monitors.findIndex(m => m.id === root.store.monitorId) : -1
        placeholder: root.monitors.length === 0 ? i18n("No monitor") : "—"
        onActivated: i => root.monitorSelected(i)
    }

    // ---- Tabs
    Column {
        y: 151
        Repeater {
            model: root.deviceTabs
            delegate: SideTab {
                required property var modelData
                objectName: "tab_" + modelData.id
                icon: modelData.icon
                label: modelData.label
                checked: root.page === modelData.id
                enabled: !(modelData.lockedByPower === true && root.powerSaving)
                onClicked: root.pageSelected(modelData.id)
            }
        }
    }

    Rectangle {
        x: 20
        y: 593
        width: 200
        height: 1
        color: Theme.text
        opacity: 0.2
    }

    Column {
        y: 610
        SideTab {
            objectName: "tab_user_center"
            icon: "usercenter"
            label: i18n("User Center")
            checked: root.page === "user_center"
            onClicked: root.pageSelected("user_center")
        }
        SideTab {
            objectName: "tab_app_settings"
            icon: "appsettings"
            label: i18n("Application Settings")
            checked: root.page === "app_settings"
            onClicked: root.pageSelected("app_settings")
        }
    }

    // ---- Daemon status
    Row {
        x: 22
        y: root.height - 36
        spacing: 8
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 7
            height: 7
            rotation: 45
            color: !root.backend || !root.backend.connected ? Theme.error : (root.backend.readonly ? Theme.warn : Theme.ok)
        }
        Text {
            text: !root.backend || !root.backend.connected ? i18n("sumid not running") : (root.backend.readonly ? i18n("sumid · read-only") : i18n("sumid connected"))
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontSmall
        }
    }
}
