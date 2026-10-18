#!/bin/bash
# AI Visual capture test against a throwaway virtual KWin: a full-screen test
# window shows a known colour; the probe (authorised by a desktop file, the
# way sumid is) must read it back through KWin ScreenShot2.
#
#   tests/kwin-capture.sh            (after cargo build -p sumi-ai --example capture-probe)
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
probe="$root/target/debug/examples/capture-probe"
if [ -z "${SUMI_TEST_INNER:-}" ]; then
    exec env SUMI_TEST_INNER=1 dbus-run-session -- "$0" "$@"
fi
work=$(mktemp -d)
run=$(mktemp -d /tmp/sumi-kwt.XXXXXX)
bus="$DBUS_SESSION_BUS_ADDRESS"
kwin=""
cleanup() {
    # kwin_wayland has cap_sys_nice, so its environ is unreadable: kill it by pid.
    [ -n "$kwin" ] && kill "$kwin" 2>/dev/null
    for p in $(pgrep -u "$(id -u)"); do
        [ "$p" = "$$" ] && continue
        a=$( { tr '\0' '\n' < "/proc/$p/environ"; } 2>/dev/null | grep -m1 '^DBUS_SESSION_BUS_ADDRESS=' | cut -d= -f2-)
        [ "$a" = "$bus" ] && kill "$p" 2>/dev/null
    done
    rm -rf "$work" "$run"
}
trap cleanup EXIT
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" XDG_RUNTIME_DIR="$run"
mkdir -p "$XDG_DATA_HOME/applications"
cat > "$XDG_DATA_HOME/applications/sumi-capture-probe.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Sumi capture probe
Exec=$probe
NoDisplay=true
X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2
DESKTOP
kbuildsycoca6 --noincremental > /dev/null 2>&1 || true
unset WAYLAND_DISPLAY DISPLAY
kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --width 1920 --height 1080 --socket kt > "$work/kwin.log" 2>&1 &
kwin=$!
for _ in $(seq 50); do [ -S "$run/kt" ] && break; sleep 0.1; done
cat > "$work/red.qml" <<'QML'
import QtQuick
import QtQuick.Window
Window { visible: true; visibility: Window.FullScreen; color: "#ff2000" }
QML
WAYLAND_DISPLAY=kt QT_QPA_PLATFORM=wayland qml6 "$work/red.qml" > "$work/qml.log" 2>&1 &
sleep 3
WAYLAND_DISPLAY=kt "$probe" Virtual-0
