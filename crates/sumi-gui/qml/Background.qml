import QtQuick
import QtQuick.Shapes

// Window backdrop drawn from gradients only: a near-black field, a faint teal
// light column along the sidebar edge with a few diagonal streaks at the
// bottom left, and a soft red glow with thin streaks in the bottom right.
Item {
    id: root

    width: Theme.designWidth
    height: Theme.designHeight

    Rectangle {
        anchors.fill: parent
        color: "#030405"
    }

    // Teal column at the sidebar edge
    Rectangle {
        x: Theme.sidebarWidth - 120
        width: 240
        height: parent.height
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: "#00203A40" }
            GradientStop { position: 0.5; color: "#38243C42" }
            GradientStop { position: 1.0; color: "#00203A40" }
        }
    }
    // Fade the column out toward the top and bottom
    Rectangle {
        x: Theme.sidebarWidth - 120
        width: 240
        height: 220
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#FF030405" }
            GradientStop { position: 1.0; color: "#00030405" }
        }
    }
    Rectangle {
        x: Theme.sidebarWidth - 120
        y: parent.height - 160
        width: 240
        height: 160
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#00030405" }
            GradientStop { position: 1.0; color: "#CC030405" }
        }
    }
    // A thin brighter line in the middle of the column
    Rectangle {
        x: Theme.sidebarWidth
        width: 1
        height: parent.height
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#00356B73" }
            GradientStop { position: 0.4; color: "#40356B73" }
            GradientStop { position: 0.85; color: "#25356B73" }
            GradientStop { position: 1.0; color: "#00356B73" }
        }
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.GeometryRenderer

        // Teal streaks, bottom left
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#162FA8B8"
            startX: 0; startY: 818
            PathLine { x: 190; y: 818 }
            PathLine { x: 300; y: 560 }
            PathLine { x: 292; y: 560 }
            PathLine { x: 0; y: 818 }
        }
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#0E2FA8B8"
            startX: 40; startY: 818
            PathLine { x: 330; y: 818 }
            PathLine { x: 372; y: 706 }
            PathLine { x: 364; y: 706 }
        }
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#0B2FA8B8"
            startX: 0; startY: 700
            PathLine { x: 120; y: 818 }
            PathLine { x: 150; y: 818 }
            PathLine { x: 0; y: 676 }
        }

        // Red glow, bottom right
        ShapePath {
            strokeColor: "transparent"
            fillGradient: RadialGradient {
                centerX: 1280; centerY: 840
                centerRadius: 620
                focalX: 1280; focalY: 840
                GradientStop { position: 0.0; color: "#4AFF1929" }
                GradientStop { position: 0.35; color: "#22FF1929" }
                GradientStop { position: 0.7; color: "#0AFF1929" }
                GradientStop { position: 1.0; color: "#00FF1929" }
            }
            startX: 560; startY: 218
            PathLine { x: 1280; y: 218 }
            PathLine { x: 1280; y: 818 }
            PathLine { x: 560; y: 818 }
            PathLine { x: 560; y: 218 }
        }
        // Thin red streaks
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#30FF1929"
            startX: 1280; startY: 560
            PathLine { x: 1280; y: 566 }
            PathLine { x: 1010; y: 818 }
            PathLine { x: 984; y: 818 }
        }
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#22FF1929"
            startX: 1280; startY: 640
            PathLine { x: 1280; y: 644 }
            PathLine { x: 1130; y: 818 }
            PathLine { x: 1112; y: 818 }
        }
        ShapePath {
            strokeColor: "transparent"
            fillColor: "#18FF1929"
            startX: 1170; startY: 818
            PathLine { x: 1280; y: 700 }
            PathLine { x: 1280; y: 712 }
            PathLine { x: 1196; y: 818 }
        }
    }
}
