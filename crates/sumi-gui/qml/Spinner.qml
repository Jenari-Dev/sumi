import QtQuick
import QtQuick.Shapes

// Red ring that fades out along its tail, turning once every 1.5 s.
Item {
    id: root

    property bool running: visible

    width: 120
    height: 120

    Shape {
        id: ring
        anchors.fill: parent
        antialiasing: true
        layer.enabled: true
        layer.samples: 4

        ShapePath {
            strokeColor: "transparent"
            strokeWidth: 0
            fillRule: ShapePath.OddEvenFill
            fillGradient: ConicalGradient {
                centerX: 60
                centerY: 60
                angle: 90
                GradientStop { position: 0.0; color: "#FFFF1929" }
                GradientStop { position: 0.8; color: "#00FF1929" }
                GradientStop { position: 1.0; color: "#00FF1929" }
            }
            PathAngleArc { centerX: 60; centerY: 60; radiusX: 56; radiusY: 56; startAngle: 0; sweepAngle: 360 }
            PathMove { x: 108; y: 60 }
            PathAngleArc { centerX: 60; centerY: 60; radiusX: 48; radiusY: 48; startAngle: 0; sweepAngle: 360 }
        }

        RotationAnimator on rotation {
            from: 0
            to: 360
            duration: 1500
            loops: Animation.Infinite
            running: root.running && root.visible
        }
    }
}
