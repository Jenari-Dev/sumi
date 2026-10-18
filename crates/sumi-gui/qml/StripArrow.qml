import QtQuick

// 34 x 56 arrow button at the ends of the mode strip. `direction`: -1 left, 1 right.
Item {
    id: root

    property int direction: 1
    readonly property bool hovered: hover.hovered && enabled
    readonly property bool down: tap.pressed
    signal clicked

    implicitWidth: 34
    implicitHeight: 56
    opacity: enabled ? 1 : 0.4

    Rectangle {
        anchors.fill: parent
        color: root.down ? Theme.alpha(Theme.pressed, 0.55) : (root.hovered ? Theme.alpha(Theme.hoverFill, 0.35) : Theme.black)
        border.width: 1
        border.color: !root.enabled ? Theme.disabledBorder : (root.down ? Theme.pressed : (root.hovered ? Theme.hoverEdge : Theme.borderButton))
    }
    Brackets {
        anchors.fill: parent
        size: 6
        thickness: 1.5
        color: root.hovered || root.down ? Theme.hoverEdge : (root.enabled ? Theme.text : Theme.disabledText)
    }
    Icon {
        anchors.centerIn: parent
        size: 24
        lineWidth: 1.8
        name: "chevron"
        rotation: root.direction < 0 ? 90 : -90
        color: !root.enabled ? Theme.disabledGlyph : (root.hovered ? Theme.hoverEdge : Theme.text)
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        id: tap
        onTapped: root.clicked()
    }
}
