set -euo pipefail

BIN_NAME="texp"
PREFIX="${PREFIX:-$HOME/.local}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "build release bin"
cargo build --release

echo "install bin with cargo install"
cargo install --path . --force

echo "find path to installed bin"
EXEC_PATH="$(command -v "$BIN_NAME" || true)"
if [ -z "$EXEC_PATH" ]; then
    EXEC_PATH="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin/$BIN_NAME"
fi
echo "    -> $EXEC_PATH"

echo "generating .desktop file with absolute path to bin file"
mkdir -p "$PREFIX/share/applications" "$PREFIX/share/icons/hicolor/256x256/apps"

sed "s|^Exec=.*|Exec=$EXEC_PATH %f|" "assets/$BIN_NAME.desktop" > "/tmp/$BIN_NAME.desktop"
install -Dm644 "/tmp/$BIN_NAME.desktop" "$PREFIX/share/applications/$BIN_NAME.desktop"
rm "/tmp/$BIN_NAME.desktop"

echo "==> Устанавливаю иконку"
install -Dm644 "assets/$BIN_NAME.svg" "$PREFIX/share/icons/hicolor/scalable/apps/$BIN_NAME.svg"
echo "update cache"
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache "$PREFIX/share/icons/hicolor" || true

echo "Ready. $BIN_NAME installed"
