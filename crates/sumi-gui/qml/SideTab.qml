import QtQuick

// Sidebar entry: 237 x 48, a 24 px line icon and a 14 px label. Hover shows
// the red bar and gradient at 45 %, checked at full strength.
Item {
    id: root

    property string icon: ""
    property string label: ""
    property bool checked: false
    readonly property bool hovered: hover.hovered && enabled
    signal clicked

    implicitWidth: 237
    implicitHeight: 48
    activeFocusOnTab: true
    opacity: enabled ? 1 : 0.4

    readonly property real strength: checked ? 1 : (hovered || activeFocus ? 0.45 : 0)

    Rectangle {
        anchors.fill: parent
        opacity: root.strength
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: "#66FF1929" }
            GradientStop { position: 1.0; color: "#1AFF1929" }
        }
    }
    Rectangle {
        width: 3
        height: parent.height
        color: Theme.red
        opacity: root.strength
    }
    Icon {
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        size: 24
        lineWidth: 1.4
        name: root.icon
        color: root.checked ? Theme.text : (root.hovered ? Theme.text : Theme.textSoft)
    }
    Text {
        x: 58
        width: root.width - x - 10
        anchors.verticalCenter: parent.verticalCenter
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: 10
        elide: Text.ElideRight
        text: root.label
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
        font.weight: root.checked ? Font.Medium : Font.Normal
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
