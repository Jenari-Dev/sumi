#!/bin/bash
# Sumi under a tiling script, on a throwaway virtual KWin (nothing touches the real desktop):
# loads Krohnkite (from your install, or KROHNKITE_DIR) plus a small watcher script, opens a
# plain window and sumi-app next to it, and counts how often KWin moves/resizes the Sumi
# window once the layout has settled. A window that fights the tiler (re-snaps its own size)
# shows up as a stream of geometry changes, i.e. flicker.
#
#   cargo build -p sumi-gui -p sumid
#   tests/kwin-krohnkite.sh
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
app="$root/target/debug/sumi-app"
sumid="$root/target/debug/sumid"
kk=${KROHNKITE_DIR:-$HOME/.local/share/kwin/scripts/krohnkite}
hs=${HEADLESS_SESSION:-$HOME/dev/oss-ports/tools/headless-session.sh}
[ -d "$kk" ] || { echo "skip: Krohnkite not found (set KROHNKITE_DIR)"; exit 0; }
if [ -z "${SUMI_TEST_INNER:-}" ]; then
    if [ -x "$hs" ]; then
        exec env SUMI_TEST_INNER=1 "$hs" "$0" "$@"
    fi
    exec env SUMI_TEST_INNER=1 dbus-run-session -- "$0" "$@"
fi
work=$(mktemp -d)
run=$(mktemp -d /tmp/sumi-kwk.XXXXXX)   # short: Wayland socket paths are limited to 108 bytes
pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
    sleep 0.3
    for p in "${pids[@]}"; do kill -9 "$p" 2>/dev/null || true; done
    cp "$work"/*.log "$logs"/ 2>/dev/null || true
    rm -rf "$work" "$run"
}
trap cleanup EXIT
logs="$root/target/kwin-krohnkite"
mkdir -p "$logs"
export XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache" XDG_RUNTIME_DIR="$run"
scripts="$XDG_DATA_HOME/kwin/scripts"
mkdir -p "$XDG_CONFIG_HOME" "$scripts/sumiwatch/contents/code"
cp -r "$kk" "$scripts/krohnkite"
cat > "$scripts/sumiwatch/metadata.json" <<'JSON'
{"KPackageStructure": "KWin/Script", "X-Plasma-API": "javascript",
 "KPlugin": {"Id": "sumiwatch", "Name": "sumiwatch", "Version": "1"}}
JSON
cat > "$scripts/sumiwatch/contents/code/main.js" <<'JS'
function watch(w) {
    if (w.resourceClass.indexOf("sumi") < 0) return;
    print("SUMIWATCH add " + w.frameGeometry.width + "x" + w.frameGeometry.height);
    w.frameGeometryChanged.connect(function () {
        print("SUMIWATCH geom " + Date.now() + " " + w.frameGeometry.x + "," + w.frameGeometry.y + " "
              + w.frameGeometry.width + "x" + w.frameGeometry.height);
    });
}
workspace.windowList().forEach(watch);
workspace.windowAdded.connect(watch);
JS
cat > "$XDG_CONFIG_HOME/kwinrc" <<'INI'
[Plugins]
krohnkiteEnabled=true
sumiwatchEnabled=true
INI
unset WAYLAND_DISPLAY DISPLAY
"$sumid" --read-only --no-monitors > "$work/sumid.log" 2>&1 &
pids+=($!)
QT_FORCE_STDERR_LOGGING=1 QT_LOGGING_RULES="kwin_scripting.debug=true;js.debug=true;qml.debug=true" kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --width 1920 --height 1080 --socket kt > "$work/kwin.log" 2>&1 &
pids+=($!)
for _ in $(seq 50); do [ -S "$run/kt" ] && break; sleep 0.1; done
cat > "$work/other.qml" <<'QML'
import QtQuick
import QtQuick.Window
Window { visible: true; width: 800; height: 600; color: "#334455"; title: "other" }
QML
WAYLAND_DISPLAY=kt QT_QPA_PLATFORM=wayland qml6 "$work/other.qml" > "$work/other.log" 2>&1 &
pids+=($!)
sleep 2
WAYLAND_DISPLAY=kt QT_QPA_PLATFORM=wayland SUMI_NO_TRAY=1 QT_FORCE_STDERR_LOGGING=1 "$app" > "$work/app.log" 2>&1 &
pids+=($!)
sleep 6                                   # open + let the tiler settle
mark=$(date +%s%3N)
sleep 8                                   # the measured window
grep SUMIWATCH "$work/kwin.log" | tail -n 5 || true
grep -q 'SUMIWATCH add' "$work/kwin.log" || { echo "FAIL: Sumi window never appeared"; echo "logs in $logs"; exit 1; }
grep -qi krohnkite "$work/kwin.log" && grep -i 'krohnkite.*error' "$work/kwin.log" | head -3 || true
late=$(awk -v m="$mark" '$1 ~ /SUMIWATCH/ || /SUMIWATCH geom/ { for (i=1;i<=NF;i++) if ($i=="geom") { if ($(i+1) >= m) n++ } } END { print n+0 }' "$work/kwin.log")
final=$(grep 'SUMIWATCH geom' "$work/kwin.log" | tail -n1 | awk '{print $NF}')
echo "Sumi window: $final, geometry changes in the 8 s after settling: $late"
if [ "$late" -gt 2 ]; then echo "FAIL: Sumi keeps resizing under Krohnkite (flicker)"; exit 1; fi
echo "ok: stable under Krohnkite"
