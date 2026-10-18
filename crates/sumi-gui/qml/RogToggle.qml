import QtQuick

// 52 x 20 outline capsule with OFF / ON text; the knob slides in 0.2 s.
// `checked` is driven from outside; `toggled(on)` fires only on user input.
Item {
    id: root

    property bool checked: false
    readonly property bool hovered: hover.hovered
    readonly property bool down: tap.pressed
    signal toggled(bool on)

    // Long words for ON / OFF (AKTIF / NONAKTIF ...) widen the capsule; 52 px for the usual ones.
    implicitWidth: Math.max(52, Math.ceil(Math.max(onMetrics.advanceWidth, offMetrics.advanceWidth)) + 14 + 3 + 3 + 4 + 7)
    implicitHeight: 20
    activeFocusOnTab: true

    readonly property color edge: !enabled ? Theme.disabledText
        : (down ? Theme.pressedBright : (hovered || activeFocus ? Theme.hoverEdge : Theme.text))

    Rectangle {
        anchors.fill: parent
        radius: 4
        color: root.checked ? (root.enabled ? root.edge : Theme.disabledText) : "transparent"
        border.width: 1
        border.color: root.edge
    }

    TextMetrics {
        id: onMetrics
        text: i18nc("@label state of a switch", "ON")
        font.family: Theme.font
        font.pixelSize: 9
        font.bold: true
    }
    TextMetrics {
        id: offMetrics
        text: i18nc("@label state of a switch", "OFF")
        font.family: Theme.font
        font.pixelSize: 9
        font.bold: true
    }

    Text {
        text: root.checked ? i18nc("@label state of a switch", "ON") : i18nc("@label state of a switch", "OFF")
        x: root.checked ? 7 : root.width - width - 7
        anchors.verticalCenter: parent.verticalCenter
        color: root.checked ? Theme.black : root.edge
        font.family: Theme.font
        font.pixelSize: 9
        font.bold: true
    }

    Rectangle {
        id: knob
        width: 14
        height: 14
        radius: 2
        y: 3
        x: root.checked ? root.width - width - 3 : 3
        color: root.checked ? Theme.black : root.edge
        Behavior on x {
            NumberAnimation { duration: 200; easing.type: Easing.InOutQuad }
        }
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        id: tap
        onTapped: root.toggled(!root.checked)
    }
    Keys.onSpacePressed: root.toggled(!root.checked)
    Keys.onReturnPressed: root.toggled(!root.checked)
}
