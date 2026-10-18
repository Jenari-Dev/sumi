#!/bin/sh
# Build Sumi and install it for the current user (~/.local + systemd --user).
# Re-run after pulling changes; it restarts sumid if it is running.
set -eu
cd "$(dirname "$0")/.."
cargo build --release
bin="$HOME/.local/bin"
install -Dm755 target/release/sumi target/release/sumid target/release/sumi-app -t "$bin"
install -Dm644 contrib/sumid.service "$HOME/.config/systemd/user/sumid.service"
# Full path: Plasma's session PATH usually lacks ~/.local/bin.
mkdir -p "$HOME/.local/share/applications"
# Translated name/comment/keywords merged in from po/.
desktop=$(mktemp)
msgfmt --desktop --template=contrib/io.github.jenaridev.sumi.desktop -d po -o "$desktop"
sed "s|^Exec=sumi-app|Exec=$bin/sumi-app|" "$desktop" > "$HOME/.local/share/applications/io.github.jenaridev.sumi.desktop"
rm -f "$desktop"
install -Dm644 contrib/io.github.jenaridev.sumi.svg -t "$HOME/.local/share/icons/hicolor/scalable/apps"
# Hidden entry naming sumid: KWin only gives screenshots (AI Visual) to listed executables.
sed "s|@BINDIR@|$bin|" contrib/io.github.jenaridev.sumid.desktop.in > "$HOME/.local/share/applications/io.github.jenaridev.sumid.desktop"
kbuildsycoca6 > /dev/null 2>&1 || true
# Translations (app and daemon share the "sumi" catalogue).
for po in po/*.po; do
    [ -e "$po" ] || continue
    lang=$(basename "$po" .po)
    mkdir -p "$HOME/.local/share/locale/$lang/LC_MESSAGES"
    msgfmt -o "$HOME/.local/share/locale/$lang/LC_MESSAGES/sumi.mo" "$po"
done
# The user manual (Application Settings › User manual).
install -Dm644 README.md "$HOME/.local/share/doc/sumi/README.md"
systemctl --user daemon-reload
if systemctl --user is-active --quiet sumid; then
    systemctl --user restart sumid
    echo "sumid restarted."
else
    echo "Installed. Start the daemon with: systemctl --user enable --now sumid"
fi
