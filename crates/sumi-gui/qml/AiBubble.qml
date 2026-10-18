import QtQuick

// AI Visual's on-screen bubble: an orb per monitor that AI Visual runs on, and a short message
// when it switches the GameVisual mode. Driven by "Show Notifications" (ai_visual.notify);
// see AiBubbleModel for the rules and the SUMI_AI_FAKE test hook.
Item {
    id: bubble

    property var host: null
    property var backend: null
    property var monitors: []
    readonly property alias model: watcher

    visible: false

    AiBubbleModel {
        id: watcher
        host: bubble.host
        backend: bubble.backend
        monitors: bubble.monitors
        fakePath: bubble.backend ? bubble.backend.env_var("SUMI_AI_FAKE") : ""
    }

    Repeater {
        model: watcher.targetIds
        delegate: AiBubbleOverlay {
            required property string modelData
            ctl: watcher
            targetId: modelData
        }
    }
}
