import QtQuick

// One settings row: title (and description) on the left, the control on the
// right. `compact` rows (inside cards) put a short label left of a control
// that fills the rest of the row. `tag` adds a small marker ("OSD") for
// settings that are Sumi's own additions.
Item {
    id: root

    property string title: ""
    property string description: ""
    property string tag: ""
    property bool dim: false
    property bool compact: false
    property bool divider: true
    property int labelWidth: 118
    default property alias control: slot.data

    // A long translated description wraps to a second line and the row grows with it.
    implicitHeight: compact ? 48 : Math.max(58, texts.height + 14)

    readonly property color titleColor: dim ? Theme.disabledText : Theme.text

    Column {
        id: texts
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        width: root.compact ? root.labelWidth : Math.max(60, root.width - slot.width - 32)
        spacing: 2

        Row {
            spacing: 8
            Text {
                id: titleText
                text: root.title
                width: Math.min(implicitWidth, texts.width - (tagBox.visible ? tagBox.width + 8 : 0))
                // A title that is too long for its column shrinks (two lines in the narrow label
                // column of compact rows), and is cut only below the minimum size.
                height: root.compact ? 34 : implicitHeight
                verticalAlignment: Text.AlignVCenter
                wrapMode: root.compact ? Text.WordWrap : Text.NoWrap
                fontSizeMode: root.compact ? Text.Fit : Text.HorizontalFit
                minimumPixelSize: root.compact ? 10 : 12
                elide: Text.ElideRight
                color: root.titleColor
                font.family: Theme.font
                font.pixelSize: root.compact ? Theme.fontBody : Theme.fontLabel
                font.weight: Font.Medium
            }
            Rectangle {
                id: tagBox
                visible: root.tag !== ""
                anchors.verticalCenter: titleText.verticalCenter
                width: tagText.implicitWidth + 8
                height: 14
                color: "transparent"
                border.width: 1
                border.color: root.dim ? Theme.disabledBorder : Theme.borderTile
                Text {
                    id: tagText
                    anchors.centerIn: parent
                    text: root.tag
                    color: root.dim ? Theme.disabledText : Theme.textDim
                    font.family: Theme.font
                    font.pixelSize: 9
                    font.bold: true
                }
            }
        }
        Text {
            visible: root.description !== "" && !root.compact
            width: texts.width
            text: root.description
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: root.dim ? Theme.disabledText : Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
    }

    Item {
        id: slot
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: root.compact ? root.width - root.labelWidth - 6 : childrenRect.width
        height: root.height
    }

    Rectangle {
        visible: root.divider
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: Theme.separator
        opacity: 0.4
    }
}
