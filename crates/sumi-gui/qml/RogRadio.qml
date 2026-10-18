import QtQuick

// Radio button drawn as a small diamond, with a label to its right.
Item {
    id: root

    property bool checked: false
    property string text: ""
    readonly property bool hovered: hover.hovered
    signal clicked

    implicitWidth: 18 + 10 + label.implicitWidth
    implicitHeight: 24
    activeFocusOnTab: true

    readonly property color edge: !enabled ? Theme.disabledText : (hovered || activeFocus ? Theme.hoverEdge : (checked ? Theme.red : Theme.text))

    Item {
        id: mark
        width: 18
        height: 18
        anchors.verticalCenter: parent.verticalCenter
        Rectangle {
            anchors.centerIn: parent
            width: 13
            height: 13
            rotation: 45
            color: "transparent"
            border.width: 1.5
            border.color: root.edge
        }
        Rectangle {
            anchors.centerIn: parent
            visible: root.checked
            width: 7
            height: 7
            rotation: 45
            color: root.enabled ? Theme.red : Theme.disabledText
        }
    }
    Text {
        id: label
        anchors.left: mark.right
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        text: root.text
        color: root.enabled ? Theme.text : Theme.disabledText
        font.family: Theme.font
        font.pixelSize: Theme.fontLabel
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        onTapped: root.clicked()
    }
    Keys.onSpacePressed: root.clicked()
    Keys.onReturnPressed: root.clicked()
}
