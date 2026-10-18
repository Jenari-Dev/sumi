import QtQuick

// A settings row with an on/off switch bound to a bit (or two-value) feature.
// `onValue` / `offValue` map the switch to the feature's values.
SettingRow {
    id: root

    property var store: null
    property string fid: ""
    property int onValue: 1
    property int offValue: 0

    readonly property bool known: store !== null && store.isSupported(fid)

    tag: store && store.isExtra(fid) ? "OSD" : ""
    dim: !known || !store.isEnabled(fid)

    RogToggle {
        objectName: "toggle_" + root.fid
        anchors.verticalCenter: parent.verticalCenter
        enabled: root.known && root.store.isEnabled(root.fid)
        checked: root.known && root.store.value(root.fid) === root.onValue
        onToggled: on => root.store.commit(root.fid, on ? root.onValue : root.offValue)
    }
}
