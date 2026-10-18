import QtQuick

// Titled panel: translucent black body with a thin grey border.
Item {
    id: root

    property string title: ""
    property int headerHeight: 46
    default property alias content: body.data
    readonly property alias body: body

    Rectangle {
        anchors.fill: parent
        color: Theme.alpha(Theme.black, 0.3)
        border.width: 1
        border.color: Theme.borderTile
    }
    Text {
        x: 20
        y: 0
        height: root.headerHeight
        verticalAlignment: Text.AlignVCenter
        text: root.title
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontLabel
        font.weight: Font.Medium
    }
    Rectangle {
        visible: root.title !== ""
        x: 1
        y: root.headerHeight - 1
        width: parent.width - 2
        height: 1
        color: Theme.separator
        opacity: 0.55
    }
    Item {
        id: body
        x: 0
        y: root.title !== "" ? root.headerHeight : 0
        width: parent.width
        height: parent.height - y
    }
}
