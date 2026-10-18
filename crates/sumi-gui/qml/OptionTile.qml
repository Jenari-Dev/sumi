import QtQuick

// A wide selectable tile: a glyph on the left, a label on the right. The glyph
// is whatever is declared inside the tile.
Item {
    id: root

    property string label: ""
    property string sublabel: ""
    property bool checked: false
    property int glyphWidth: 76
    default property alias glyph: glyphBox.data
    readonly property bool hovered: hover.hovered && enabled
    signal clicked

    implicitWidth: 298
    implicitHeight: 76
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

    Item {
        id: glyphBox
        width: root.glyphWidth
        height: parent.height
        opacity: root.enabled ? 1 : 0.4
    }
    Column {
        anchors.left: glyphBox.right
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text {
            width: parent.width
            text: root.label
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: 10
            elide: Text.ElideRight
            color: root.enabled ? Theme.text : Theme.disabledText
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
            font.weight: root.checked ? Font.Medium : Font.Normal
        }
        Text {
            visible: root.sublabel !== ""
            width: parent.width
            text: root.sublabel
            fontSizeMode: Text.HorizontalFit
            minimumPixelSize: 9
            elide: Text.ElideRight
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontSmall
        }
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
