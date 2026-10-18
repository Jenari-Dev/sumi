import QtQuick
import QtQuick.Dialogs
import QtCore

// Opens the system's file dialog on demand. `save(name, cb)` / `open(cb)` call
// `cb(url)` with the chosen file's URL ("file://…") and nothing if cancelled.
Item {
    id: root

    property var callback: null
    // Name filters and the suffix added to names without one; the defaults suit settings files.
    property var filters: [i18n("Settings files (*.json)"), i18n("All files (*)")]
    property string suffix: "json"

    visible: false

    function save(suggestedName, cb, nameFilters, defaultSuffix) {
        callback = cb;
        filters = nameFilters || [i18n("Settings files (*.json)"), i18n("All files (*)")];
        suffix = defaultSuffix || "json";
        dialog.fileMode = FileDialog.SaveFile;
        dialog.currentFile = StandardPaths.writableLocation(StandardPaths.DocumentsLocation) + "/" + suggestedName;
        dialog.open();
    }

    function open(cb) {
        callback = cb;
        dialog.fileMode = FileDialog.OpenFile;
        dialog.currentFolder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation);
        dialog.open();
    }

    FileDialog {
        id: dialog
        nameFilters: root.filters
        defaultSuffix: root.suffix
        onAccepted: {
            const cb = root.callback;
            root.callback = null;
            if (cb) {
                cb(dialog.selectedFile.toString());
            }
        }
        onRejected: root.callback = null
    }
}
