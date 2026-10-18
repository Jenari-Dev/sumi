pragma ComponentBehavior: Bound

import QtQuick

// AI Visual settings (replaces the Image / Color cards on the GameVisual page):
// the on/off switch with its one-time consent, the model download, which modes the
// recogniser may pick, what to do otherwise, and a live "Now seeing" read-out.
// The daemon does the recognising; this panel only edits its config and shows
// its status, polling it once a second while the panel is on screen.
Item {
    id: panel

    property var store: null
    property var host: null
    property int pollInterval: 1000
    signal closeRequested

    // ---- What the daemon says
    readonly property var ai: host && host.config.ai_visual ? host.config.ai_visual : ({})
    readonly property bool aiOn: ai.enabled === true
    readonly property var status: host ? host.aiStatus : ({})
    readonly property var daemonState: status.state || ({})
    readonly property bool installed: status.installed === true
    readonly property bool promptsOk: status.prompts !== false
    readonly property var model: status.model || ({})
    readonly property int sizeMb: Math.round((model.size || 352000000) / 1000000)
    readonly property var download: daemonState.download || null
    readonly property bool downloading: download !== null && download !== undefined
    readonly property var verdictOf: host && store && daemonState.monitors ? daemonState.monitors[store.monitorId] : undefined
    // While AI Visual is off, the read-out shows a preview (nothing is switched).
    readonly property bool previewing: !aiOn && installed && promptsOk
    readonly property var preview: host && host.aiPreview ? host.aiPreview : ({})
    readonly property var seen: aiOn ? (verdictOf && verdictOf.verdict ? verdictOf.verdict : null)
                                     : (previewing && preview.verdict ? preview.verdict : null)
    readonly property string errorText: daemonState.error || (previewing && preview.error ? preview.error : "")
    readonly property bool unavailable: host !== null && host.aiError !== ""

    // The modes AI Visual may pick, in the order the page lists them.
    readonly property var modeList: [
        { key: "rts_rpg", label: i18n("Game (RTS/RPG)"), blurb: i18n("Role-playing and strategy games") },
        { key: "fps", label: i18n("FPS"), blurb: i18n("Shooters") },
        { key: "scenery", label: i18n("Scenery"), blurb: i18n("Landscapes and photos") },
        { key: "racing", label: i18n("Racing"), blurb: i18n("Driving games") },
        { key: "moba", label: i18n("MOBA"), blurb: i18n("Battle arenas") },
        { key: "srgb", label: i18n("Office (sRGB)"), blurb: i18n("Documents, web and apps") }
    ]
    readonly property var defaultModes: ["rts_rpg", "fps", "scenery", "racing", "srgb", "moba"]
    // Every mode the daemon knows, in the order of the mode tiles (for the preferred mode).
    readonly property var allModes: [
        { key: "rts_rpg", label: i18n("RTS/RPG") }, { key: "fps", label: i18n("FPS") }, { key: "cinema", label: i18n("Cinema") },
        { key: "scenery", label: i18n("Scenery") }, { key: "racing", label: i18n("Racing") }, { key: "srgb", label: i18n("sRGB") },
        { key: "moba", label: i18n("MOBA") }, { key: "user", label: i18n("User") }
    ]
    readonly property var sceneNames: ({
        "fps": i18n("FPS"), "moba": i18n("MOBA"), "office": i18n("Office"), "racing": i18n("Racing"),
        "rpg": i18n("Game (RPG)"), "scenery": i18n("Scenery")
    })

    function modeLabel(key) {
        const m = allModes.find(x => x.key === key);
        return m ? m.label : String(key);
    }

    function modeIndex(key) {
        return allModes.findIndex(x => x.key === key);
    }

    function hasMode(key) {
        return (ai.modes || []).indexOf(key) >= 0;
    }

    // ---- Edits (all read-modify-write on the whole config)
    function edit(fn) {
        host.updateConfig(c => {
            c.ai_visual = c.ai_visual || {};
            fn(c.ai_visual);
        });
    }

    function setEnabled(on) {
        edit(a => {
            a.enabled = on;
        });
    }

    // Turning it on the first time explains what it looks at.
    function toggleEnabled(on) {
        if (on && host.backend.pref("ai_visual_consent") !== "1") {
            store.ask(i18n("AI Visual"),
                i18n("AI Visual looks at what's on this monitor about once a second to pick a GameVisual mode. The picture stays in memory on this PC; nothing is saved or sent anywhere."),
                i18n("Turn on"), () => {
                    host.backend.set_pref("ai_visual_consent", "1");
                    setEnabled(true);
                });
            return;
        }
        setEnabled(on);
    }

    function setMode(key, on) {
        edit(a => {
            const rest = (a.modes || []).filter(m => m !== key);
            if (on) {
                rest.push(key);
            }
            a.modes = rest;
        });
    }

    function setFallback(key) {
        edit(a => {
            if (key === null) {
                delete a.fallback;
            } else {
                a.fallback = key;
            }
        });
    }

    function reset() {
        edit(a => {
            a.modes = panel.defaultModes.slice();
            a.notify = true;
            delete a.fallback;
        });
    }

    function askDownload() {
        store.ask(i18n("Download the AI model"),
            i18n("Download the AI Visual model? %1, %2 MB, from huggingface.co (OpenAI CLIP ViT-B/32, MIT licence). It is checked against its published SHA-256 and kept in ~/.local/share/sumi/models.",
                model.name || "vision_model.onnx", sizeMb),
            i18n("Download"), () => host.downloadAi());
    }

    function askRemove() {
        store.ask(i18n("Remove the AI model"), i18n("Remove the AI Visual model from this PC? You can download it again later."), i18n("Remove"), () => {
            host.removeAi(ok => {
                if (ok && panel.aiOn) {
                    panel.setEnabled(false);
                }
            });
        });
    }

    Component.onCompleted: host.loadAi()
    onVisibleChanged: if (visible) host.loadAi()

    // Poll the daemon only while the panel is on screen.
    Timer {
        interval: panel.pollInterval
        repeat: true
        running: panel.visible && panel.host !== null
        onTriggered: panel.host.loadAi()
    }
    Timer {
        objectName: "ai_preview_timer"
        interval: 2000
        repeat: true
        triggeredOnStart: true
        running: panel.visible && panel.host !== null && panel.previewing && panel.store !== null
        onTriggered: panel.host.previewAi(panel.store.monitorId)
    }

    // ---- Header
    Text {
        x: 0
        y: 0
        height: 32
        verticalAlignment: Text.AlignVCenter
        text: i18n("AI Visual mode settings")
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontLabel
        font.weight: Font.Medium
    }
    Row {
        x: panel.width - width
        y: 4
        spacing: 14
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: i18n("AI Visual")
            color: toggle.enabled ? Theme.text : Theme.disabledText
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
        RogToggle {
            id: toggle
            objectName: "toggle_ai"
            anchors.verticalCenter: parent.verticalCenter
            enabled: panel.installed && panel.promptsOk && !panel.unavailable
            checked: panel.aiOn
            onToggled: on => panel.toggleEnabled(on)
        }
    }

    // ---- Model
    Item {
        id: modelRow
        x: 0
        y: 44
        width: 560
        height: 44

        Text {
            visible: !panel.installed
            anchors.verticalCenter: parent.verticalCenter
            text: i18n("AI model")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
            font.weight: Font.Medium
        }
        RogButton {
            objectName: "download_ai"
            visible: !panel.installed && !panel.downloading && !panel.unavailable
            x: parent.width - width
            anchors.verticalCenter: parent.verticalCenter
            minWidth: 200
            text: i18n("Download (%1 MB)", panel.sizeMb)
            onClicked: panel.askDownload()
        }
        // Progress
        ScoreBar {
            objectName: "download_progress"
            visible: panel.downloading
            x: 130
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 130
            label: i18n("Downloading the model")
            value: panel.downloading && panel.download[1] > 0 ? panel.download[0] / panel.download[1] * 100 : 0
            valueText: panel.downloading ? i18n("%1 of %2 MB", Math.round(panel.download[0] / 1000000), Math.round(panel.download[1] / 1000000)) : ""
        }
        // Installed
        Text {
            objectName: "ai_installed"
            visible: panel.installed
            anchors.verticalCenter: parent.verticalCenter
            text: i18n("AI model installed (%1 MB)", panel.sizeMb)
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        RogButton {
            objectName: "remove_ai"
            visible: panel.installed
            x: parent.width - width
            anchors.verticalCenter: parent.verticalCenter
            minWidth: 90
            height: 26
            text: i18n("Remove")
            secondary: true
            onClicked: panel.askRemove()
        }
        Text {
            visible: panel.unavailable
            anchors.verticalCenter: parent.verticalCenter
            text: i18n("This sumid has no AI Visual; update it.")
            color: Theme.warn
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
    }

    RogCheckBox {
        objectName: "ai_notify"
        x: 0
        y: 96
        text: i18n("Show Notifications")
        checked: panel.ai.notify !== false
        onToggled: on => panel.edit(a => {
            a.notify = on;
        })
    }

    // ---- Which modes it may pick
    Rectangle {
        id: frame
        x: 0
        y: 134
        width: 560
        height: 6 * 34 + 12
        color: Theme.alpha(Theme.black, 0.3)
        border.width: 1
        border.color: Theme.borderTile

        Column {
            x: 16
            y: 6
            Repeater {
                model: panel.modeList
                delegate: Item {
                    id: modeRow
                    required property var modelData
                    width: frame.width - 32
                    height: 34
                    RogCheckBox {
                        objectName: "ai_mode_" + modeRow.modelData.key
                        anchors.verticalCenter: parent.verticalCenter
                        text: modeRow.modelData.label
                        checked: panel.hasMode(modeRow.modelData.key)
                        onToggled: on => panel.setMode(modeRow.modelData.key, on)
                    }
                    Text {
                        x: 190
                        anchors.verticalCenter: parent.verticalCenter
                        text: modeRow.modelData.blurb
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontBody
                    }
                }
            }
        }
    }

    // ---- Everything else
    Text {
        x: 0
        y: 358
        text: i18n("Others:")
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
        font.weight: Font.Medium
    }
    RogRadio {
        objectName: "ai_keep"
        x: 0
        y: 384
        text: i18n("Remain unchanged")
        checked: !panel.ai.fallback
        onClicked: panel.setFallback(null)
    }
    RogRadio {
        id: assign
        objectName: "ai_assign"
        x: 0
        y: 418
        text: i18n("Assign my preference:")
        checked: !!panel.ai.fallback
        onClicked: panel.setFallback(panel.ai.fallback || "user")
    }
    RogCombo {
        objectName: "ai_fallback"
        x: 270
        y: 414
        width: 200
        enabled: !!panel.ai.fallback
        model: panel.allModes.map(m => m.label)
        currentIndex: panel.modeIndex(panel.ai.fallback)
        onActivated: i => panel.setFallback(panel.allModes[i].key)
    }

    // ---- Now seeing
    Card {
        id: now
        x: 592
        y: 44
        width: 376
        height: 410
        title: i18n("Now seeing")

        Column {
            x: 20
            y: 14
            width: parent.width - 40
            spacing: 8

            Repeater {
                model: 3
                delegate: ScoreBar {
                    id: scene
                    required property int index
                    objectName: "scene_" + index
                    width: parent.width
                    readonly property var entry: panel.seen && panel.seen.scores && panel.seen.scores.length > index ? panel.seen.scores[index] : null
                    label: entry ? (panel.sceneNames[entry[0]] || entry[0]) : "—"
                    value: entry ? entry[1] : 0
                    dim: entry === null
                }
            }
            Rectangle {
                width: parent.width
                height: 1
                color: Theme.separator
                opacity: 0.4
            }
            ScoreBar {
                objectName: "gauge_game"
                width: parent.width
                label: i18n("Game")
                value: panel.seen ? panel.seen.is_game * 100 : 0   // daemon: 0..1
                dim: panel.seen === null
            }
            ScoreBar {
                objectName: "gauge_confidence"
                width: parent.width
                label: i18n("Confidence")
                value: panel.seen ? panel.seen.confidence * 100 : 0
                dim: panel.seen === null
            }
            Text {
                objectName: "ai_target"
                width: parent.width
                topPadding: 6
                text: i18n("Mode: %1", panel.seen && panel.seen.target ? panel.modeLabel(panel.seen.target) : i18n("unchanged"))
                color: Theme.text
                font.family: Theme.font
                font.pixelSize: Theme.fontLabel
                font.weight: Font.Medium
            }
            Text {
                objectName: "ai_paused"
                visible: !!(panel.verdictOf && panel.verdictOf.paused)
                width: parent.width
                wrapMode: Text.WordWrap
                text: i18n("Paused: %1", panel.verdictOf && panel.verdictOf.paused ? i18n(panel.verdictOf.paused) : "")
                color: Theme.warn
                font.family: Theme.font
                font.pixelSize: Theme.fontBody
            }
            Text {
                objectName: "ai_error"
                visible: panel.errorText !== ""
                width: parent.width
                wrapMode: Text.WordWrap
                text: panel.errorText
                color: Theme.error
                font.family: Theme.font
                font.pixelSize: Theme.fontBody
            }
            Text {
                objectName: "ai_idle"
                visible: !panel.aiOn && panel.errorText === ""
                width: parent.width
                wrapMode: Text.WordWrap
                text: panel.installed ? i18n("Preview: AI Visual is off, so nothing is switched.") : i18n("Download the model to try AI Visual.")
                color: Theme.textDim
                font.family: Theme.font
                font.pixelSize: Theme.fontBody
            }
        }
    }

    // ---- Buttons
    Row {
        x: panel.width - width
        y: 470
        spacing: 14
        RogButton {
            objectName: "ai_reset"
            text: i18n("Reset")
            onClicked: panel.reset()
        }
        RogButton {
            objectName: "ai_close"
            text: i18n("Close")
            secondary: true
            onClicked: panel.closeRequested()
        }
    }
}
