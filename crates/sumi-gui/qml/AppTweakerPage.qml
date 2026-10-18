pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2

// App Tweaker: switch the GameVisual mode by the application that has focus.
// The rules live in sumid's configuration; every change is saved at once.
PageBase {
    id: page

    title: i18n("App Tweaker")
    stateKey: "game_visual"
    waiting: !appsLoaded
    flick: rowsFlick

    readonly property int maxRules: 10
    // AI Visual picks the mode for apps without a rule while it is on.
    readonly property bool aiOn: host !== null && host.config.ai_visual !== undefined && host.config.ai_visual.enabled === true

    property var apps: []
    property var recents: []
    property bool appsLoaded: false

    readonly property var app: host && host.config.app_sync ? host.config.app_sync : ({})
    readonly property var rules: app.rule || []
    readonly property var modes: store ? store.options("gamevisual") : []

    // The app combo's model: recently focused windows, then installed apps.
    readonly property var appItems: {
        const items = [];
        const byKey = {};
        for (const a of apps) {
            byKey[a.key.toLowerCase()] = a;
            byKey[a.id.toLowerCase()] = a;
        }
        if (recents.length > 0) {
            items.push({ header: true, label: i18n("Recently used") });
            for (const r of recents) {
                const key = r.class || r.desktop_file || r.exe;
                const known = byKey[(r.class || "").toLowerCase()] || byKey[(r.desktop_file || "").replace(/\.desktop$/, "").toLowerCase()];
                items.push({ label: r.exe || r.class || r.desktop_file, key: key, icon: known ? known.icon : "" });
            }
        }
        if (apps.length > 0) {
            items.push({ header: true, label: i18n("Installed apps") });
            for (const a of apps) {
                items.push({ label: a.name, key: a.key, icon: a.icon || "" });
            }
        }
        return items;
    }

    // The first app that two rules map to different modes, or "".
    readonly property string conflict: {
        const seen = {};
        for (const r of rules) {
            const k = (r.app || "").trim().toLowerCase();
            if (k === "") {
                continue;
            }
            if (k in seen && seen[k].mode !== r.mode) {
                return r.name || r.app;
            }
            seen[k] = r;
        }
        return "";
    }

    function modeIndex(mode) {
        return modes.findIndex(o => o.value === mode || (o.label === "sRGB" && (mode === 3 || mode === 10)));
    }

    function appIndex(key) {
        const k = (key || "").toLowerCase();
        return k === "" ? -1 : appItems.findIndex(i => !i.header && i.key.toLowerCase() === k);
    }

    function setRuleApp(index, item) {
        host.updateConfig(c => {
            const r = (c.app_sync.rule || [])[index];
            if (r) {
                r.app = item.key;
                r.name = item.label;
            }
        });
    }

    function setRuleMode(index, value) {
        host.updateConfig(c => {
            const r = (c.app_sync.rule || [])[index];
            if (r) {
                r.mode = value;
            }
        });
    }

    function removeRule(index) {
        host.updateConfig(c => {
            c.app_sync.rule = c.app_sync.rule || [];
            c.app_sync.rule.splice(index, 1);
        });
    }

    function addRule() {
        const first = modes.length > 0 ? modes[0].value : 0;
        host.updateConfig(c => {
            c.app_sync.rule = c.app_sync.rule || [];
            c.app_sync.rule.push({ app: "", name: "", mode: first });
        });
    }

    function setDefaultMode(i) {
        host.updateConfig(c => {
            if (i <= 0 || i > modes.length) {
                delete c.app_sync.default_mode;
            } else {
                c.app_sync.default_mode = modes[i - 1].value;
            }
        });
    }

    function loadApps() {
        let pending = 2;
        const done = () => {
            if (--pending === 0) {
                appsLoaded = true;
            }
        };
        host.call("ListApps", [], false, (ok, text) => {
            apps = ok ? host.json(text, []) : [];
            done();
        });
        host.call("RecentWindows", [], false, (ok, text) => {
            recents = ok ? host.json(text, []) : [];
            done();
        });
    }

    Component.onCompleted: {
        host.loadConfig();
        loadApps();
    }

    // ---- Header
    Text {
        x: 31
        y: 58
        width: 700
        height: 38
        wrapMode: Text.WordWrap
        fontSizeMode: Text.Fit
        minimumPixelSize: 11
        elide: Text.ElideRight
        text: i18n("Switch GameVisual mode automatically when an application gets focus.")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Row {
        x: Theme.pageMargin + Theme.contentWidth - width
        y: 26
        spacing: 14
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: page.app.enabled ? i18n("On") : i18n("Off")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
        RogToggle {
            objectName: "toggle_app_sync"
            anchors.verticalCenter: parent.verticalCenter
            checked: !!page.app.enabled
            onToggled: on => page.host.updateConfig(c => {
                c.app_sync.enabled = on;
            })
        }
    }

    // Column titles
    Text {
        x: Theme.pageMargin
        y: 98
        text: i18n("App")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }
    Text {
        x: Theme.pageMargin + 446
        y: 98
        text: i18n("Mode")
        color: Theme.textDim
        font.family: Theme.font
        font.pixelSize: Theme.fontBody
    }

    // ---- Rows
    Flickable {
        id: rowsFlick
        x: Theme.pageMargin
        y: 124
        width: Theme.contentWidth + 18
        height: Math.min(rows.height, 400)
        clip: true
        contentWidth: Theme.contentWidth
        contentHeight: rows.height
        boundsBehavior: Flickable.StopAtBounds
        QQC2.ScrollBar.vertical: RogScrollBar {}

        Column {
            id: rows
            width: Theme.contentWidth
            spacing: 6

            // The fixed first row: what apps without a rule get
            Item {
                width: parent.width
                height: 40
                RogCombo {
                    objectName: "app_default"
                    width: 426
                    enabled: false
                    displayText: i18n("Default Setting")
                    iconsAlways: true
                }
                RogCombo {
                    objectName: "mode_default_ai"
                    visible: page.aiOn
                    x: 446
                    width: 296
                    enabled: false
                    displayText: i18n("AI Visual")
                }
                Text {
                    objectName: "ai_note"
                    visible: page.aiOn
                    x: 760
                    width: 208
                    height: 32
                    verticalAlignment: Text.AlignVCenter
                    wrapMode: Text.WordWrap
                    text: i18n("Unlisted apps follow AI Visual")
                    color: Theme.textDim
                    font.family: Theme.font
                    font.pixelSize: Theme.fontSmall
                }
                RogCombo {
                    objectName: "mode_default"
                    visible: !page.aiOn
                    x: 446
                    width: 296
                    enabled: page.app.enabled === true
                    model: [i18n("No change")].concat(page.modes.map(o => page.store.tr(o.label)))
                    currentIndex: page.app.default_mode === undefined || page.app.default_mode === null ? 0 : page.modeIndex(page.app.default_mode) + 1
                    onActivated: i => page.setDefaultMode(i)
                }
            }

            Repeater {
                model: page.rules
                delegate: Item {
                    id: ruleRow
                    required property var modelData
                    required property int index
                    width: rows.width
                    height: 40
                    RogCombo {
                        objectName: "app_" + ruleRow.index
                        width: 426
                        enabled: page.app.enabled === true
                        iconsAlways: true
                        model: page.appItems
                        currentIndex: page.appIndex(ruleRow.modelData.app)
                        forceText: ruleRow.modelData.name
                        displayText: ruleRow.modelData.app || i18n("Choose an app")
                        onActivated: i => page.setRuleApp(ruleRow.index, page.appItems[i])
                    }
                    RogCombo {
                        objectName: "mode_" + ruleRow.index
                        x: 446
                        width: 296
                        enabled: page.app.enabled === true
                        model: page.store.displayOptions("gamevisual")
                        currentIndex: page.modeIndex(ruleRow.modelData.mode)
                        onActivated: i => page.setRuleMode(ruleRow.index, page.modes[i].value)
                    }
                    RogButton {
                        objectName: "delete_" + ruleRow.index
                        x: 762
                        width: 40
                        secondary: true
                        enabled: page.app.enabled === true
                        onClicked: page.removeRule(ruleRow.index)
                        Icon {
                            anchors.centerIn: parent
                            size: 18
                            lineWidth: 1.4
                            name: "trash"
                            color: parent.enabled ? Theme.text : Theme.disabledText
                        }
                    }
                }
            }
        }
    }

    // ---- Add, conflict
    Row {
        id: addRow
        x: Theme.pageMargin
        y: rowsFlick.y + rowsFlick.height + 14
        spacing: 20
        RogButton {
            objectName: "add_rule"
            visible: page.rules.length < page.maxRules && page.conflict === ""
            text: i18n("Add")
            enabled: page.app.enabled === true
            onClicked: page.addRule()
        }
        Text {
            objectName: "conflict"
            visible: page.conflict !== ""
            height: 32
            verticalAlignment: Text.AlignVCenter
            text: i18n("Settings conflict: %1", page.conflict)
            color: Theme.error
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
    }

    // ---- Options
    Column {
        x: Theme.pageMargin
        y: addRow.y + 32 + 22
        width: Theme.contentWidth

        SettingRow {
            width: parent.width
            height: 56
            title: i18n("Restore the previous mode when the app loses focus")
            tag: i18n("Sumi")
            description: i18n("Without this the mode of the last app stays on.")
            dim: page.app.enabled !== true
            RogToggle {
                objectName: "toggle_restore"
                anchors.verticalCenter: parent.verticalCenter
                enabled: page.app.enabled === true
                checked: !!page.app.restore_on_unfocus
                onToggled: on => page.host.updateConfig(c => {
                    c.app_sync.restore_on_unfocus = on;
                })
            }
        }
        SettingRow {
            width: parent.width
            height: 56
            title: i18n("Turn HDR off when an app's mode needs SDR")
            tag: i18n("Sumi")
            description: i18n("GameVisual modes other than HDR only apply while HDR is off.")
            divider: false
            dim: page.app.enabled !== true
            RogToggle {
                objectName: "toggle_hdr_off"
                anchors.verticalCenter: parent.verticalCenter
                enabled: page.app.enabled === true
                checked: page.app.hdr_off_for_mode !== false
                onToggled: on => page.host.updateConfig(c => {
                    c.app_sync.hdr_off_for_mode = on;
                })
            }
        }
    }
}
