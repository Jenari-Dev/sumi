import QtQuick

// Slim bar for errors (red) and notices (amber). Errors can be dismissed.
Item {
    id: root

    property string text: ""
    property bool error: false
    signal dismissed

    implicitHeight: 34
    visible: text !== ""

    Rectangle {
        anchors.fill: parent
        color: "#F2101010"
        border.width: 1
        border.color: root.error ? Theme.alpha(Theme.error, 0.7) : Theme.alpha(Theme.warn, 0.6)
    }
    Rectangle {
        width: 3
        height: parent.height
        color: root.error ? Theme.error : Theme.warn
    }
    Text {
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.right: closeBox.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: root.text
        elide: Text.ElideRight
        color: root.error ? "#FFB4B4" : "#FFE2A8"
        font.family: Theme.font
        font.pixelSize: 13
    }
    Item {
        id: closeBox
        visible: root.error
        anchors.right: parent.right
        width: 34
        height: parent.height
        Icon {
            anchors.centerIn: parent
            size: 14
            lineWidth: 1.4
            name: "close"
            color: closeHover.hovered ? Theme.hoverEdge : Theme.textDim
        }
        HoverHandler {
            id: closeHover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: root.dismissed()
        }
    }
}
