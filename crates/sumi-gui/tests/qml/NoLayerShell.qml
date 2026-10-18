import QtQuick
import QtQuick.Window
import org.kde.layershell.thisdoesnotexist 1.0 as LayerShell

// Stands in for AiLayerWindow.qml on a machine without KDE's layer-shell module:
// the import cannot be resolved, so the component never becomes Ready.
Window {
    LayerShell.Window.scope: "sumi-ai"
}
