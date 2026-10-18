pragma Singleton

import QtQuick

// Palette and metrics of Sumi's dark gaming theme. The colour values follow the
// look described in notes/app-shell-and-features.md §1.9; every shape, icon and
// background is Sumi's own drawing.
QtObject {
    // Surfaces and text
    readonly property color black: "#000000"
    readonly property color text: "#FFFFFF"
    readonly property color textDim: "#A6A6A6"
    readonly property color textSoft: "#DCDCDC"
    readonly property color separator: "#66A7ABAC"
    readonly property color dialog: "#181818"
    readonly property color dim: "#B0000000"

    // Disabled
    readonly property color disabledFill: "#171717"
    readonly property color disabledBorder: "#262626"
    readonly property color disabledText: "#5E5E5E"
    readonly property color disabledGlyph: "#838383"

    // Borders
    readonly property color borderTile: "#5E5E5E"
    readonly property color borderButton: "#5B5B5B"
    readonly property color borderCombo: "#B2B2B2"
    readonly property color borderWindow: "#666666"

    // Accents
    readonly property color hoverFill: "#008099"
    readonly property color hoverEdge: "#00D5FF"
    readonly property color pressed: "#990030"
    readonly property color pressedBright: "#FF0050"
    readonly property color red: "#FF1929"
    readonly property color error: "#FF3939"
    readonly property color warn: "#FFB020"
    readonly property color ok: "#3DDC84"

    // Slider
    readonly property color trackStart: "#0E7287"
    readonly property color trackMid: "#5600E5"
    readonly property color trackEnd: "#940038"
    readonly property color trackRest: "#989898"

    // Fonts: Inter when installed, otherwise Noto Sans.
    readonly property string font: Qt.fontFamilies().indexOf("Inter") >= 0 ? "Inter" : "Noto Sans"
    readonly property int fontBody: 14
    readonly property int fontLabel: 16
    readonly property int fontTitle: 24
    readonly property int fontSmall: 12

    // Metrics (design size 1280 x 818)
    readonly property int designWidth: 1280
    readonly property int designHeight: 818
    readonly property int titleHeight: 32
    readonly property int sidebarWidth: 240
    readonly property int pageWidth: 1040
    readonly property int pageHeight: 786
    readonly property int pageMargin: 36
    readonly property int contentWidth: 968

    function alpha(c, a) {
        return Qt.rgba(c.r, c.g, c.b, a);
    }
}
