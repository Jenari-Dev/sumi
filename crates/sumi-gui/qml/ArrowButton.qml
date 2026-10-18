import QtQuick

// 48 x 48 arrow key. `step()` fires on press and then every 150 ms while held.
Item {
    id: root

    property real direction: 0   // rotation of the arrow: 0 up, 90 right, 180 down, 270 left
    readonly property bool down: area.pressed
    readonly property bool hovered: area.containsMouse
    signal step

    implicitWidth: 48
    implicitHeight: 48
    opacity: enabled ? 1 : 0.5

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
        color: root.hovered || root.down ? Theme.hoverEdge : Theme.text
    }
    Icon {
        anchors.centerIn: parent
        size: 22
        lineWidth: 1.5
        name: "arrow"
        rotation: root.direction
        color: !root.enabled ? Theme.disabledGlyph : (root.hovered ? Theme.hoverEdge : Theme.text)
    }

    Timer {
        interval: 150
        repeat: true
        running: area.pressed
        onTriggered: root.step()
    }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onPressed: root.step()
    }
}
