import QtQuick

// Check box drawn as a small square with a line check mark, label on the right.
Item {
    id: root

    property bool checked: false
    property string text: ""
    readonly property bool hovered: hover.hovered
    signal toggled(bool on)

    implicitWidth: 22 + 8 + label.implicitWidth
    implicitHeight: 24
    activeFocusOnTab: true

    readonly property color edge: !enabled ? Theme.disabledText : (hovered || activeFocus ? Theme.hoverEdge : Theme.text)

    Rectangle {
        id: box
        width: 16
        height: 16
        anchors.verticalCenter: parent.verticalCenter
        color: root.checked && root.enabled ? Theme.alpha(Theme.red, 0.25) : "transparent"
        border.width: 1.5
        border.color: root.checked && root.enabled ? Theme.red : root.edge
        Icon {
            visible: root.checked
            anchors.centerIn: parent
            size: 14
            lineWidth: 1.8
            name: "check"
            color: root.enabled ? Theme.text : Theme.disabledText
        }
    }
    Text {
        id: label
        anchors.left: box.right
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: root.text
        color: root.enabled ? Theme.text : Theme.disabledText
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        onTapped: root.toggled(!root.checked)
    }
    Keys.onSpacePressed: root.toggled(!root.checked)
}
