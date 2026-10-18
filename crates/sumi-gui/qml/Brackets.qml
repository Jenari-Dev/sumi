pragma ComponentBehavior: Bound

import QtQuick

// Four angular corner brackets (the frame accent of Sumi's buttons, combos and
// dialogs), drawn as thin bars so they stay crisp and obey clipping.
// `corners` selects which ones to draw: "all" or "bottom".
Item {
    id: root

    property color color: Theme.text
    property real size: 8
    property real thickness: 1.5
    property string corners: "all"

    readonly property bool withTop: corners === "all"

    component Bar: Rectangle {
        color: root.color
    }

    // top left
    Bar { visible: root.withTop; x: 0; y: 0; width: root.size; height: root.thickness }
    Bar { visible: root.withTop; x: 0; y: 0; width: root.thickness; height: root.size }
    // top right
    Bar { visible: root.withTop; x: root.width - root.size; y: 0; width: root.size; height: root.thickness }
    Bar { visible: root.withTop; x: root.width - root.thickness; y: 0; width: root.thickness; height: root.size }
    // bottom left
    Bar { x: 0; y: root.height - root.thickness; width: root.size; height: root.thickness }
    Bar { x: 0; y: root.height - root.size; width: root.thickness; height: root.size }
    // bottom right
    Bar { x: root.width - root.size; y: root.height - root.thickness; width: root.size; height: root.thickness }
    Bar { x: root.width - root.thickness; y: root.height - root.size; width: root.thickness; height: root.size }
}
