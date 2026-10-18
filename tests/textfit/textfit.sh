#!/bin/bash
# Do the app's texts fit in every language? Runs every page, tab and dialog of the real QML (a fake
# daemon behind it, the translations of po/<lang>.po behind i18n) through SUMI_CHECK_TEXT's checker
# (qml/TextCheck.qml) and prints what it finds; nothing printed = everything fits. Touches no
# monitor, daemon or desktop (offscreen).
#
#   tests/textfit/textfit.sh [lang ...]        (default: English and every po/*.po; 5 at a time)
#   TOUR_ONLY=game_visual SCENARIO=rich_hdr tests/textfit/textfit.sh ja
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
scenario=${SCENARIO:-rich}
langs=("$@")
[ ${#langs[@]} -eq 0 ] && langs=(en $(cd "$root/po" && ls *.po | sed 's/\.po$//'))
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
one() {   # one <lang>: the whole tour in one language
    local l=$1 d
    d=$(mktemp -d -p "$out")
    python3 "$here/make.py" "$d" "$scenario" game_visual "" "$here/tour.qml" --env SUMI_TOUR=1 --env "SUMI_TOUR_ONLY=${TOUR_ONLY:-}" $([ "$l" != en ] && echo --lang "$l") > "$d.make" 2>&1 || { cat "$d.make"; return 1; }
    (cd "$d" && QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen timeout 300 /usr/lib/qt6/bin/qml Main.qml > "$d.log" 2>&1)
    { grep -E "TEXTFIT|TOUR timeout|ReferenceError|TypeError|Type .* unavailable" "$d.log" | sed "s/^qml: /[$l] /"; grep -q "TOUR end" "$d.log" || echo "[$l] TOUR did not finish"; } > "$out/$l.result"
}
export -f one; export here root scenario out
printf '%s\n' "${langs[@]}" | xargs -P 5 -I{} bash -c 'one {}'
cat "$out"/*.result
