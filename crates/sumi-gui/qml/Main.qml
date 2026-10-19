import QtQuick
import QtQuick.Controls as QQC2
import Qt.labs.platform as Labs
import io.github.jenaridev.sumi

// Sumi main window: frameless, designed at 1280 x 818 and scaled as one piece.
QQC2.ApplicationWindow {
    id: win

    title: i18n("Sumi")
    visible: false   // shown once this is the only instance (see Component.onCompleted)
    flags: Qt.Window | Qt.FramelessWindowHint
    width: Theme.designWidth
    height: Theme.designHeight
    // Small enough for a tiling layout's tile; the content scales and letterboxes.
    minimumWidth: 640
    minimumHeight: 409
    color: Theme.black

    // Page ids as used by the sidebar and SUMI_PAGE. `key` is the csv of daemon
    // pages the page reads ("" = no monitor state needed).
    readonly property var pages: ({
        "game_visual": { key: "game_visual", device: true },
        "game_plus": { key: "game_plus", device: true },
        "oled_care": { key: "oled_care", device: true },
        "multi_screen": { key: "multi_screen", device: true },
        "hotkey": { key: "", device: false },
        "app_tweaker": { key: "game_visual", device: true },
        "system": { key: "system", device: true },
        "user_center": { key: "device_info,system,game_visual", device: true },
        "app_settings": { key: "", device: false }
    })

    property string page: "game_visual"
    property var monitors: []
    property bool refreshed: false
    property string dismissedStatus: ""

    readonly property var pageInfo: pages[page] || pages["game_visual"]
    readonly property var monitor: monitors.find(m => m.id === sumiStore.monitorId) || null
    readonly property bool oled: monitor !== null && monitor.oled === true
    readonly property bool offline: refreshed && !sumiBackend.connected
    readonly property bool noMonitor: refreshed && sumiBackend.connected && monitors.length === 0 && pageInfo.device

    readonly property string screenshotPath: sumiBackend.env_var("SUMI_SCREENSHOT")
    // SUMI_CHECK_TEXT=1: report texts that don't fit (see TextCheck.qml); runs just before the
    // screenshot, or alone (the app quits afterwards) when there is no SUMI_SCREENSHOT.
    readonly property bool checkText: sumiBackend.env_var("SUMI_CHECK_TEXT") !== ""
    readonly property bool shooting: screenshotPath !== "" || checkText

    Backend {
        id: sumiBackend
        onMonitorsChanged: win.adoptMonitors()
    }

    Store {
        id: sumiStore
        backend: sumiBackend
        pageKey: win.pageInfo.key
    }

    Host {
        id: sumiHost
        backend: sumiBackend
    }

    // AI Visual's on-screen bubble (orb + "Switched to … mode"): its own small windows, shown
    // while AI Visual and Show Notifications are on, also when this window is in the tray.
    AiBubble {
        id: aiBubble
        host: sumiHost
        backend: sumiBackend
        monitors: win.monitors
    }

    // ---- Monitors and pages

    function adoptMonitors() {
        let list = [];
        try {
            list = JSON.parse(sumiBackend.monitors || "[]");
        } catch (e) {
            list = [];
        }
        win.monitors = list;
        sumiStore.monitors = list;
        if (list.length === 0) {
            sumiStore.monitorId = "";
        } else if (!list.some(m => m.id === sumiStore.monitorId)) {
            sumiStore.monitorId = list[0].id;
        }
        if (!win.oled && win.page === "oled_care") {
            win.page = "game_visual";
        }
        win.refreshed = true;
        win.checkProblems();
    }

    // ASUS monitors that were found but don't answer DDC/CI: say so once per session.
    property var problemsShown: ({})
    function checkProblems() {
        sumiHost.call("Problems", [], false, (ok, text) => {
            const list = ok ? sumiHost.json(text, []) : [];
            for (const p of list) {
                if (win.problemsShown[p.id] || sumiStore.confirm !== null) {
                    continue;
                }
                win.problemsShown[p.id] = true;
                const model = String(p.id).split("@")[0];
                sumiStore.notice(i18n("Monitor not responding"),
                    i18n("%1 was found but doesn't answer DDC/CI. Turn on DDC/CI in the monitor's menu (System Setup) and check that /dev/i2c-* is accessible.", model), false);
            }
        });
    }

    function requestPage() {
        if (sumiStore.pageKey !== "" && sumiStore.monitorId !== "") {
            sumiStore.request(sumiStore.pageKey);
        }
    }

    function select(id) {
        if (id === win.page) {
            win.requestPage();
        } else {
            win.page = id;
        }
    }

    onOledChanged: {
        if (!win.oled && win.page === "oled_care") {
            win.page = "game_visual";
        }
    }

    onPageChanged: {
        Qt.callLater(win.requestPage);
        win.maybeShoot();
    }

    Connections {
        target: sumiStore
        function onMonitorIdChanged() {
            Qt.callLater(win.requestPage);
            sumiHost.cacheOsdLanguage(sumiStore.monitorId);
        }
    }

    // While the daemon is away, look for it every few seconds.
    Timer {
        interval: 3000
        repeat: true
        running: win.offline
        onTriggered: sumiBackend.refresh()
    }

    // ---- Tray, single instance, closing
    readonly property bool headless: shooting
    // Qt Labs Platform aborts when asked for a tray icon without a tray, so ask the bus first.
    readonly property bool trayAllowed: !headless && sumiBackend.tray_available()
    readonly property bool trayUsable: trayLoader.item !== null && trayLoader.item.available
    property bool quitting: false

    function showFromTray() {
        if (win.visibility === Window.Hidden || win.visibility === Window.Minimized) {
            win.showNormal();
        } else {
            win.show();
        }
        win.raise();
        win.requestActivate();
    }

    function quit() {
        win.quitting = true;
        Qt.quit();
    }

    // The close button hides to the tray (when there is one); Quit in the tray menu really quits.
    onClosing: close => {
        if (win.trayUsable && !win.quitting) {
            close.accepted = false;
            win.hide();
            if (sumiBackend.pref("tray_notice_shown") !== "1") {
                sumiBackend.set_pref("tray_notice_shown", "1");
                trayLoader.item.showMessage(i18n("Sumi"), i18n("Sumi keeps running in the tray"));
            }
        } else {
            // The AI bubble's windows would keep the application alive after the last main window.
            win.quit();
        }
    }

    Loader {
        id: trayLoader
        active: win.trayAllowed
        sourceComponent: Labs.SystemTrayIcon {
            visible: true
            icon.source: "qrc:/qt/qml/io/github/jenaridev/sumi/qml/tray.svg"
            tooltip: i18n("Sumi")
            onActivated: reason => {
                if (reason === Labs.SystemTrayIcon.Trigger) {
                    win.showFromTray();
                }
            }
            menu: Labs.Menu {
                Labs.MenuItem {
                    text: i18n("Show Sumi")
                    onTriggered: win.showFromTray()
                }
                Labs.MenuItem {
                    text: i18n("Quit")
                    onTriggered: win.quit()
                }
            }
        }
    }

    Connections {
        target: sumiBackend
        function onActivate_requested() {
            win.showFromTray();
        }
    }

    // `--hidden` (autostart) starts in the tray; without a tray it starts minimised.
    function startVisible() {
        if (sumiBackend.hidden_start()) {
            if (!win.trayUsable) {
                win.showMinimized();
            }
        } else {
            win.show();
        }
    }

    Component.onCompleted: {
        if (!win.headless && !sumiBackend.claim_instance()) {
            Qt.quit();   // another Sumi owns the bus name and was asked to show itself
            return;
        }
        const wanted = sumiBackend.env_var("SUMI_PAGE");
        if (wanted !== "") {
            win.page = wanted === "device_info" ? "user_center" : wanted;
        }
        sumiBackend.refresh();
        sumiBackend.start_events();
        sumiHost.loadConfig();
        if (win.headless) {
            win.show();
        } else {
            Qt.callLater(win.startVisible);
        }
    }

    // ---- Aspect lock (like the vendor window): snap to 1280 : 818 after the user
    // drags a corner grip. Sizes set by the compositor (tiling scripts such as
    // Krohnkite, maximise, KWin's own tiles) are kept as they are: snapping those
    // back makes the tiler resize again, and the two fight (flicker).
    readonly property real ratio: Theme.designWidth / Theme.designHeight
    property bool userResizing: false
    property real lastW: width
    property real lastH: height
    Timer {
        id: snap
        interval: 120
        onTriggered: {
            const dw = Math.abs(win.width - win.lastW);
            const dh = Math.abs(win.height - win.lastH);
            win.lastW = win.width;
            win.lastH = win.height;
            if (!win.userResizing) {
                return;
            }
            win.userResizing = false;
            if (win.visibility !== Window.Windowed || Math.abs(win.width / win.height - win.ratio) < 0.004) {
                return;
            }
            if (dw >= dh) {
                win.height = Math.max(win.minimumHeight, Math.round(win.width / win.ratio));
            } else {
                win.width = Math.max(win.minimumWidth, Math.round(win.height * win.ratio));
            }
        }
    }
    onWidthChanged: snap.restart()
    onHeightChanged: snap.restart()

    // ---- Content, scaled as one piece
    Item {
        id: viewport
        anchors.fill: parent

        Item {
            id: scaler
            width: Theme.designWidth
            height: Theme.designHeight
            transformOrigin: Item.TopLeft
            scale: Math.min(win.width / Theme.designWidth, win.height / Theme.designHeight)
            x: (win.width - width * scale) / 2
            y: (win.height - height * scale) / 2

            Background {}

            // ---- Sidebar
            Sidebar {
                x: 0
                y: Theme.titleHeight
                store: sumiStore
                backend: sumiBackend
                monitors: win.monitors
                page: win.page
                oled: win.oled
                powerSaving: sumiStore.context.power_saving === true
                onPageSelected: id => win.select(id)
                onMonitorSelected: i => {
                    if (i >= 0 && i < win.monitors.length) {
                        sumiStore.monitorId = win.monitors[i].id;
                    }
                }
            }

            // ---- Page
            Item {
                id: content
                x: Theme.sidebarWidth
                y: Theme.titleHeight
                width: Theme.pageWidth
                height: Theme.pageHeight
                clip: true

                Loader {
                    id: pageLoader
                    anchors.fill: parent
                    active: !win.noMonitor
                    sourceComponent: {
                        switch (win.page) {
                        case "game_visual": return gameVisual;
                        case "game_plus": return gamePlus;
                        case "oled_care": return oledCare;
                        case "multi_screen": return multiScreen;
                        case "hotkey": return hotkey;
                        case "app_tweaker": return appTweaker;
                        case "system": return systemPage;
                        case "user_center": return userCenter;
                        default: return appSettings;
                        }
                    }
                }

                // No monitor attached
                Item {
                    anchors.fill: parent
                    visible: win.noMonitor
                    Rectangle {
                        anchors.fill: parent
                        color: Theme.alpha(Theme.black, 0.88)
                    }
                    Column {
                        anchors.centerIn: parent
                        spacing: 12
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: i18n("No supported monitor found")
                            color: Theme.text
                            font.family: Theme.font
                            font.pixelSize: Theme.fontTitle
                            font.bold: true
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: i18n("Connect the monitor with a DDC/CI-capable cable and enable DDC/CI in its menu.")
                            color: Theme.textDim
                            font.family: Theme.font
                            font.pixelSize: Theme.fontLabel
                        }
                    }
                }
            }

            Component { id: gameVisual; GameVisualPage { store: sumiStore; host: sumiHost } }
            Component { id: gamePlus; GamePlusPage { store: sumiStore; host: sumiHost } }
            Component { id: oledCare; OledCarePage { store: sumiStore; host: sumiHost } }
            Component { id: multiScreen; MultiScreenPage { store: sumiStore; host: sumiHost } }
            Component { id: systemPage; SystemPage { store: sumiStore; host: sumiHost } }
            Component { id: userCenter; UserCenterPage { store: sumiStore; host: sumiHost; backend: sumiBackend } }
            Component { id: appSettings; AppSettingsPage { store: sumiStore; host: sumiHost; backend: sumiBackend; title: i18n("Application Settings") } }
            Component { id: hotkey; HotKeyPage { store: sumiStore; host: sumiHost } }
            Component { id: appTweaker; AppTweakerPage { store: sumiStore; host: sumiHost } }

            // ---- Status toast
            Toast {
                readonly property bool hasError: sumiBackend.status !== "" && !win.offline
                x: Theme.sidebarWidth + Theme.pageMargin
                y: Theme.designHeight - height - 12
                width: Theme.contentWidth
                error: hasError
                text: hasError ? (win.dismissedStatus === sumiBackend.status ? "" : sumiBackend.status)
                    : (sumiBackend.connected && sumiBackend.readonly ? i18n("Read-only daemon: changes are not sent") : "")
                onDismissed: win.dismissedStatus = sumiBackend.status
            }

            // ---- Daemon not running
            Item {
                anchors.fill: parent
                anchors.topMargin: Theme.titleHeight
                visible: win.offline
                Rectangle {
                    anchors.fill: parent
                    color: "#F2030405"
                }
                Column {
                    anchors.centerIn: parent
                    spacing: 16
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: i18n("Sumi can't reach its daemon")
                        color: Theme.text
                        font.family: Theme.font
                        font.pixelSize: Theme.fontTitle
                        font.bold: true
                    }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: i18n("sumid isn't running. Start it with:")
                        color: Theme.textDim
                        font.family: Theme.font
                        font.pixelSize: Theme.fontLabel
                    }
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 420
                        height: 44
                        color: Theme.dialog
                        border.width: 1
                        border.color: Theme.borderButton
                        Text {
                            anchors.centerIn: parent
                            text: i18n("systemctl --user start sumid")
                            color: Theme.hoverEdge
                            font.family: "monospace"
                            font.pixelSize: Theme.fontLabel
                        }
                    }
                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 14
                        RogButton {
                            text: i18n("Copy command")
                            secondary: true
                            onClicked: sumiBackend.copy_text("systemctl --user start sumid")
                        }
                        RogButton {
                            objectName: "retry"
                            text: i18n("Try again")
                            onClicked: sumiBackend.refresh()
                        }
                    }
                }
            }

            // ---- Title bar
            TitleBar {
                x: 0
                y: 0
                onDragStarted: win.startSystemMove()
                onMinimiseRequested: win.showMinimized()
                onCloseRequested: win.close()
            }

            // ---- Confirmation
            ConfirmDialog {
                id: confirmDialog
                anchors.fill: parent
                visible: sumiStore.confirm !== null
                title: sumiStore.confirm ? sumiStore.confirm.title : ""
                text: sumiStore.confirm ? sumiStore.confirm.text : ""
                confirmText: sumiStore.confirm ? sumiStore.confirm.ok : i18n("Confirm")
                cancelText: i18n("Cancel")
                showCancel: sumiStore.confirm ? sumiStore.confirm.showCancel : true
                large: sumiStore.confirm ? sumiStore.confirm.large : false
                onAccepted: sumiStore.answer(true)
                onRejected: sumiStore.answer(false)
            }

            // Layer for drop-down lists (see Overlay.qml)
            Item {
                id: overlayLayer
                anchors.fill: parent
                z: 1000
            }

            // Window outline
            Rectangle {
                anchors.fill: parent
                z: 2000
                color: "transparent"
                border.width: 1
                border.color: Theme.borderWindow
            }
        }
    }

    Component.onDestruction: Overlay.layer = null
    Binding {
        target: Overlay
        property: "layer"
        value: overlayLayer
    }

    // ---- Corner resize grips (true window corners, outside the scaled content)
    component Grip: MouseArea {
        property int edges: 0
        width: 10
        height: 10
        hoverEnabled: true
        cursorShape: (edges === (Qt.TopEdge | Qt.LeftEdge) || edges === (Qt.BottomEdge | Qt.RightEdge)) ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor
        onPressed: {
            win.lastW = win.width;
            win.lastH = win.height;
            win.userResizing = true;
            win.startSystemResize(edges);
        }
    }
    Grip { edges: Qt.TopEdge | Qt.LeftEdge; anchors.top: parent.top; anchors.left: parent.left }
    Grip { edges: Qt.TopEdge | Qt.RightEdge; anchors.top: parent.top; anchors.right: parent.right }
    Grip { edges: Qt.BottomEdge | Qt.LeftEdge; anchors.bottom: parent.bottom; anchors.left: parent.left }
    Grip { edges: Qt.BottomEdge | Qt.RightEdge; anchors.bottom: parent.bottom; anchors.right: parent.right }

    // ---- Headless UI check:
    // SUMI_SCREENSHOT=/path.png renders 3 s after the first state arrives, saves and quits.
    function maybeShoot() {
        if (!shooting || shotTimer.running) {
            return;
        }
        const ready = pageInfo.key === "" ? refreshed : sumiStore.isLoaded(pageInfo.key);
        if (ready) {
            shotTimer.start();
            // SUMI_SCROLL=end: show the bottom of a scrolling page.
            if (sumiBackend.env_var("SUMI_SCROLL") === "end") {
                scrollTimer.start();
            }
        }
    }
    TextCheck {
        id: textCheck
    }
    function grab() {
        if (checkText) {
            textCheck.run(scaler, win.page + (sumiBackend.env_var("SUMI_TAB") !== "" ? "/" + sumiBackend.env_var("SUMI_TAB") : ""));
            if (screenshotPath === "") {
                Qt.quit();
                return;
            }
        }
        const ok = viewport.grabToImage(result => {
            result.saveToFile(screenshotPath);
            Qt.quit();
        });
        if (!ok) {
            console.warn("screenshot: grabToImage failed");
            Qt.quit();
        }
    }
    Timer {
        id: scrollTimer
        interval: 1500
        onTriggered: {
            const item = pageLoader.item;
            const f = item ? item.flick : null;
            if (f) {
                f.contentY = Math.max(0, f.contentHeight - f.height);
            } else if (item && item.scrollEnd) {
                item.scrollEnd();   // the GameVisual mode strip
            }
        }
    }
    Timer {
        id: shotTimer
        interval: 3000
        onTriggered: win.grab()
    }
    Timer {
        // Safety net: nothing ever loaded (daemon missing, monitor asleep).
        interval: 40000
        running: win.shooting
        onTriggered: win.grab()
    }
    Connections {
        target: sumiStore
        function onLoadedChanged() {
            win.maybeShoot();
        }
    }
    onRefreshedChanged: maybeShoot()
}
