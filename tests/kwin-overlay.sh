#!/bin/bash
# AI Visual bubble on a throwaway virtual KWin (nothing touches the real desktop or monitor):
# sumi-app runs with its main window minimised, a full-screen test window below it, and canned
# AiStatus data (SUMI_AI_FAKE) instead of the daemon's. The script checks, from the Wayland
# protocol trace, from KWin ScreenShot2 captures and from real (fake-input) pointer events, that
# the orb is a layer-shell overlay surface at the bottom-right of the output, above the
# full-screen window; that a mode switch shows the message (and removes it again after 4 s);
# that clicks go through the message but not through the orb; that dragging the orb moves it and
# the new place is saved and restored after a restart; that a double-click hides it until the
# next switch; and that a paused monitor dims it.
#
#   cargo build -p sumi-gui -p sumid && cargo build -p sumi-ai --example capture-png
#   tests/kwin-overlay.sh [output-dir]      (default: a temporary directory, PNGs kept in output-dir)
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
app="$root/target/debug/sumi-app"
sumid="$root/target/debug/sumid"
grab="$root/target/debug/examples/capture-png"
check="$root/tests/kwin-overlay-check.py"
hs=${HEADLESS_SESSION:-$HOME/dev/oss-ports/tools/headless-session.sh}
if [ -z "${SUMI_TEST_INNER:-}" ]; then
    if [ -x "$hs" ]; then
        exec env SUMI_TEST_INNER=1 "$hs" "$0" "$@"
    fi
    exec env SUMI_TEST_INNER=1 dbus-run-session -- "$0" "$@"
fi
out=${1:-$(cd "$(dirname "$0")/.." && pwd)/target/kwin-overlay}
work=$(mktemp -d)
run=$(mktemp -d /tmp/sumi-kwo.XXXXXX)   # short: Wayland socket paths are limited to 108 bytes
pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    sleep 0.3
    for p in "${pids[@]}"; do kill -9 "$p" 2>/dev/null || true; done
    rm -rf "$work" "$run"
}
trap cleanup EXIT
mkdir -p "$out"
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" XDG_RUNTIME_DIR="$run"
mkdir -p "$XDG_DATA_HOME/applications"
# Pointer input: a tiny fake-input client, built here, authorised like the capture tool.
wayland-scanner client-header "$root/tests/fake-input.xml" "$work/fake-input-client-protocol.h"
wayland-scanner private-code "$root/tests/fake-input.xml" "$work/fake-input-protocol.c"
cc -O1 -Wall -I"$work" -o "$work/kwin-input" "$root/tests/kwin-input.c" "$work/fake-input-protocol.c" -lwayland-client -lm
cat > "$XDG_DATA_HOME/applications/sumi-kwin-input.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Sumi test input
Exec=$work/kwin-input
NoDisplay=true
X-KDE-Wayland-Interfaces=org_kde_kwin_fake_input
DESKTOP
cat > "$XDG_DATA_HOME/applications/sumi-capture-png.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Sumi capture
Exec=$grab
NoDisplay=true
X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2
DESKTOP
kbuildsycoca6 --noincremental > /dev/null 2>&1 || true
unset WAYLAND_DISPLAY DISPLAY

# A read-only daemon with no monitors, so the app has something to talk to.
"$sumid" --read-only --no-monitors > "$work/sumid.log" 2>&1 &
pids+=($!)
kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --width 1920 --height 1080 --socket kt > "$work/kwin.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do [ -S "$run/kt" ] && break; sleep 0.1; done

# The "game": a full-screen window of one flat colour. The overlay has to float above it.
cat > "$work/game.qml" <<'QML'
import QtQuick
import QtQuick.Window
Window {
    visible: true; visibility: Window.FullScreen; color: "#1c2a3a"
    MouseArea { anchors.fill: parent; onPressed: mouse => console.log("GAMECLICK " + Math.round(mouse.x) + " " + Math.round(mouse.y)) }
}
QML
WAYLAND_DISPLAY=kt QT_QPA_PLATFORM=wayland QT_FORCE_STDERR_LOGGING=1 qml6 "$work/game.qml" > "$work/game.log" 2>&1 &
pids+=($!)

# Canned AiStatus: one monitor on Virtual-0 whose last mode is FPS (the baseline: no message).
ai="$work/ai.json"
status() {   # status <last_sent> <paused or empty>
    local paused=""
    [ -n "$2" ] && paused=", \"paused\": \"$2\""
    cat > "$ai.new" <<JSON
{"config": {"enabled": true, "notify": true},
 "monitors": [{"id": "FAKE@i2c-9", "connector": "Virtual-0"}],
 "status": {"installed": true, "prompts": true, "model": {"name": "vision_model.onnx"},
  "state": {"running": true, "monitors": {"FAKE@i2c-9": {
   "verdict": {"scores": [["$1", 61.2], ["office", 22.5]], "is_game": 0.8, "confidence": 0.7, "top": "$1", "target": "$1"},
   "last_sent": "$1"$paused}}}}}
JSON
    mv "$ai.new" "$ai"
}
status fps ""

# The app: --hidden without a tray starts minimised, i.e. the "main window in the tray" case.
start_app() {
    WAYLAND_DISPLAY=kt QT_QPA_PLATFORM=wayland WAYLAND_DEBUG=client QT_FORCE_STDERR_LOGGING=1 \
        SUMI_NO_TRAY=1 SUMI_AI_FAKE="$ai" "$app" --hidden >> "$work/app.log" 2>&1 &
    app_pid=$!
    pids+=($app_pid)
}
shot() {   # shot <name>
    WAYLAND_DISPLAY=kt "$grab" Virtual-0 "$out/overlay-$1.png" > /dev/null
}
pointer() {
    WAYLAND_DISPLAY=kt "$work/kwin-input" "$@"
}
start_app
sleep 6
pointer move 800 500 click     # control: the window below does get clicks where nothing is in front
shot idle                      # baseline: orb only, no message
status racing ""               # AI Visual switched to Racing
sleep 1.2
shot switched                  # orb with its red pulse, message to its left
pointer move 1680 936 click wait 200 move 1840 936 click   # on the message, then on the orb
sleep 5
shot after                     # message gone again
# Drag the orb from its centre (1840, 936) up and left by (140, 136).
pointer move 1840 936 wait 100 press wait 100 move 1810 910 wait 80 move 1770 870 wait 80 move 1720 820 wait 80 move 1700 800 wait 200 release
sleep 1
shot dragged
# Double-click it: hidden until the next switch.
pointer move 1700 800 wait 100 dblclick
sleep 1
shot hidden
status fps ""
sleep 1.5
shot returned                  # the switch brought it (and a message) back
# A restart finds the orb where it was left.
kill "$app_pid"; sleep 1
status fps ""
start_app
sleep 6
shot restored
status fps "HDR is on"
sleep 2
shot paused                    # dimmed orb, no message
sleep 0.5
cp "$work/game.log" "$out/game.log"
cp "$XDG_CONFIG_HOME/sumi/app.json" "$out/app.json" 2>/dev/null || echo '{}' > "$out/app.json"
python3 -I "$check" "$work/app.log" "$out" 1920 1080
echo "screenshots in $out"
