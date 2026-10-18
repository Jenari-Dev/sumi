import QtQuick
import QtTest

// Photographs the AI bubble's visuals offscreen (run through shots.sh, which fills in the paths):
// the orb idle, the orb paused, and orb + message right after a switch.
Item {
    id: root
    width: 540
    height: 140

    Rectangle {
        anchors.fill: parent
        color: "#1c2a3a"
    }

    AiOrb {
        id: idle
        x: 12
        y: 22
        animated: false
        ringAngle: 215
    }
    AiOrb {
        id: paused
        x: 120
        y: 22
        animated: false
        ringAngle: 215
        paused: true
    }
    Row {
        x: 230
        y: 22
        spacing: 8
        AiBubbleText {
            id: message
            anchors.verticalCenter: orb.verticalCenter
            holdMs: 600000
            fadeInMs: 1
        }
        AiOrb {
            id: orb
            animated: false
            ringAngle: 215
        }
    }

    TestCase {
        name: "AiBubbleShots"
        when: windowShown

        function test_shots() {
            orb.glow = 1;
            message.flash(Tr.i18n("Switched to %1 mode", "FPS"));
            wait(400);
            const all = grabImage(root);
            all.save("@SHOTDIR@/ai_orb_overview.png");
        }
    }
}
