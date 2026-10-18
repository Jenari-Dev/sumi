import QtQuick

// The AI bubble of one monitor: two small windows on that monitor's screen.
//
//  - the orb window (96 x 96) takes the mouse: drag it to move it, double-click to hide it
//    until the next switch;
//  - the message window sits to the left of the orb and is transparent for input, so it can
//    never get in the way of the game underneath.
//
// Both are made from `ctl.shell` (a layer-shell overlay surface, or a plain stay-on-top
// window). Their margins are pixels from the right / bottom edge of the screen.
Item {
    id: ov

    required property var ctl            // AiBubbleModel
    required property string targetId

    readonly property int orbSize: 96
    readonly property int gap: 8

    readonly property var st: ctl.stateOf(targetId)
    readonly property string connector: st ? st.connector : ""
    readonly property bool paused: st ? st.paused !== "" : false
    readonly property int switchSeq: st ? st.switchSeq : 0
    readonly property int analyseSeq: st ? st.analyseSeq : 0
    readonly property var screenObject: ctl.screenFor(connector)
    readonly property bool wanted: ctl.shown(targetId) && ctl.shell !== null && screenObject !== null

    // Where the orb is: pixels from the screen's right and bottom edge (live while dragging).
    property int curRight: 32
    property int curBottom: 96
    readonly property Item orbItem: orbWin.item ? orbWin.item.inner : null
    readonly property Item messageItem: msgWin.item ? msgWin.item.inner : null
    readonly property int messageWidth: messageItem ? Math.ceil(messageItem.implicitWidth) : 0
    readonly property int messageHeight: messageItem ? Math.ceil(messageItem.implicitHeight) : 44
    readonly property int screenWidth: screenObject ? screenObject.width : 100000
    // The message goes left of the orb; when the orb is too near the left edge, to its right.
    readonly property bool messageLeft: curRight + orbSize + gap + messageWidth <= screenWidth
    readonly property int messageRight: messageLeft ? curRight + orbSize + gap : Math.max(0, curRight - gap - messageWidth)
    readonly property int messageBottom: curBottom + Math.round((orbSize - messageHeight) / 2)

    visible: false

    function syncPosition() {
        const p = ctl.position(connector);
        curRight = p.right;
        curBottom = p.bottom;
    }

    function moveBy(dx, dy) {
        const s = screenObject;
        const maxRight = s ? s.width - orbSize : 100000;
        const maxBottom = s ? s.height - orbSize : 100000;
        curRight = Math.max(0, Math.min(maxRight, Math.round(curRight - dx)));
        curBottom = Math.max(0, Math.min(maxBottom, Math.round(curBottom - dy)));
    }

    function savePosition() {
        ctl.setPosition(connector, curRight, curBottom);
    }

    // The mode was switched: red pulse, and the message unless the monitor is paused.
    function announce(mode) {
        if (orbItem) {
            orbItem.pulse();
        }
        if (messageItem && !paused) {
            messageItem.flash(ctl.message(mode));
        }
    }

    Component.onCompleted: syncPosition()
    onConnectorChanged: syncPosition()
    onAnalyseSeqChanged: if (orbItem) orbItem.analyse()
    onWantedChanged: {
        if (wanted) {
            syncPosition();
        } else if (ctl.shown(targetId) && ctl.shell !== null && screenObject === null && connector !== "") {
            ctl.noteMissingScreen(connector);
        }
    }

    Connections {
        target: ov.ctl
        function onSwitched(id, mode) {
            if (id === ov.targetId) {
                ov.announce(mode);
            }
        }
        function onPositionsChanged() {
            ov.syncPosition();
        }
    }

    Component {
        id: orbContent
        AiOrb {
            paused: ov.paused
            AiDragArea {
                objectName: "orb_drag"
                anchors.fill: parent
                onMoved: (dx, dy) => ov.moveBy(dx, dy)
                onDragFinished: ov.savePosition()
                onHideRequested: ov.ctl.dismiss(ov.targetId)
            }
        }
    }
    Component {
        id: messageContent
        AiBubbleText {}
    }

    Loader {
        id: orbWin
        objectName: "orb_window"
        active: ov.wanted
        sourceComponent: ov.ctl.shell
        onLoaded: {
            // Everything but the content first: the window shows itself once it has content.
            item.targetScreen = Qt.binding(() => ov.screenObject);
            item.marginRight = Qt.binding(() => ov.curRight);
            item.marginBottom = Qt.binding(() => ov.curBottom);
            item.content = orbContent;
        }
    }
    Loader {
        id: msgWin
        objectName: "message_window"
        active: ov.wanted
        sourceComponent: ov.ctl.shell
        onLoaded: {
            item.targetScreen = Qt.binding(() => ov.screenObject);
            item.marginRight = Qt.binding(() => ov.messageRight);
            item.marginBottom = Qt.binding(() => ov.messageBottom);
            item.clickThrough = true;
            const w = item;   // `item` is null once the Loader lets go of the window
            w.wanted = Qt.binding(() => w.inner !== null && (w.inner.shown || w.inner.opacity > 0.01));
            w.content = messageContent;
        }
    }
}
