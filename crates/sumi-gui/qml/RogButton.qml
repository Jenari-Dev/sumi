import QtQuick

// Primary button: black body, thin grey border, angular corner brackets.
// Hover turns teal, pressed crimson.
Item {
    id: root

    property string text: ""
    property bool secondary: false
    // The button is as wide as its label needs, but never narrower than this.
    property real minWidth: 136
    property real padding: 18
    readonly property bool hovered: hover.hovered
    readonly property bool down: tap.pressed
    signal clicked

    implicitWidth: Math.max(minWidth, Math.ceil(metrics.advanceWidth) + 2 * padding)
    implicitHeight: 32
    activeFocusOnTab: true
    opacity: enabled ? 1 : 0.55

    Rectangle {
        id: body
        anchors.fill: parent
        color: root.down ? Theme.alpha(Theme.pressed, 0.55) : (root.hovered ? Theme.alpha(Theme.hoverFill, 0.35) : Theme.black)
        border.width: 1
        border.color: !root.enabled ? Theme.disabledBorder : (root.down ? Theme.pressed : (root.hovered ? Theme.hoverEdge : (root.activeFocus ? Theme.hoverFill : Theme.borderButton)))

        // Soft top-down gloss
        Rectangle {
            anchors.fill: parent
            anchors.margins: 1
            gradient: Gradient {
                GradientStop { position: 0.0; color: root.secondary ? "#55FFFFFF" : "#304D4D4D" }
                GradientStop { position: 0.55; color: "#00000000" }
            }
        }
    }

    Brackets {
        anchors.fill: parent
        size: 7
        color: !root.enabled ? Theme.disabledText : (root.hovered || root.down ? Theme.hoverEdge : Theme.text)
    }

    TextMetrics {
        id: metrics
        text: root.text
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    // A fixed width (set by the page) is respected: the label then shrinks to fit.
    Text {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.width - 2 * 8)
        horizontalAlignment: Text.AlignHCenter
        text: root.text
        elide: Text.ElideRight
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: 10
        color: root.enabled ? Theme.text : Theme.disabledText
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        id: tap
        onTapped: root.clicked()
    }
    Keys.onReturnPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()
}
