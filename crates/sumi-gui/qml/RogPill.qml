import QtQuick

// Small selectable pill (HDR sub-modes). 104 x 20 by default.
Item {
    id: root

    property string text: ""
    property bool checked: false
    readonly property bool hovered: hover.hovered
    signal clicked

    // 104 px, or as wide as the label needs.
    implicitWidth: Math.max(104, Math.ceil(metrics.advanceWidth) + 16)
    implicitHeight: 20
    activeFocusOnTab: true

    TextMetrics {
        id: metrics
        text: root.text
        font.family: Theme.font
        font.pixelSize: Theme.fontSmall
    }

    Rectangle {
        anchors.fill: parent
        color: !root.enabled ? Theme.disabledFill : (root.checked ? Theme.alpha(Theme.pressed, 0.35) : (root.hovered ? Theme.alpha(Theme.hoverFill, 0.3) : "transparent"))
        border.width: 1
        border.color: !root.enabled ? Theme.disabledBorder : (root.checked ? Theme.red : (root.hovered ? Theme.hoverEdge : Theme.borderTile))
    }
    Text {
        anchors.centerIn: parent
        width: parent.width - 8
        horizontalAlignment: Text.AlignHCenter
        text: root.text
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: 9
        elide: Text.ElideRight
        color: !root.enabled ? Theme.disabledText : Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontSmall
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        onTapped: root.clicked()
    }
    Keys.onSpacePressed: root.clicked()
}
