import QtQuick

// User Center: what the monitor reports about itself, with a Copy button.
PageBase {
    id: page

    title: i18n("User Center")
    stateKey: "device_info,system,game_visual"

    property bool copied: false
    property var openUrl: url => Qt.openUrlExternally(url)

    // ---- Firmware update check (only when the button is pressed: it asks ASUS's download server)
    // fwState: unchecked | checking | uptodate | available | downloading | saved | error
    property string fwState: "unchecked"
    property var fwInfo: null
    property real fwDone: 0
    property real fwTotal: 0
    property string fwPath: ""
    property string fwError: ""
    readonly property string fwLatest: fwInfo && fwInfo.latest ? String(fwInfo.latest) : ""
    readonly property string fwFolder: fwPath.indexOf("/") >= 0 ? fwPath.substring(0, fwPath.lastIndexOf("/")) : ""

    function checkFirmware() {
        if (!host || !store || fwState === "checking" || fwState === "downloading") {
            return;
        }
        fwState = "checking";
        host.call("CheckFirmware", [store.monitorId], false, (ok, text) => {
            if (!ok) {
                fwError = text;
                fwState = "error";
                return;
            }
            fwInfo = host.json(text, {});
            fwState = fwInfo.latest ? "available" : "uptodate";
        });
    }
    function downloadFirmware() {
        if (!host || !store || fwState !== "available") {
            return;
        }
        fwState = "downloading";
        fwDone = 0;
        fwTotal = 0;
        host.call("DownloadFirmware", [store.monitorId], false, (ok, text) => {
            if (!ok) {
                fwError = text;
                fwState = "error";
            }
        });
    }
    function pollFirmware() {
        host.call("FirmwareStatus", [], false, (ok, text) => {
            if (!ok || fwState !== "downloading") {
                return;
            }
            const d = host.json(text, {}).download || {};
            if (d.state === "done") {
                fwPath = d.path || "";
                fwState = "saved";
            } else if (d.state === "error") {
                fwError = d.error || "";
                fwState = "error";
            } else {
                fwDone = d.done || 0;
                fwTotal = d.total || 0;
            }
        });
    }
    Timer {
        interval: 500
        repeat: true
        running: page.fwState === "downloading" && page.host !== null
        onTriggered: page.pollFirmware()
    }

    readonly property var fw: store ? store.info.firmware : undefined

    function firmwareText() {
        if (!fw || fw.fw_version === undefined) return "—";
        return (fw.fw_version <= 100 ? "MCE" : "MCM") + fw.fw_version;
    }
    function schemeText() {
        return fw && fw.scheme_major !== undefined ? fw.scheme_major + "." + fw.scheme_minor : "—";
    }
    function refreshText() {
        const v = store.value("vertical_freq");
        if (v === undefined || v === 0) return "—";
        const hz = v / 100;
        return i18n("%1 Hz", Number.isInteger(hz) ? String(hz) : hz.toFixed(2));
    }
    function hoursText() {
        const v = store.value("usage_hours");
        return v === undefined ? "—" : i18n("%1 h", v);
    }
    function inputText() {
        const l = store.optionText("input", store.value("input"));
        return l !== "" ? l : "—";
    }
    function modeText() {
        if (store.context.hdr === true) return i18n("HDR");
        const v = store.value("gamevisual");
        const l = store.optionText("gamevisual", v);
        return l !== "" ? l : (v === 3 ? "sRGB" : "—");
    }
    function brightnessText() {
        const v = store.value("brightness");
        return v === undefined ? "—" : String(v);
    }
    function hdrText() {
        return store.context.hdr === true ? i18n("On") : i18n("Off");
    }
    function formatText() {
        const n = store.hdrFormatName();
        return n !== "" ? n : "—";
    }
    function connectorText() {
        return store.info.connector ? store.info.connector : "—";
    }
    function busText() {
        return store.info.bus !== undefined ? "i2c-" + store.info.bus : "—";
    }

    readonly property var cells: store ? [
        [i18n("Model"), store.info.model || "—"],
        [i18n("Firmware"), firmwareText()],
        [i18n("VCP scheme"), schemeText()],
        [i18n("Refresh rate"), refreshText()],
        [i18n("Display usage time"), hoursText()],
        [i18n("Input"), inputText()],
        [i18n("GameVisual mode"), modeText()],
        [i18n("Brightness"), brightnessText()],
        [i18n("HDR"), hdrText()],
        [i18n("HDR format"), formatText()],
        [i18n("I2C bus"), busText()],
        [i18n("Connector"), connectorText()]
    ] : []

    function asText() {
        let t = i18n("Sumi device info") + "\n";
        for (const c of cells) {
            t += c[0] + ": " + c[1] + "\n";
        }
        return t;
    }

    Card {
        x: Theme.pageMargin
        y: 76
        width: Theme.contentWidth
        height: 46 + 14 + Math.ceil(page.cells.length / 2) * 66 + 16 + 32 + 16
        title: i18n("Device")

        Grid {
            x: 20
            y: 14
            columns: 2
            columnSpacing: 0
            rowSpacing: 0
            Repeater {
                model: page.cells
                delegate: Item {
                    required property var modelData
                    required property int index
                    width: 464
                    height: 66
                    Text {
                        y: 10
                        text: parent.modelData[0]
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontBody
                    }
                    Text {
                        y: 32
                        text: parent.modelData[1]
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: 20
                        font.weight: Font.Medium
                    }
                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width - 24
                        height: 1
                        color: Theme.separator
                        opacity: 0.3
                    }
                }
            }
        }

        // In the Firmware cell (the first row, right-hand column).
        RogButton {
            objectName: "fw_check"
            visible: page.host !== null
            x: 20 + 464 + 464 - 24 - width
            y: 14 + 18
            height: 30
            text: i18n("Check for updates")
            secondary: true
            enabled: page.fwState !== "checking" && page.fwState !== "downloading"
            onClicked: page.checkFirmware()
        }

        Text {
            visible: page.copied
            anchors.right: copyButton.left
            anchors.rightMargin: 16
            anchors.verticalCenter: copyButton.verticalCenter
            text: i18n("Copied to the clipboard")
            color: Theme.ok
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        RogButton {
            id: copyButton
            objectName: "copy"
            x: parent.width - width - 20
            y: parent.height - height - 16
            text: i18n("Copy")
            onClicked: {
                page.backend.copy_text(page.asText());
                page.copied = true;
                resetCopied.restart();
            }
        }
        Timer {
            id: resetCopied
            interval: 2500
            onTriggered: page.copied = false
        }
    }

    property var backend: null

    // ---- Firmware update result, below the device card
    Item {
        id: fwPanel
        objectName: "fw_panel"
        visible: page.host !== null && page.store !== null
        x: Theme.pageMargin
        y: 76 + 46 + 14 + Math.ceil(page.cells.length / 2) * 66 + 16 + 32 + 16 + 12
        width: Theme.contentWidth
        height: 96

        Text {
            objectName: "fw_status"
            x: 4
            y: 0
            height: 32
            width: parent.width - 4 - (fwButton.visible ? fwButton.width + 16 : 0)
            verticalAlignment: Text.AlignVCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            fontSizeMode: Text.Fit
            minimumPixelSize: 11
            elide: Text.ElideRight
            visible: page.fwState !== "downloading"
            color: page.fwState === "error" ? Theme.error : (page.fwState === "uptodate" || page.fwState === "saved" ? Theme.ok : (page.fwState === "available" ? Theme.text : Theme.textDim))
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
            text: {
                switch (page.fwState) {
                case "checking": return i18n("Checking…");
                case "uptodate": return i18n("Up to date");
                case "available": return i18n("%1 is available", page.fwLatest);
                case "saved": return i18n("Saved to %1", page.fwPath);
                case "error": return page.fwError;
                default: return i18n("Asks ASUS's download server whether newer firmware exists; only the model name is sent.");
                }
            }
        }
        ScoreBar {
            objectName: "fw_progress"
            visible: page.fwState === "downloading"
            x: 4
            width: 520
            label: i18n("Downloading %1", page.fwLatest)
            value: page.fwTotal > 0 ? page.fwDone / page.fwTotal * 100 : 0
            valueText: page.fwTotal > 0 ? i18n("%1 of %2 MB", (page.fwDone / 1000000).toFixed(1), (page.fwTotal / 1000000).toFixed(1)) : ""
        }
        RogButton {
            id: fwButton
            objectName: page.fwState === "saved" ? "fw_open" : "fw_download"
            visible: page.fwState === "available" || page.fwState === "saved"
            x: parent.width - width
            y: 1
            height: 30
            text: page.fwState === "saved" ? i18n("Open folder") : i18n("Download")
            onClicked: {
                if (page.fwState === "saved") {
                    page.openUrl("file://" + page.fwFolder);
                } else {
                    page.downloadFirmware();
                }
            }
        }
        Text {
            objectName: "fw_note"
            visible: page.fwState === "saved"
            x: 4
            y: 40
            width: parent.width - 8
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
            text: i18n("Run ASUS's update tool from this file on Windows; Sumi doesn't flash firmware.")
        }
    }
}
