import QtQuick

// An application icon from the icon theme (`image://icon/<name>`), with our own
// window glyph behind it for apps whose icon cannot be found. The glyph is drawn
// with plain rectangles: unlike a Shape it is clipped properly inside the lists.
Item {
    id: root

    property string name: ""
    property real size: 20
    property bool dim: false

    width: size
    height: size
    opacity: dim ? 0.45 : 1

    Item {
        visible: root.name === "" || image.status !== Image.Ready || image.implicitWidth <= 1
        anchors.fill: parent
        readonly property real t: Math.max(1, root.size / 15)

        Rectangle {
            x: parent.width * 0.1
            y: parent.height * 0.18
            width: parent.width * 0.8
            height: parent.height * 0.64
            color: "transparent"
            border.width: parent.t
            border.color: Theme.textDim
        }
        Rectangle {
            x: parent.width * 0.1
            y: parent.height * 0.18 + parent.height * 0.18
            width: parent.width * 0.8
            height: parent.t
            color: Theme.textDim
        }
        Rectangle {
            x: parent.width * 0.2
            y: parent.height * 0.18 + parent.height * 0.07
            width: parent.t * 1.5
            height: parent.t
            color: Theme.textDim
        }
        Rectangle {
            x: parent.width * 0.2 + parent.t * 3
            y: parent.height * 0.18 + parent.height * 0.07
            width: parent.t * 1.5
            height: parent.t
            color: Theme.textDim
        }
    }
    Image {
        id: image
        anchors.fill: parent
        visible: root.name !== ""
        source: root.name !== "" ? "image://icon/" + root.name : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        cache: true
    }
}
