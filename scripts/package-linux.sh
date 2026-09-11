#!/usr/bin/env bash
# Package Pitype into a self-contained Linux tarball for releases.
# Usage: scripts/package-linux.sh [target-triple]
set -euo pipefail

VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
ARCH="${ARCH:-x86_64}"
TARGET="${1:-${ARCH}-unknown-linux-gnu}"
NAME="pitype"
# target-dir may be overridden (e.g. shared cache in ~/.cargo/config.toml),
# so ask cargo where the build landed instead of assuming target/.
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
TARGET_DIR="${TARGET_DIR:-target}"
OUT_DIR="${TARGET_DIR}/package"
STAGE="${OUT_DIR}/${NAME}-${VERSION}-${TARGET}"

cd "$(dirname "$0")/.."
rm -rf "$STAGE"
mkdir -p "$STAGE"

cargo build --release --locked --target "$TARGET"

install -Dm755 "${TARGET_DIR}/${TARGET}/release/${NAME}" "$STAGE/${NAME}"
install -Dm644 "dist/${NAME}.desktop" "$STAGE/${NAME}.desktop"
install -Dm644 "dist/${NAME}.svg" "$STAGE/${NAME}.svg"
install -Dm644 "LICENSE" "$STAGE/LICENSE"
install -Dm644 "fonts/OFL.txt" "$STAGE/OFL.txt"

cat > "$STAGE/install.sh" << 'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-$HOME/.local}"
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
install -Dm755 "$HERE/pitype" "$PREFIX/bin/pitype"
install -Dm644 "$HERE/pitype.desktop" "$PREFIX/share/applications/pitype.desktop"
install -Dm644 "$HERE/LICENSE" "$PREFIX/share/licenses/pitype/LICENSE"
install -Dm644 "$HERE/OFL.txt" "$PREFIX/share/licenses/pitype/OFL.txt"
if command -v rsvg-convert >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"; rsvg-convert -w 128 -h 128 "$HERE/pitype.svg" -o "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pitype.png"; rm -f "$tmp"
fi
install -Dm644 "$HERE/pitype.svg" "$PREFIX/share/icons/hicolor/scalable/apps/pitype.svg"
update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
echo "Installed pitype into $PREFIX"
INSTALL
chmod +x "$STAGE/install.sh"

tar -czf "${OUT_DIR}/${NAME}-${VERSION}-${TARGET}.tar.gz" -C "$OUT_DIR" "${NAME}-${VERSION}-${TARGET}"
echo "Packaged ${OUT_DIR}/${NAME}-${VERSION}-${TARGET}.tar.gz"