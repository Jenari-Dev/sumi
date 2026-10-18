#!/bin/sh
# Headless QML tests: copies the app's QML next to the tests (singletons via qmldir) and runs
# them with Qt 6's qmltestrunner on the offscreen platform. The app's i18n() calls are
# redirected to the Tr stand-in (the real functions only exist inside the app's engine).
set -e
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "$here"/../../qml/*.qml "$here"/tst_*.qml "$here"/Tr.qml "$here"/NoLayerShell.qml "$work"/
cp "$here"/../../../../fixtures/state-pg32ucdm3.json "$work"/   # a real daemon state
rm -f "$work/Main.qml"   # needs the compiled module (Backend); everything else is plain QML
for f in "$work"/*.qml; do
    case "$f" in */Tr.qml) continue ;; esac
    sed -E -i 's/(^|[^A-Za-z0-9_.])(i18n(c|p|cp)?)\(/\1Tr.\2(/g' "$f"
done
printf 'singleton Theme 1.0 Theme.qml\nsingleton Overlay 1.0 Overlay.qml\nsingleton IconData 1.0 IconData.qml\nsingleton Languages 1.0 Languages.qml\nsingleton Tr 1.0 Tr.qml\n' > "$work/qmldir"
cd "$work"
QML_XHR_ALLOW_FILE_READ=1 QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -input . "$@"
