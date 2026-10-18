import QtQuick

// A label, a percentage and a thin gradient bar (the slider track without a thumb).
Item {
    id: root

    property string label: ""
    property real value: 0          // 0..100
    property string valueText: ""   // overrides "NN%" (for "x of y MB")
    property bool dim: false

    implicitWidth: 300
    implicitHeight: 38

    readonly property real fraction: Math.max(0, Math.min(1, value / 100))

    Text {
        x: 0
        y: 0
        width: parent.width - Math.max(70, valueLabel.implicitWidth + 12)
        elide: Text.ElideRight
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: 10
        text: root.label
        color: root.dim ? Theme.disabledText : Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Text {
        id: valueLabel
        anchors.right: parent.right
        y: 0
        text: root.valueText !== "" ? root.valueText : Math.round(root.value) + "%"
        color: root.dim ? Theme.disabledText : Theme.textSoft
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Rectangle {
        id: track
        y: 26
        width: parent.width
        height: 4
        color: Theme.disabledBorder
        gradient: root.dim ? null : fill

        Gradient {
            id: fill
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: Theme.trackStart }
            GradientStop { position: 0.28; color: Theme.trackStart }
            GradientStop { position: 0.51; color: Theme.trackMid }
            GradientStop { position: 0.73; color: Theme.trackEnd }
            GradientStop { position: 1.0; color: Theme.trackEnd }
        }
    }
    Rectangle {
        x: root.fraction * track.width
        y: track.y
        width: track.width - x
        height: 4
        color: root.dim ? Theme.alpha(Theme.disabledGlyph, 0.18) : Theme.trackRest
    }
}
