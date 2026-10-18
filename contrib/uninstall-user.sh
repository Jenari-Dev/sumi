#!/bin/sh
# Remove what contrib/install-user.sh installed for the current user.
#   contrib/uninstall-user.sh           keep settings, the cache and the AI model
#   contrib/uninstall-user.sh --purge   also delete them and Sumi's KDE shortcuts
set -eu
purge=false
[ "${1:-}" = "--purge" ] && purge=true
# Stopping sumid also unloads its KWin scripts.
systemctl --user disable --now sumid 2>/dev/null || true
pkill -x sumi-app 2>/dev/null || true
rm -f "$HOME/.local/bin/sumi" "$HOME/.local/bin/sumid" "$HOME/.local/bin/sumi-app"
rm -f "$HOME/.config/systemd/user/sumid.service"
systemctl --user daemon-reload
rm -f "$HOME/.local/share/applications/io.github.jenaridev.sumi.desktop" \
      "$HOME/.local/share/applications/io.github.jenaridev.sumid.desktop" \
      "$HOME/.local/share/icons/hicolor/scalable/apps/io.github.jenaridev.sumi.svg" \
      "$HOME/.config/autostart/io.github.jenaridev.sumi.desktop"
rm -f "$HOME"/.local/share/locale/*/LC_MESSAGES/sumi.mo
rm -rf "$HOME/.local/share/doc/sumi"
kbuildsycoca6 > /dev/null 2>&1 || true
if $purge; then
    # Forget Sumi's entries in System Settings › Shortcuts.
    if command -v qdbus6 > /dev/null; then
        for action in $(qdbus6 org.kde.kglobalaccel /component/sumi org.kde.kglobalaccel.Component.shortcutNames 2>/dev/null); do
            qdbus6 org.kde.kglobalaccel /kglobalaccel org.kde.KGlobalAccel.unregister sumi "$action" > /dev/null 2>&1 || true
        done
    fi
    rm -rf "$HOME/.config/sumi" "$HOME/.cache/sumi" "$HOME/.local/share/sumi"
    echo "Sumi removed, with its settings, cache, AI model and shortcuts."
else
    echo "Sumi removed. Settings (~/.config/sumi), cache and the AI model (~/.local/share/sumi) were kept; --purge deletes them."
fi
