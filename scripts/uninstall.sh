#!/usr/bin/env bash
# Uninstall Pitype from ~/.local.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"

rm -f "$PREFIX/bin/pitype"
rm -f "$PREFIX/share/applications/pitype.desktop"
rm -f "$PREFIX/share/icons/hicolor/scalable/apps/pitype.svg"
rm -f "$PREFIX/share/icons/hicolor/128x128/apps/pitype.png"
rm -rf "$PREFIX/share/licenses/pitype"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Removed pitype from $PREFIX"
echo "(Session history in ~/.local/share/pitype was kept.)"