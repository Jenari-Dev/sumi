import QtQuick

// Feature tile: line icon above a caption. Hover tints it teal, checked
// crimson. `locked` keeps a status tile (HDR) checked and non-clickable.
Item {
    id: root

    property string icon: ""
    property string caption: ""
    property bool checked: false
    property bool locked: false
    readonly property bool hovered: hover.hovered && enabled && !locked
    signal clicked

    implicitWidth: 108
    implicitHeight: 104
    activeFocusOnTab: true

    Rectangle {
        anchors.fill: parent
        color: !root.enabled ? Theme.disabledFill : Theme.alpha(Theme.black, 0.12)
        border.width: 1
        border.color: !root.enabled ? Theme.disabledBorder
            : (root.checked ? Theme.pressed : (root.hovered || root.activeFocus ? Theme.hoverFill : Theme.borderTile))
        gradient: root.checked && root.enabled ? checkedFill : (root.hovered ? hoverFill : null)

        Gradient {
            id: hoverFill
            GradientStop { position: 0.0; color: "#00000000" }
            GradientStop { position: 1.0; color: "#66148AC4" }
        }
        Gradient {
            id: checkedFill
            GradientStop { position: 0.0; color: "#00000000" }
            GradientStop { position: 1.0; color: "#66FF1929" }
        }
    }

    Brackets {
        anchors.fill: parent
        anchors.margins: 3
        corners: "bottom"
        size: 6
        thickness: 1
        color: !root.enabled ? Theme.disabledBorder : (root.checked ? Theme.red : (root.hovered ? Theme.hoverEdge : Theme.borderTile))
    }

    Icon {
        anchors.horizontalCenter: parent.horizontalCenter
        y: 14
        size: 48
        lineWidth: 1.6
        name: root.icon
        color: !root.enabled ? Theme.disabledGlyph : (root.hovered ? Theme.hoverEdge : Theme.text)
        fill: root.checked && root.enabled ? Theme.alpha(Theme.red, 0.22) : "transparent"
    }

    Text {
        x: 5
        y: 64
        width: parent.width - 10
        height: 32
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        text: root.caption
        wrapMode: Text.WordWrap
        fontSizeMode: Text.Fit
        minimumPixelSize: 9
        elide: Text.ElideRight
        maximumLineCount: 2
        color: !root.enabled ? Theme.disabledText : Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontSmall
    }

    HoverHandler {
        id: hover
        cursorShape: root.locked ? Qt.ArrowCursor : Qt.PointingHandCursor
    }
    TapHandler {
        enabled: !root.locked
        onTapped: root.clicked()
    }
    Keys.onSpacePressed: if (!root.locked) root.clicked()
    Keys.onReturnPressed: if (!root.locked) root.clicked()
}
