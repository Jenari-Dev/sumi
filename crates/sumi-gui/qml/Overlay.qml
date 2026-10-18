pragma Singleton

import QtQuick

// The top layer of the scaled window content. Drop-downs and dialogs attach
// here so they scale together with the rest of the UI. Main.qml sets `layer`.
QtObject {
    property Item layer: null
}
