import QtQuick

// Application Settings: version, daemon status, start on login.
PageBase {
    id: page

    title: i18n("Application Settings")

    property var backend: null
    property bool autostart: false
    // How the user manual is opened (tests hand in a recorder).
    property var openUrl: url => Qt.openUrlExternally(url)
    property bool collecting: false

    function sync() {
        autostart = backend ? backend.autostart_enabled() : false;
    }
    Component.onCompleted: {
        sync();
        loadLanguage();
    }

    // The file dialog (tests hand in a stand-in).
    property var chooser: defaultChooser
    FileChooser {
        id: defaultChooser
    }

    function stamp() {
        const d = new Date();
        const p = n => String(n).padStart(2, "0");
        return d.getFullYear() + p(d.getMonth() + 1) + p(d.getDate());
    }

    // Collect what a bug report needs into one text file: the app and daemon state, every
    // monitor's capabilities, decoded state and host state. The daemon never sends serials.
    function exportDiagnostics() {
        page.chooser.save("sumi-diagnostics-" + stamp() + ".txt", url => collect(url), [i18n("Text files (*.txt)"), i18n("All files (*)")], "txt");
    }

    function collect(url) {
        collecting = true;
        const out = [];
        out.push("Sumi diagnostics");
        out.push("App version: " + backend.app_version());
        out.push("Daemon: " + (backend.connected ? (backend.readonly ? "connected, read-only" : "connected") : "not running")
            + (backend.daemon_version !== "" ? ", version " + backend.daemon_version : ""));
        out.push("");
        const finish = () => {
            collecting = false;
            backend.write_text_file(url, out.join("\n") + "\n");
        };
        const section = (title, ok, text) => {
            out.push("== " + title + " ==");
            out.push(ok ? text : "error: " + text);
            out.push("");
        };
        host.call("ListMonitors", [], false, (ok, text) => {
            section("ListMonitors", ok, text);
            host.call("Problems", [], false, (ok2, text2) => {
                section("Problems", ok2, text2);
                const monitors = ok ? host.json(text, []) : [];
                const tasks = [];
                for (const m of monitors) {
                    tasks.push([m.id, "Capabilities", [m.id]]);
                    tasks.push([m.id, "GetState", [m.id, []]]);
                    tasks.push([m.id, "GetHostState", [m.id]]);
                }
                const next = i => {
                    if (i >= tasks.length) {
                        finish();
                        return;
                    }
                    const t = tasks[i];
                    host.call(t[1], t[2], false, (ok3, text3) => {
                        section(t[0] + " " + t[1], ok3, text3);
                        next(i + 1);
                    });
                };
                next(0);
            });
        });
    }

    // ---- Language: fixed for a run, so a change asks for a restart.
    readonly property var languageChoices: [
        { pref: "", label: i18n("System language") },
        { pref: "osd", label: i18n("Same as the monitor menu") }
    ].concat(Languages.list.map(l => ({ pref: l.locale, label: l.name })))
    property string languagePref: ""
    property bool languageChanged: false

    function loadLanguage() {
        languagePref = backend ? backend.pref("language") : "";
    }

    function setLanguage(i) {
        const pref = languageChoices[i].pref;
        backend.set_pref("language", pref);
        languagePref = pref;
        languageChanged = true;
    }

    function openManual() {
        openUrl(backend.manual_url());
    }

    readonly property string daemonText: !backend ? "—" : (!backend.connected ? i18n("Not running") : (backend.readonly ? i18n("Connected, read-only") : i18n("Connected")) + (backend.daemon_version !== "" ? " · " + backend.daemon_version : ""))
    readonly property color daemonColor: !backend || !backend.connected ? Theme.error : (backend.readonly ? Theme.warn : Theme.ok)

    Card {
        x: Theme.pageMargin
        y: 76
        width: Theme.contentWidth
        height: rows.height + 24
        title: ""

        Column {
            id: rows
            x: 20
            y: 12
            width: parent.width - 40

            SettingRow {
                width: parent.width
                height: 70
                title: i18n("Version")
                description: i18n("The version of this app.")
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: page.backend ? page.backend.app_version() : ""
                    color: Theme.text
                    font.family: Theme.font
                    font.pixelSize: Theme.fontLabel
                }
            }
            SettingRow {
                width: parent.width
                height: 70
                title: i18n("Daemon")
                description: i18n("sumid talks to the monitor; this app only talks to sumid.")
                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 10
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 9
                        height: 9
                        rotation: 45
                        color: page.daemonColor
                    }
                    Text {
                        text: page.daemonText
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: Theme.fontLabel
                    }
                }
            }
            SettingRow {
                width: parent.width
                height: 70
                title: i18n("Language")
                description: i18n("The language of Sumi's windows.")
                RogCombo {
                    objectName: "combo_language"
                    anchors.verticalCenter: parent.verticalCenter
                    width: 260
                    model: page.languageChoices.map(c => c.label)
                    currentIndex: page.languageChoices.findIndex(c => c.pref === page.languagePref)
                    onActivated: i => page.setLanguage(i)
                }
            }
            SettingRow {
                objectName: "restart_row"
                visible: page.languageChanged
                width: parent.width
                height: visible ? 56 : 0
                title: i18n("Restart Sumi to change the language")
                RogButton {
                    objectName: "restart"
                    anchors.verticalCenter: parent.verticalCenter
                    text: i18n("Restart")
                    onClicked: page.backend.restart()
                }
            }
            SettingRow {
                width: parent.width
                height: 70
                title: i18n("Start on login")
                description: i18n("Opens Sumi minimised when you log in.")
                RogToggle {
                    objectName: "toggle_autostart"
                    anchors.verticalCenter: parent.verticalCenter
                    checked: page.autostart
                    onToggled: on => {
                        if (page.backend) {
                            page.backend.set_autostart(on);
                        }
                        page.sync();
                    }
                }
            }
            SettingRow {
                width: parent.width
                height: 70
                title: i18n("Diagnostics")
                description: i18n("Saves a text file with the app, daemon and monitor state for a bug report.")
                RogButton {
                    objectName: "export_diagnostics"
                    anchors.verticalCenter: parent.verticalCenter
                    minWidth: 170
                    text: page.collecting ? i18n("Collecting…") : i18n("Export diagnostics")
                    enabled: !page.collecting && page.backend !== null && page.backend.connected
                    onClicked: page.exportDiagnostics()
                }
            }
            SettingRow {
                width: parent.width
                height: 70
                title: i18n("User manual")
                description: i18n("How to use Sumi, step by step.")
                divider: false
                RogButton {
                    objectName: "open_manual"
                    anchors.verticalCenter: parent.verticalCenter
                    text: i18n("Open")
                    onClicked: page.openManual()
                }
            }
        }
    }
}
