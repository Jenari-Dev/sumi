pragma ComponentBehavior: Bound

import QtQuick

// Little drawings of the crosshair styles (our own, not the monitor's).
// kind: "off", "dot", "mini", "heavy" or "plain" (a style we have no name for).
Item {
    id: root

    property string kind: "dot"
    property color tint: "#2B8CFF"

    width: 56
    height: 56

    readonly property real armLength: kind === "heavy" ? 16 : 9
    readonly property real armWidth: kind === "heavy" ? 3 : 2
    readonly property real armGap: kind === "heavy" ? 7 : 6
    readonly property real armSpread: kind === "heavy" ? 7 : 4

    Icon {
        visible: root.kind === "off"
        anchors.centerIn: parent
        size: 34
        lineWidth: 1.6
        name: "off"
        color: Theme.textDim
    }

    // Centre dot (dot, mini, heavy)
    Rectangle {
        visible: root.kind === "dot" || root.kind === "mini" || root.kind === "heavy"
        anchors.centerIn: parent
        width: root.kind === "dot" ? 11 : (root.kind === "mini" ? 4 : 5)
        height: width
        radius: width / 2
        color: root.tint
    }

    // Two parallel bars per arm (mini, heavy)
    Repeater {
        model: root.kind === "mini" || root.kind === "heavy" ? 4 : 0
        delegate: Item {
            id: arm
            required property int index
            anchors.centerIn: parent
            width: 56
            height: 56
            rotation: index * 90
            Rectangle {
                x: 28 - root.armSpread / 2 - root.armWidth
                y: 28 - root.armGap - root.armLength
                width: root.armWidth
                height: root.armLength
                color: root.tint
            }
            Rectangle {
                x: 28 + root.armSpread / 2
                y: 28 - root.armGap - root.armLength
                width: root.armWidth
                height: root.armLength
                color: root.tint
            }
        }
    }

    // Unknown style: a faint thin cross inside a ring
    Rectangle {
        visible: root.kind === "plain"
        anchors.centerIn: parent
        width: 30
        height: 30
        radius: 15
        color: "transparent"
        border.width: 1.5
        border.color: Theme.textDim
    }
    Repeater {
        model: root.kind === "plain" ? 2 : 0
        delegate: Rectangle {
            required property int index
            anchors.centerIn: parent
            width: index === 0 ? 44 : 1.5
            height: index === 0 ? 1.5 : 44
            color: Theme.textDim
        }
    }
}
