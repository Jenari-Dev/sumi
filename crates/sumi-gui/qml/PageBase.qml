import QtQuick

// Frame shared by the pages: 24 px bold title at (31, 20), a spinner while the
// page's first state is loading, and an error message with a retry button.
Item {
    id: root

    property var store: null
    property var host: null
    property string title: ""
    // Pages the daemon must read for this page (csv); "" = needs no monitor state.
    property string stateKey: ""
    // Set while something other than the monitor state is still loading (for example the app list).
    property bool waiting: false
    // The page's scrolling area, if it has one (SUMI_SCROLL=end scrolls it for screenshots).
    property Flickable flick: null
    default property alias content: body.data

    readonly property bool ready: !waiting && (stateKey === "" || (store !== null && store.isLoaded(stateKey)))
    readonly property bool failedLoad: !ready && stateKey !== "" && store !== null && store.failed[stateKey] === true

    width: Theme.pageWidth
    height: Theme.pageHeight

    Text {
        x: 31
        y: 20
        text: root.title
        color: Theme.text
        font.family: Theme.font
        font.pixelSize: Theme.fontTitle
        font.bold: true
    }

    Item {
        id: body
        anchors.fill: parent
        visible: root.ready
    }

    Spinner {
        objectName: "spinner"
        anchors.centerIn: parent
        visible: !root.ready && !root.failedLoad
    }

    Column {
        anchors.centerIn: parent
        spacing: 16
        visible: root.failedLoad
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: i18n("The monitor didn't answer.")
            color: Theme.text
            font.family: Theme.font
            font.pixelSize: Theme.fontLabel
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: i18n("Check that DDC/CI is enabled in the monitor's OSD, then try again.")
            color: Theme.textDim
            font.family: Theme.font
            font.pixelSize: Theme.fontBody
        }
        RogButton {
            anchors.horizontalCenter: parent.horizontalCenter
            text: i18n("Try again")
            onClicked: root.store.request(root.stateKey)
        }
    }
}
