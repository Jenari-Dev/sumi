import QtQuick

// 32 px black title bar: title left, minimise and close (48 x 32) right.
// Dragging the bar moves the window (handled by the owner via `dragStarted`).
Item {
    id: root

    property string title: "Sumi"
    signal dragStarted
    signal minimiseRequested
    signal closeRequested

    width: Theme.designWidth
    height: Theme.titleHeight

    Rectangle {
        anchors.fill: parent
        color: Theme.black
    }
    Text {
        x: 12
        anchors.verticalCenter: parent.verticalCenter
        text: root.title
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontSmall
    }

    DragHandler {
        target: null
        onActiveChanged: if (active) root.dragStarted()
    }

    component CaptionButton: Item {
        id: btn
        property string icon: ""
        property color hoverColor: Theme.alpha(Theme.text, 0.12)
        signal clicked
        width: 48
        height: 32
        Rectangle {
            anchors.fill: parent
            color: btn.hoverColor
            opacity: hover.hovered ? (tap.pressed ? 0.6 : 1) : 0
        }
        Icon {
            anchors.centerIn: parent
            size: 16
            lineWidth: 1.2
            name: btn.icon
            color: Theme.text
        }
        HoverHandler {
            id: hover
        }
        TapHandler {
            id: tap
            onTapped: btn.clicked()
        }
    }

    CaptionButton {
        objectName: "minimise"
        x: parent.width - 96
        icon: "minimise"
        onClicked: root.minimiseRequested()
    }
    CaptionButton {
        objectName: "close"
        x: parent.width - 48
        icon: "close"
        hoverColor: Theme.alpha(Theme.red, 0.4)
        onClicked: root.closeRequested()
    }
}
