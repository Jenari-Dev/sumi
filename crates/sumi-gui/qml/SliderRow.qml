import QtQuick

// A settings row with a slider bound to a range feature.
SettingRow {
    id: root

    property var store: null
    property string fid: ""
    property var tint: null
    // Optional formatter for the value label: function(value) -> string
    property var format: null

    readonly property bool known: store !== null && store.isSupported(fid)
    readonly property real current: known && store.value(fid) !== undefined ? store.value(fid) : 0

    tag: store && store.isExtra(fid) ? "OSD" : ""
    dim: !known || !store.isEnabled(fid)

    RogSlider {
        objectName: "slider_" + root.fid
        anchors.verticalCenter: parent.verticalCenter
        width: root.compact ? root.width - root.labelWidth - 6 : 320
        enabled: root.known && root.store.isEnabled(root.fid)
        from: 0
        to: root.known ? root.store.maxOf(root.fid) : 100
        value: root.current
        tint: root.tint
        valueText: !root.known ? "—" : (root.format ? root.format(shown) : "")
        onMoved: v => root.store.write(root.fid, v)
        onReleased: v => root.store.settle(root.fid, v)
    }
}
