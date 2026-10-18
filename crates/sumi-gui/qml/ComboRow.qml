import QtQuick

// A settings row with a drop-down bound to a choice feature.
SettingRow {
    id: root

    property var store: null
    property string fid: ""

    readonly property bool known: store !== null && store.isSupported(fid)

    tag: store && store.isExtra(fid) ? "OSD" : ""
    dim: !known || !store.isEnabled(fid)

    RogCombo {
        objectName: "combo_" + root.fid
        anchors.verticalCenter: parent.verticalCenter
        width: root.compact ? root.width - root.labelWidth - 6 : 296
        enabled: root.known && root.store.isEnabled(root.fid)
        model: root.known ? root.store.displayOptions(root.fid) : []
        currentIndex: root.known ? root.store.indexOfValue(root.fid) : -1
        placeholder: root.known ? "—" : i18n("Not available")
        onActivated: i => root.store.commit(root.fid, root.store.options(root.fid)[i].value)
    }
}
