import QtQuick
import QtQuick.Window

// A small borderless window for the AI bubble, anchored to the bottom-right corner of one
// screen. This is the plain version (X11, or Wayland without layer-shell): a frameless
// stay-on-top tool window placed by x / y. AiLayerWindow.qml derives from it and turns it
// into a KDE layer-shell overlay surface.
//
// Both take the same properties; the content comes as a Component so the window itself
// knows nothing about the orb or the message.
Window {
    id: win

    property var targetScreen: null     // the QScreen (its name is the connector, DP-3 ...)
    property int marginRight: 32        // pixels between the window and the screen's right edge
    property int marginBottom: 96       // ... and its bottom edge
    property bool clickThrough: false   // pointer events pass through to what is below
    property bool wanted: true          // false hides the window without destroying it
    property Component content: null
    // Layer-shell windows are placed by margins, not by x / y.
    property bool placeByPosition: true
    // Qt.Tool keeps the plain fallback out of the task bar; a layer surface has no task bar entry.
    property int extraFlags: Qt.Tool

    readonly property Item inner: body.item
    color: "transparent"
    title: "Sumi AI"
    transientParent: null
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus | extraFlags
        | (clickThrough ? Qt.WindowTransparentForInput : 0)
    screen: targetScreen !== null ? targetScreen : Qt.application.screens[0]
    visible: wanted && targetScreen !== null && inner !== null
    width: inner ? Math.max(1, Math.ceil(inner.implicitWidth)) : 1
    height: inner ? Math.max(1, Math.ceil(inner.implicitHeight)) : 1
    x: placeByPosition && targetScreen ? targetScreen.virtualX + targetScreen.width - width - marginRight : 0
    y: placeByPosition && targetScreen ? targetScreen.virtualY + targetScreen.height - height - marginBottom : 0

    Loader {
        id: body
        anchors.fill: parent
        sourceComponent: win.content
    }
}
