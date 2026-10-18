pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2

// Drop-down list in the black / corner-bracket style.
//
// `model` is an array of strings or {value, label} objects. An entry may also
// carry `icon` (a theme icon name, drawn at the left) or `header: true` (a
// section title that cannot be picked). `activated(i)` fires only when the user
// picks an entry. The list is attached to the top-level overlay so it scales
// with the rest of the window.
Item {
    id: root

    property var model: []
    property int currentIndex: -1
    property string placeholder: "—"
    // Shown instead of the placeholder when nothing in the model is current
    // (for example an app the list does not know).
    property string displayText: ""
    property string displayIcon: ""
    // Shown instead of the current entry's label when set (an app row shows the name the user picked).
    property string forceText: ""
    // Draw an icon slot (with the fallback glyph) in front of the text even when the entry has none.
    property bool iconsAlways: false
    readonly property bool opened: popup.visible
    readonly property bool hovered: hover.hovered
    readonly property bool hasCurrent: currentIndex >= 0 && currentIndex < model.length
    readonly property string currentText: forceText !== "" ? forceText : hasCurrent ? labelAt(currentIndex) : (displayText !== "" ? displayText : placeholder)
    readonly property string currentIcon: hasCurrent ? iconAt(currentIndex) : displayIcon
    signal activated(int index)

    implicitWidth: 296
    implicitHeight: 32
    activeFocusOnTab: true

    function labelAt(i) {
        const m = model[i];
        return m === undefined ? "" : (typeof m === "string" ? m : m.label);
    }

    function iconAt(i) {
        const m = model[i];
        return m === undefined || typeof m === "string" || !m.icon ? "" : m.icon;
    }

    function isHeader(i) {
        const m = model[i];
        return m !== undefined && typeof m !== "string" && m.header === true;
    }

    function open() {
        if (!enabled || model.length === 0) {
            return;
        }
        popup.show();
    }

    function close() {
        popup.visible = false;
        root.forceActiveFocus();
    }

    function choose(i) {
        if (isHeader(i)) {
            return;
        }
        close();
        if (i >= 0 && i < model.length) {
            activated(i);
        }
    }

    onEnabledChanged: if (!enabled) popup.visible = false
    onVisibleChanged: if (!visible) popup.visible = false

    Rectangle {
        anchors.fill: parent
        color: !root.enabled ? Theme.disabledFill : (root.hovered || root.opened ? Theme.alpha(Theme.hoverFill, 0.22) : Theme.black)
        border.width: 1
        border.color: !root.enabled ? Theme.disabledBorder : (root.hovered || root.opened || root.activeFocus ? Theme.hoverEdge : Theme.borderCombo)
    }
    Brackets {
        anchors.fill: parent
        size: 5
        thickness: 1
        color: !root.enabled ? Theme.disabledText : (root.hovered || root.opened ? Theme.hoverEdge : Theme.text)
    }
    AppIcon {
        id: shownIcon
        visible: root.iconsAlways || root.currentIcon !== ""
        x: 10
        anchors.verticalCenter: parent.verticalCenter
        size: 20
        name: root.currentIcon
        dim: !root.enabled
    }
    Text {
        anchors.left: shownIcon.visible ? shownIcon.right : parent.left
        anchors.leftMargin: shownIcon.visible ? 8 : 12
        anchors.right: chevron.left
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        text: root.currentText
        elide: Text.ElideRight
        color: !root.enabled ? Theme.disabledText : (!root.hasCurrent && root.displayText === "" && root.forceText === "" ? Theme.textDim : Theme.text)
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Icon {
        id: chevron
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        size: 18
        lineWidth: 1.5
        name: "chevron"
        rotation: root.opened ? 180 : 0
        color: !root.enabled ? Theme.disabledText : (root.hovered || root.opened ? Theme.hoverEdge : Theme.text)
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        onTapped: root.opened ? root.close() : root.open()
    }
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Down) {
            event.accepted = true;
            root.open();
        }
    }

    // ---- The list
    Item {
        id: popup

        property real listX: 0
        property real listY: 0
        readonly property real listHeight: Math.min(root.model.length, 8) * 32 + 2

        parent: Overlay.layer ? Overlay.layer : root.Window.contentItem
        visible: false
        z: 10000
        x: 0
        y: 0
        width: parent ? parent.width : 0
        height: parent ? parent.height : 0

        function show() {
            const p = root.mapToItem(popup, 0, root.height);
            listX = p.x;
            listY = p.y + listHeight > height - 8 ? Math.max(8, p.y - root.height - listHeight) : p.y;
            visible = true;
            view.currentIndex = root.currentIndex;
            view.positionViewAtIndex(Math.max(0, root.currentIndex), ListView.Contain);
            view.forceActiveFocus();
        }

        MouseArea {
            anchors.fill: parent
            onClicked: root.close()
            onWheel: wheel => wheel.accepted = true
        }

        Rectangle {
            x: popup.listX
            y: popup.listY
            width: root.width
            height: popup.listHeight
            color: "#060606"
            border.width: 1
            border.color: Theme.borderCombo

            ListView {
                id: view
                anchors.fill: parent
                anchors.margins: 1
                clip: true
                model: root.model.length
                boundsBehavior: Flickable.StopAtBounds
                QQC2.ScrollBar.vertical: RogScrollBar {}
                Keys.onEscapePressed: root.close()
                Keys.onReturnPressed: root.choose(view.currentIndex)
                Keys.onEnterPressed: root.choose(view.currentIndex)
                Keys.onSpacePressed: root.choose(view.currentIndex)

                delegate: Rectangle {
                    id: row
                    required property int index
                    readonly property bool current: index === root.currentIndex
                    readonly property bool header: root.isHeader(index)
                    readonly property string icon: root.iconAt(index)
                    width: view.width
                    height: header ? 28 : 32
                    color: !header && (rowHover.hovered || ListView.isCurrentItem) ? Theme.alpha(Theme.hoverFill, 0.35)
                        : (current ? Theme.alpha(Theme.pressed, 0.3) : "transparent")

                    Rectangle {
                        visible: row.current
                        width: 2
                        height: parent.height
                        color: Theme.red
                    }
                    AppIcon {
                        id: rowIcon
                        visible: root.iconsAlways || row.icon !== ""
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        size: 20
                        name: row.icon
                    }
                    Text {
                        anchors.left: rowIcon.visible ? rowIcon.right : parent.left
                        anchors.leftMargin: rowIcon.visible ? 8 : 12
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.labelAt(row.index)
                        elide: Text.ElideRight
                        color: row.header ? Theme.textDim : Theme.text
                        font.family: Theme.font
                        font.pixelSize: row.header ? Theme.fontSmall : Theme.fontBody
                        font.capitalization: row.header ? Font.AllUppercase : Font.MixedCase
                        font.letterSpacing: row.header ? 1 : 0
                    }
                    Rectangle {
                        visible: row.header
                        anchors.bottom: parent.bottom
                        x: 12
                        width: parent.width - 24
                        height: 1
                        color: Theme.separator
                        opacity: 0.4
                    }
                    HoverHandler {
                        id: rowHover
                        enabled: !row.header
                        cursorShape: Qt.PointingHandCursor
                    }
                    TapHandler {
                        enabled: !row.header
                        onTapped: root.choose(row.index)
                    }
                }
            }
        }
    }
}
