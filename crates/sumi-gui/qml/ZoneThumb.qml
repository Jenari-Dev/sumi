import QtQuick

// Thumbnail of one of the 13 MultiFrame zone layouts (0 = none).
Item {
    id: root

    property int layout: 0
    property bool checked: false
    property bool hovered: false

    width: 56
    height: 40

    // Zone rectangles [x, y, w, h] as fractions of the screen.
    function zonesFor(n) {
        const t = 1 / 3, tt = 2 / 3;
        switch (n) {
        case 1: return [[0, 0, 1, 0.5], [0, 0.5, 1, 0.5]];
        case 2: return [[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]];
        case 3: return [[0, 0, tt, 1], [tt, 0, t, 1]];
        case 4: return [[0, 0, t, 1], [t, 0, tt, 1]];
        case 5: return [[0, 0, 1, tt], [0, tt, 1, t]];
        case 6: return [[0, 0, 1, t], [0, t, 1, tt]];
        case 7: return [[0, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0, 0.5, 1]];
        case 8: return [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]];
        case 9: return [[0, 0, 1, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]];
        case 10: return [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 1, 0.5]];
        case 11: return [[0, 0, t, 1], [t, 0, t, 1], [tt, 0, t, 1]];
        case 12: return [[0, 0, 1, t], [0, t, 1, t], [0, tt, 1, t]];
        case 13: return [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]];
        default: return [];
        }
    }

    readonly property var zones: zonesFor(layout)
    readonly property color line: checked ? Theme.red : (hovered ? Theme.hoverEdge : Theme.text)

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: 1.5
        border.color: root.line
    }
    Repeater {
        model: root.zones
        delegate: Rectangle {
            required property var modelData
            x: 2 + modelData[0] * (root.width - 4)
            y: 2 + modelData[1] * (root.height - 4)
            width: modelData[2] * (root.width - 4)
            height: modelData[3] * (root.height - 4)
            color: Theme.alpha(root.line, 0.14)
            border.width: 1
            border.color: Theme.alpha(root.line, 0.8)
        }
    }
    // None: a slash through an empty screen
    Icon {
        visible: root.layout === 0
        anchors.centerIn: parent
        size: 26
        lineWidth: 1.4
        name: "off"
        color: Theme.textDim
    }
}
