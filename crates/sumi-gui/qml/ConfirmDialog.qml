import QtQuick
import QtQuick.Controls as QQC2

// Modal dialog: dark panel, grey border, corner brackets, Cancel + Confirm.
// `showCancel: false` makes it a plain notice (one button); `large` is a bigger
// panel whose text scrolls (used for logs).
Item {
    id: root

    property string title: ""
    property string text: ""
    property string confirmText: i18n("Confirm")
    property string cancelText: i18n("Cancel")
    property bool showCancel: true
    property bool large: false
    signal accepted
    signal rejected

    visible: false
    focus: visible
    onVisibleChanged: if (visible) forceActiveFocus()

    Keys.onEscapePressed: root.rejected()
    Keys.onReturnPressed: root.accepted()
    Keys.onEnterPressed: root.accepted()

    Rectangle {
        anchors.fill: parent
        color: Theme.dim
    }
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.showCancel ? root.rejected() : undefined
    }

    Item {
        id: panel
        anchors.centerIn: parent
        width: root.large ? 680 : 484
        height: root.large ? 460 : 252

        MouseArea {
            anchors.fill: parent
        }
        Rectangle {
            anchors.fill: parent
            color: Theme.dialog
            border.width: 1
            border.color: Theme.borderButton
        }
        Brackets {
            anchors.fill: parent
            anchors.margins: -1
            size: 12
            thickness: 2
            color: Theme.red
        }

        Text {
            x: 32
            y: 28
            width: parent.width - 64
            text: root.title
            elide: Text.ElideRight
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: 18
            font.bold: true
        }
        Rectangle {
            x: 32
            y: 66
            width: parent.width - 64
            height: 1
            color: Theme.separator
            opacity: 0.5
        }
        Flickable {
            id: body
            x: 32
            y: 84
            width: parent.width - 64
            height: parent.height - 84 - 84
            clip: true
            contentWidth: width
            contentHeight: message.height
            boundsBehavior: Flickable.StopAtBounds
            QQC2.ScrollBar.vertical: RogScrollBar {}

            Text {
                id: message
                width: body.width - (root.large ? 14 : 0)
                text: root.text
                wrapMode: Text.WordWrap
                color: Theme.textSoft
                font.family: root.large ? "monospace" : Theme.font
                font.pixelSize: root.large ? Theme.fontSmall + 1 : Theme.fontBody + 1
                lineHeight: root.large ? 1.1 : 1.2
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 32
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 24
            spacing: 14
            RogButton {
                objectName: "cancel"
                visible: root.showCancel
                text: root.cancelText
                secondary: true
                onClicked: root.rejected()
            }
            RogButton {
                objectName: "confirm"
                text: root.confirmText
                onClicked: root.accepted()
            }
        }
    }
}
