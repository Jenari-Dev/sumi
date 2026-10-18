import QtQuick
import QtQuick.Shapes

// The AI Visual orb: Sumi's ink drop on a dark disc inside a thin teal arc ring.
// Pure drawing (no layer-shell, no daemon), so it can be tested and photographed
// offscreen. 96 x 96 logical px; the disc is 80 px and the rest is room for the glow.
//
//   idle        slow breathing, ring turns slowly
//   analyse()   the ring spins faster for a moment (the picture changed)
//   pulse()     a red glow pulse (AI Visual switched the mode)
//   paused      dimmed to 40 %, nothing moves
//
// The animation is stepped by one 30 Hz timer instead of the display's refresh rate:
// this window sits on a gaming screen, so it asks the compositor for 30 small
// repaints a second at most, and none at all while paused or `animated` is off.
Item {
    id: orb

    property bool paused: false
    property bool animated: true
    property real ringAngle: 0          // degrees; the timer advances it
    property real breath: 0             // 0..1, follows a slow sine
    property real boost: 0              // 0..1, extra spin speed after analyse()
    property real glow: 0               // 0..1, red glow strength
    readonly property bool busy: boost > 0.02
    readonly property bool glowing: glow > 0.02

    readonly property real idleSpeed: 40       // degrees per second
    readonly property real boostSpeed: 540     // extra degrees per second at full boost
    readonly property real breathSeconds: 6.4  // one full breath

    implicitWidth: 96
    implicitHeight: 96
    width: implicitWidth
    height: implicitHeight
    opacity: paused ? 0.4 : 1
    Behavior on opacity {
        NumberAnimation { duration: 250 }
    }

    // A short faster spin.
    function analyse() {
        if (paused) {
            return;
        }
        spinDown.stop();
        boost = 1;
        spinDown.start();
    }

    // The mode was switched: two red beats.
    function pulse() {
        if (paused) {
            return;
        }
        beat.stop();
        beat.start();
    }

    SequentialAnimation {
        id: spinDown
        PauseAnimation { duration: 250 }
        NumberAnimation { target: orb; property: "boost"; to: 0; duration: 750; easing.type: Easing.OutCubic }
    }
    SequentialAnimation {
        id: beat
        NumberAnimation { target: orb; property: "glow"; to: 1; duration: 150; easing.type: Easing.OutQuad }
        NumberAnimation { target: orb; property: "glow"; to: 0.3; duration: 330; easing.type: Easing.InOutQuad }
        NumberAnimation { target: orb; property: "glow"; to: 1; duration: 150; easing.type: Easing.OutQuad }
        NumberAnimation { target: orb; property: "glow"; to: 0; duration: 700; easing.type: Easing.InOutQuad }
    }

    // One clock for the ring and the breath.
    Timer {
        id: clock
        property double last: 0
        interval: 33
        repeat: true
        running: orb.animated && !orb.paused && orb.visible
        onRunningChanged: last = Date.now()
        onTriggered: {
            const now = Date.now();
            const dt = Math.min(0.25, (now - last) / 1000);
            last = now;
            orb.ringAngle = (orb.ringAngle + dt * (orb.idleSpeed + orb.boost * orb.boostSpeed)) % 360;
            orb.breath = 0.5 - 0.5 * Math.cos(2 * Math.PI * (now % (orb.breathSeconds * 1000)) / (orb.breathSeconds * 1000));
        }
    }

    Item {
        id: body
        anchors.fill: parent
        scale: 1 + 0.03 * orb.breath
        transformOrigin: Item.Center

        // Red glow: a halo hugging the disc, strongest at the rim.
        Shape {
            anchors.fill: parent
            opacity: orb.glow
            scale: 0.9 + 0.1 * orb.glow
            transformOrigin: Item.Center
            preferredRendererType: Shape.CurveRenderer
            visible: orb.glowing
            ShapePath {
                strokeColor: "transparent"
                fillGradient: RadialGradient {
                    centerX: 48
                    centerY: 48
                    centerRadius: 48
                    focalX: 48
                    focalY: 48
                    GradientStop { position: 0.0; color: "#00FF1929" }
                    GradientStop { position: 0.72; color: "#00FF1929" }
                    GradientStop { position: 0.84; color: "#C8FF1929" }
                    GradientStop { position: 1.0; color: "#00FF1929" }
                }
                PathAngleArc { centerX: 48; centerY: 48; radiusX: 48; radiusY: 48; startAngle: 0; sweepAngle: 360 }
            }
        }

        // The dark disc.
        Rectangle {
            anchors.centerIn: parent
            width: 80
            height: 80
            radius: 40
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#1A1A1F" }
                GradientStop { position: 1.0; color: "#09090B" }
            }
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.10)
        }

        // Faint full ring, so the orb still reads as a ring where the arc has faded.
        Rectangle {
            anchors.centerIn: parent
            width: 88
            height: 88
            radius: 44
            color: "transparent"
            border.width: 1
            border.color: Theme.alpha(Theme.hoverFill, 0.35)
        }

        // The turning teal arc (head #00D5FF fading through #008099 into the dark).
        Shape {
            id: ring
            anchors.fill: parent
            rotation: orb.ringAngle
            transformOrigin: Item.Center
            opacity: 0.8 + 0.2 * orb.breath
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: "transparent"
                fillRule: ShapePath.OddEvenFill
                fillGradient: ConicalGradient {
                    centerX: 48
                    centerY: 48
                    angle: 90
                    GradientStop { position: 0.0; color: "#FF00D5FF" }
                    GradientStop { position: 0.30; color: "#FF008099" }
                    GradientStop { position: 0.62; color: "#00008099" }
                    GradientStop { position: 1.0; color: "#00008099" }
                }
                PathAngleArc { centerX: 48; centerY: 48; radiusX: 45; radiusY: 45; startAngle: 0; sweepAngle: 360 }
                PathMove { x: 90; y: 48 }
                PathAngleArc { centerX: 48; centerY: 48; radiusX: 42; radiusY: 42; startAngle: 0; sweepAngle: 360 }
            }
        }

        // Sumi's ink drop (the shape of the app icon's, 64-unit grid, centred on the disc).
        Shape {
            id: drop
            width: 64
            height: 64
            x: 48 - 32 * scale
            y: 48 - 30 * scale
            scale: 1.7 + 0.04 * orb.breath
            transformOrigin: Item.TopLeft
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0
                    y1: 18
                    x2: 0
                    y2: 41
                    GradientStop { position: 0.0; color: orb.glowing ? Qt.lighter(Theme.red, 1 + 0.3 * orb.glow) : "#E8394C" }
                    GradientStop { position: 1.0; color: orb.glowing ? Theme.red : "#D7263D" }
                }
                PathSvg { path: "M32 18 C27 25 24 29 24 33 a8 8 0 0 0 16 0 c0-4-3-8-8-15z" }
            }
        }
    }
}
