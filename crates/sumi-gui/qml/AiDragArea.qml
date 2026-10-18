import QtQuick

// Drag handle for a window that moves itself (the AI orb): reports how far to move the window,
// in pixels, as `moved(dx, dy)`.
//
// The pointer position arrives in the window's own coordinates. While a layer-shell window
// follows the pointer, that position stays near the press point, so (position - press point) is
// what is still missing and is what gets reported. The compositor needs a moment to apply a new
// margin; until then positions are stale and would be counted twice, so after every report the
// next `settleMs` of motion is ignored.
//
// A press that never moves farther than the drag threshold is a click, not a drag; a double
// click is reported as `hideRequested()`.
MouseArea {
    id: area

    property int threshold: 5
    property int settleMs: 24

    property bool moving: false
    property real pressX: 0
    property real pressY: 0
    property double blockedUntil: 0

    signal moved(real dx, real dy)
    signal dragFinished
    signal hideRequested

    acceptedButtons: Qt.LeftButton
    cursorShape: moving ? Qt.ClosedHandCursor : Qt.OpenHandCursor
    preventStealing: true

    onPressed: mouse => {
        pressX = mouse.x;
        pressY = mouse.y;
        moving = false;
        blockedUntil = 0;
    }
    onPositionChanged: mouse => {
        if (!pressed) {
            return;
        }
        const dx = mouse.x - pressX;
        const dy = mouse.y - pressY;
        if (!moving && Math.abs(dx) < threshold && Math.abs(dy) < threshold) {
            return;
        }
        moving = true;
        if (Date.now() < blockedUntil) {
            return;
        }
        blockedUntil = Date.now() + settleMs;
        moved(dx, dy);
    }
    onReleased: {
        if (moving) {
            moving = false;
            dragFinished();
        }
    }
    onDoubleClicked: hideRequested()
}
