import QtQuick

// Thumbnail of a PIP / PBP layout on a small monitor outline.
// mode: 0 off, 1 PIP top right, 2 top left, 3 bottom right, 4 bottom left,
// 5 PBP 50/50, 6 PBP wide left, 7 PBP wide right.
Item {
    id: root

    property int mode: 0
    property color accent: Theme.hoverEdge

    width: 64
    height: 44

    readonly property real w: width
    readonly property real h: height

    function rect(i) {
        // [x, y, w, h] of window i (0 = main, 1 = second) in the thumbnail, 3 px inset.
        const iw = w - 6, ih = h - 6;
        switch (mode) {
        case 1: return i === 0 ? [3, 3, iw, ih] : [w - 3 - iw * 0.38, 3, iw * 0.38, ih * 0.42];
        case 2: return i === 0 ? [3, 3, iw, ih] : [3, 3, iw * 0.38, ih * 0.42];
        case 3: return i === 0 ? [3, 3, iw, ih] : [w - 3 - iw * 0.38, h - 3 - ih * 0.42, iw * 0.38, ih * 0.42];
        case 4: return i === 0 ? [3, 3, iw, ih] : [3, h - 3 - ih * 0.42, iw * 0.38, ih * 0.42];
        case 5: return i === 0 ? [3, 3, iw / 2 - 1, ih] : [3 + iw / 2 + 1, 3, iw / 2 - 1, ih];
        case 6: return i === 0 ? [3, 3, iw * 0.64, ih] : [3 + iw * 0.64 + 2, 3, iw * 0.36 - 2, ih];
        case 7: return i === 0 ? [3, 3, iw * 0.36 - 2, ih] : [3 + iw * 0.36, 3, iw * 0.64, ih];
        default: return [3, 3, iw, ih];
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: 1.5
        border.color: Theme.text
    }
    Rectangle {
        readonly property var r: root.rect(0)
        x: r[0]
        y: r[1]
        width: r[2]
        height: r[3]
        color: root.mode === 0 ? "transparent" : Theme.alpha(Theme.text, 0.14)
        border.width: root.mode === 0 ? 0 : 1
        border.color: Theme.textDim
    }
    Rectangle {
        visible: root.mode > 0
        readonly property var r: root.rect(1)
        x: r[0]
        y: r[1]
        width: r[2]
        height: r[3]
        color: Theme.alpha(root.accent, 0.3)
        border.width: 1
        border.color: root.accent
    }
}
