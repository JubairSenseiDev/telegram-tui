#!/usr/bin/env bash
# One-line installer for telegram-tui.
#   curl -fsSL https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh | sh
set -euo pipefail

REPO="JubairSenseiDev/telegram-tui"
BASE="https://github.com/$REPO/releases/latest/download"

# --- detect os / arch -------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)
    if [ "${PREFIX:-}" != "" ] && [ -d "${PREFIX}/bin" ] && uname -o 2>/dev/null | grep -qi android; then
      os="termux"
    fi
    ;;
esac

case "$os:$arch" in
  termux:*)                     suffix="aarch64-termux" ;;
  termux:aarch64)               suffix="aarch64-termux" ;;
  Linux:x86_64)                 suffix="x86_64-linux" ;;
  Linux:aarch64 | Linux:arm64)  suffix="aarch64-linux" ;;
  Linux:armv7l | Linux:armhf)   suffix="armv7-linux" ;;
  Darwin:arm64)                 suffix="aarch64-macos" ;;
  MINGW* | MSYS* | CYGWIN*)     suffix="x86_64-windows.exe" ;;
  *)
    echo "error: unsupported platform: $os / $arch" >&2
    exit 1
    ;;
esac

# --- pick install dir -------------------------------------------------------
if [ "$suffix" = "x86_64-windows.exe" ]; then
  DIR="${USERPROFILE:-$HOME}/.local/bin"
elif [ -n "${TERMUX_VERSION:-}" ] || [ "$os" = "termux" ]; then
  DIR="${PREFIX:-/data/data/com.termux/files/usr}/bin"
elif [ -w /usr/local/bin ]; then
  DIR="/usr/local/bin"
else
  DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
fi

BIN="$DIR/telegram-tui"
URL="$BASE/telegram-tui-$suffix"

echo "==> downloading telegram-tui ($suffix)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "$URL" -o "$TMP/telegram-tui"

# --- verify sha256 if published ---------------------------------------------
if curl -fsSL "$URL.sha256" -o "$TMP/telegram-tui.sha256" 2>/dev/null; then
  expected="$(awk 'NR==1{print $1}' "$TMP/telegram-tui.sha256")"
  actual="$(sha256sum "$TMP/telegram-tui" | awk '{print $1}')"
  [ -n "$expected" ] && [ "$actual" = "$expected" ] \
    || { echo "error: checksum mismatch" >&2; exit 1; }
fi

mkdir -p "$DIR"
chmod +x "$TMP/telegram-tui"
mv -f "$TMP/telegram-tui" "$BIN"

echo "==> installed: $BIN"
case ":$PATH:" in
  *":$DIR:"*) ;;
  *) echo "==> note: add $DIR to your PATH to run 'telegram-tui'" ;;
esac
