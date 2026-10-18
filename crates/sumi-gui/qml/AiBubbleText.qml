import QtQuick

// The message bubble next to the AI Visual orb: a dark panel (85 % opaque) with a red
// accent bar on the left and white text. flash(text) fades it in, keeps it for
// `holdMs` and fades it out again. Pure drawing, no layer-shell.
Item {
    id: bubble

    property string text: ""
    property bool shown: false
    property int holdMs: 4000
    property int fadeInMs: 200
    property int fadeOutMs: 500

    readonly property int barWidth: 4

    implicitWidth: barWidth + 14 + label.implicitWidth + 16
    implicitHeight: 44
    width: implicitWidth
    height: implicitHeight
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation { duration: bubble.shown ? bubble.fadeInMs : bubble.fadeOutMs; easing.type: Easing.OutQuad }
    }

    function flash(message) {
        text = message;
        shown = true;
        hold.interval = holdMs;
        hold.restart();
    }

    function hideNow() {
        hold.stop();
        shown = false;
    }

    Timer {
        id: hold
        onTriggered: bubble.shown = false
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0.043, 0.043, 0.051, 0.85)
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.10)
    }
    Rectangle {
        width: bubble.barWidth
        height: parent.height
        color: Theme.red
    }
    Text {
        id: label
        x: bubble.barWidth + 14
        anchors.verticalCenter: parent.verticalCenter
        text: bubble.text
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: 15
        font.weight: Font.Medium
    }
}
