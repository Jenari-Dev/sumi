import QtQuick
import QtQuick.Shapes

// 4 px gradient track with an angular thumb and a right-aligned value label.
//
// `value` is driven from outside. User input emits `moved(v)` for every change
// while dragging and `released(v)` once when the thumb is let go; setting
// `value` from code never emits either.
Item {
    id: root

    property real from: 0
    property real to: 100
    property real value: 0
    property real stepSize: 1
    property bool showValue: true
    property string valueText: ""
    // Optional tint of the track (R/G/B sliders): [start, end].
    property var tint: null

    property bool dragging: false
    property real dragValue: 0
    readonly property real shown: dragging ? dragValue : value
    readonly property bool hovered: hover.hovered
    // Room for the value: 56 px, more when a (translated) word needs it at the smallest size.
    // It only ever grows, so the thumb does not jump while dragging through changing words.
    property real neededWidth: 0
    readonly property real labelWidth: showValue ? Math.max(56, neededWidth + 12) : 0
    readonly property real trackWidth: Math.max(1, width - labelWidth)
    readonly property real fraction: to > from ? Math.max(0, Math.min(1, (shown - from) / (to - from))) : 0
    readonly property real thumbX: fraction * trackWidth

    signal moved(real value)
    signal released(real value)

    implicitWidth: 300
    implicitHeight: 24
    activeFocusOnTab: true

    function valueAt(x) {
        const t = Math.max(0, Math.min(1, x / trackWidth));
        const raw = from + t * (to - from);
        return Math.max(from, Math.min(to, Math.round(raw / stepSize) * stepSize));
    }

    // Filled part (gradient), then the grey remainder over it.
    Rectangle {
        id: track
        y: (parent.height - height) / 2
        width: root.trackWidth
        height: 4
        color: Theme.disabledBorder
        gradient: root.enabled ? fillGradient : null

        Gradient {
            id: fillGradient
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: root.tint ? root.tint[0] : Theme.trackStart }
            GradientStop { position: root.tint ? 0.0 : 0.28; color: root.tint ? root.tint[0] : Theme.trackStart }
            GradientStop { position: root.tint ? 1.0 : 0.51; color: root.tint ? root.tint[1] : Theme.trackMid }
            GradientStop { position: root.tint ? 1.0 : 0.73; color: root.tint ? root.tint[1] : Theme.trackEnd }
            GradientStop { position: 1.0; color: root.tint ? root.tint[1] : Theme.trackEnd }
        }
    }
    Rectangle {
        x: root.thumbX
        y: track.y
        width: root.trackWidth - root.thumbX
        height: 4
        color: root.enabled ? Theme.trackRest : Theme.alpha(Theme.disabledGlyph, 0.18)
    }

    // Angular thumb
    Shape {
        id: thumb
        x: root.thumbX - 6
        y: (parent.height - 18) / 2
        width: 12
        height: 18
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: root.enabled ? Theme.black : Theme.disabledBorder
            strokeWidth: 1
            fillColor: !root.enabled ? Theme.disabledText
                : (root.dragging ? Theme.pressedBright : (root.hovered || root.activeFocus ? Theme.hoverEdge : Theme.text))
            joinStyle: ShapePath.MiterJoin
            PathSvg { path: "M0.5,4 L6,0.5 L11.5,4 L11.5,13.5 L6,17.5 L0.5,13.5 Z" }
        }
    }

    TextMetrics {
        id: valueMetrics
        text: root.valueText !== "" ? root.valueText : String(Math.round(root.shown))
        font.family: Theme.font
        font.pixelSize: 10
        onAdvanceWidthChanged: root.neededWidth = Math.max(root.neededWidth, Math.ceil(advanceWidth))
    }

    Text {
        visible: root.showValue
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: root.labelWidth - 12
        horizontalAlignment: Text.AlignRight
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: 10
        elide: Text.ElideRight
        text: root.valueText !== "" ? root.valueText : String(Math.round(root.shown))
        color: root.enabled ? Theme.text : Theme.disabledText
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    MouseArea {
        id: area
        x: -8
        width: root.trackWidth + 16
        height: parent.height
        enabled: root.enabled
        preventStealing: true
        function at(mx) {
            return root.valueAt(mx - 8);
        }
        onPressed: mouse => {
            root.forceActiveFocus();
            root.dragging = true;
            root.dragValue = at(mouse.x);
            root.moved(root.dragValue);
        }
        onPositionChanged: mouse => {
            if (!pressed) {
                return;
            }
            const v = at(mouse.x);
            if (v !== root.dragValue) {
                root.dragValue = v;
                root.moved(v);
            }
        }
        onCanceled: root.dragging = false
        onReleased: {
            // Commit first so the owner's value is final before the drag override ends.
            root.released(root.dragValue);
            root.dragging = false;
        }
    }

    Keys.onPressed: event => {
        let v = root.shown;
        const big = Math.max(root.stepSize, (root.to - root.from) / 10);
        switch (event.key) {
        case Qt.Key_Left:
        case Qt.Key_Down:
            v -= root.stepSize;
            break;
        case Qt.Key_Right:
        case Qt.Key_Up:
            v += root.stepSize;
            break;
        case Qt.Key_PageDown:
            v -= big;
            break;
        case Qt.Key_PageUp:
            v += big;
            break;
        case Qt.Key_Home:
            v = root.from;
            break;
        case Qt.Key_End:
            v = root.to;
            break;
        default:
            return;
        }
        event.accepted = true;
        v = Math.max(root.from, Math.min(root.to, v));
        if (v !== root.shown) {
            root.moved(v);
            root.released(v);
        }
    }
}
