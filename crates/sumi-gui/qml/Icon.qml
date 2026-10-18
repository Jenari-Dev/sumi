import QtQuick
import QtQuick.Shapes

// One of Sumi's line icons (see IconData). `size` is the rendered edge in px,
// `lineWidth` the stroke in px, independent of the size.
Item {
    id: root

    property string name: ""
    property color color: Theme.text
    property color fill: "transparent"
    property real size: 24
    property real lineWidth: 1.5

    width: size
    height: size

    readonly property real k: size / 48

    Shape {
        width: 48
        height: 48
        scale: root.k
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: root.color
            strokeWidth: root.lineWidth / root.k
            fillColor: root.fill
            joinStyle: ShapePath.MiterJoin
            capStyle: ShapePath.FlatCap
            PathSvg {
                path: IconData.d(root.name)
            }
        }
    }
}
