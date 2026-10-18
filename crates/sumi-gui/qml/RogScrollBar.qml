import QtQuick
import QtQuick.Controls as QQC2

// Thin scroll bar (about 5 px) on a faint track.
QQC2.ScrollBar {
    id: control

    implicitWidth: 6
    padding: 0
    minimumSize: 0.1

    background: Rectangle {
        color: Theme.alpha("#D9D9D9", 0.14)
        visible: control.size < 1
    }
    contentItem: Rectangle {
        implicitWidth: 6
        visible: control.size < 1
        color: control.pressed ? Theme.hoverEdge : (control.hovered ? "#D9D9D9" : "#8A8A8A")
    }
}
