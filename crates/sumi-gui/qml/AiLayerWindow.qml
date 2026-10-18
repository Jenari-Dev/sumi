import QtQuick
import QtQuick.Window
import org.kde.layershell 1.0 as LayerShell

// AiWindow as a KDE layer-shell surface: on the overlay layer (above full-screen games),
// anchored bottom + right with margins, no keyboard focus, no exclusive zone.
// The margins are the window's position; changing them moves it.
//
// Loaded on demand (AiBubbleModel.probeShell): when the module or the protocol is missing
// this file fails to load and the overlay uses AiWindow.qml.
AiWindow {
    id: win

    placeByPosition: false
    extraFlags: 0

    LayerShell.Window.scope: "sumi-ai"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorRight
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.exclusionZone: 0
    LayerShell.Window.margins.right: win.marginRight
    LayerShell.Window.margins.bottom: win.marginBottom
}
