#!/bin/sh
# Offscreen screenshots of the AI bubble's visuals into <dir> (default: ./shots).
#   tests/qml/shots.sh [dir]
set -e
here=$(cd "$(dirname "$0")" && pwd)
out=$(mkdir -p "${1:-shots}" && cd "${1:-shots}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "$here"/../../qml/*.qml "$here"/Tr.qml "$work"/
cp "$here"/shot_ai_bubble.qml "$work"/tst_shot.qml
rm -f "$work/Main.qml"
sed -i "s|@SHOTDIR@|$out|g" "$work/tst_shot.qml"
for f in "$work"/*.qml; do
    case "$f" in */Tr.qml) continue ;; esac
    sed -E -i 's/(^|[^A-Za-z0-9_.])(i18n(c|p|cp)?)\(/\1Tr.\2(/g' "$f"
done
printf 'singleton Theme 1.0 Theme.qml\nsingleton Overlay 1.0 Overlay.qml\nsingleton IconData 1.0 IconData.qml\nsingleton Languages 1.0 Languages.qml\nsingleton Tr 1.0 Tr.qml\n' > "$work/qmldir"
cd "$work"
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input tst_shot.qml
python3 -I - "$out" <<'PY'
import sys
from PIL import Image
out = sys.argv[1]
im = Image.open(f"{out}/ai_orb_overview.png").convert("RGB")
im.crop((12, 22, 108, 118)).resize((288, 288), Image.LANCZOS).save(f"{out}/ai_orb_idle.png")
im.crop((120, 22, 216, 118)).resize((288, 288), Image.LANCZOS).save(f"{out}/ai_orb_paused.png")
im.crop((230, 10, 540, 130)).resize((620, 240), Image.LANCZOS).save(f"{out}/ai_orb_switched.png")
PY
