#!/bin/bash
# MultiFrame end-to-end test against a throwaway virtual KWin: no real desktop,
# config or monitor is touched. Builds each layout through sumid's D-Bus API
# and checks KWin's read-back.
#
#   tests/kwin-tiling.sh            (from the workspace root, after cargo build)
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/target/debug"
if [ -z "${SUMI_TEST_INNER:-}" ]; then
    exec env SUMI_TEST_INNER=1 dbus-run-session -- "$0" "$@"
fi
work=$(mktemp -d)
run=$(mktemp -d /tmp/sumi-kwt.XXXXXX)   # short: Wayland socket paths max 108 bytes
bus="$DBUS_SESSION_BUS_ADDRESS"
kwin=""
cleanup() {
    # kwin_wayland has cap_sys_nice, so its environ is unreadable: kill it by pid.
    [ -n "$kwin" ] && kill "$kwin" 2>/dev/null
    # Stop everything on this private bus (KWin, sumid, activated helpers).
    for p in $(pgrep -u "$(id -u)"); do
        [ "$p" = "$$" ] && continue
        a=$( { tr '\0' '\n' < "/proc/$p/environ"; } 2>/dev/null | grep -m1 '^DBUS_SESSION_BUS_ADDRESS=' | cut -d= -f2-)
        [ "$a" = "$bus" ] && kill "$p" 2>/dev/null
    done
    rm -rf "$work" "$run"
}
trap cleanup EXIT
export XDG_CONFIG_HOME="$work" XDG_RUNTIME_DIR="$run"
unset WAYLAND_DISPLAY DISPLAY
kwin_wayland --virtual --no-lockscreen --no-global-shortcuts --width 3840 --height 2160 --socket kt > "$work/kwin.log" 2>&1 &
kwin=$!
for _ in $(seq 50); do [ -S "$run/kt" ] && break; sleep 0.1; done
export WAYLAND_DISPLAY=kt
"$bin/sumid" --no-monitors --poll 0 --config "$work/sumi.toml" > "$work/sumid.log" 2>&1 &
for _ in $(seq 50); do busctl --user status io.github.JenAriDev.Sumi > /dev/null 2>&1 && break; sleep 0.1; done
call() { busctl --user call io.github.JenAriDev.Sumi /io/github/JenAriDev/Sumi io.github.JenAriDev.Sumi1 "$@"; }
fail=0
for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 0; do
    if call SetTileLayout sy Virtual-0 "$n" > /dev/null; then
        echo "ok   layout $n"
    else
        echo "FAIL layout $n"; fail=1
    fi
done
[ $fail = 0 ] && echo "all layouts verified by KWin" || { cat "$work/sumid.log"; exit 1; }
