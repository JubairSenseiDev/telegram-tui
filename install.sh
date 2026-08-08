#!/usr/bin/env bash
set -euo pipefail

REPO="JubairSenseiDev/telegram-tui"
APP_DIR="$HOME/telegram-tui"
VERSION="${VERSION:-latest}"

log()   { printf '\033[1;36m[telegram-tui]\033[0m %s\n' "$*"; }
ok()    { printf '\033[1;32m[telegram-tui]\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m[telegram-tui]\033[0m %s\n' "$*"; }
die()   { printf '\033[1;31m[telegram-tui]\033[0m %s\n' "$*" >&2; exit 1; }

MODE="auto"
for arg in "$@"; do
  case "$arg" in
    --source)  MODE="source" ;;
    --release) MODE="release" ;;
    -h|--help) echo "usage: bash install.sh [--release|--source]"; exit 0 ;;
  esac
done

# --- detect platform ---------------------------------------------------------
ARCH="$(uname -m)"
if command -v pkg >/dev/null 2>&1; then
  OS="termux"
  BIN_DIR="$PREFIX/bin"
  ASSET=""
  [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ] && ASSET="telegram-tui-aarch64-termux"
  # no prebuilt binary for armv7 Termux devices yet
  [ "$ARCH" = "armv7l" ] && MODE="source"
else
  OS="linux"
  BIN_DIR="$HOME/.local/bin"
  case "$ARCH" in
    x86_64|amd64)           ASSET="telegram-tui-x86_64-linux" ;;
    aarch64|arm64)          ASSET="telegram-tui-aarch64-linux" ;;
    armv7l|armhf)           ASSET="telegram-tui-armv7-linux" ;;
    *)                      ASSET="" ;;
  esac
fi

release_url() {
  if [ "$VERSION" = "latest" ]; then
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
      | grep -o '"browser_download_url": *"[^"]*'"$ASSET"'"' \
      | head -n1 | sed -E 's/.*"browser_download_url": *"([^"]+)".*/\1/'
  else
    printf '%s' "https://github.com/$REPO/releases/download/$VERSION/$ASSET"
  fi
}

# --- download prebuilt -------------------------------------------------------
download() {
  local url bin
  url="$(release_url)"
  [ -n "$url" ] || return 1
  log "downloading $ASSET from GitHub releases"
  mkdir -p "$BIN_DIR"
  curl -fL --progress-bar "$url" -o "$BIN_DIR/.telegram-tui.tmp"
  if [ -x "$BIN_DIR/.telegram-tui.tmp" ] || head -c4 "$BIN_DIR/.telegram-tui.tmp" | grep -q ELF; then
    chmod +x "$BIN_DIR/.telegram-tui.tmp"
    mv "$BIN_DIR/.telegram-tui.tmp" "$BIN_DIR/telegram-tui"
    return 0
  fi
  rm -f "$BIN_DIR/.telegram-tui.tmp"
  return 1
}

# --- build from source -------------------------------------------------------
build() {
  log "installing rust + git"
  if command -v pkg >/dev/null 2>&1; then
    pkg update -y
    pkg install git rust -y
  elif command -v apt-get >/dev/null 2>&1; then
    apt-get update -y
    apt-get install -y git curl build-essential pkg-config
    command -v cargo >/dev/null 2>&1 || \
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    die "unsupported package manager; install rust + git manually"
  fi

  log "fetching source"
  if [ -d "$APP_DIR/.git" ]; then
    git -C "$APP_DIR" pull --ff-only
  else
    git clone "https://github.com/$REPO.git" "$APP_DIR"
  fi

  log "building rust TUI (this can take a while on Termux)"
  mkdir -p "$BIN_DIR"
  cargo build --release --manifest-path "$APP_DIR/Cargo.toml"
  install -Dm755 "$APP_DIR/target/release/telegram-tui" "$BIN_DIR/telegram-tui"
}

# --- main --------------------------------------------------------------------
case "$MODE" in
  release) download || die "download failed; run again with --source to build locally" ;;
  source)  build ;;
  auto)
    if [ -n "$ASSET" ] && download; then
      ok "installed prebuilt binary"
    else
      warn "no prebuilt binary for this platform — building from source"
      build
    fi
    ;;
esac

# go bot (optional — only when the source tree is present)
if [ -d "$APP_DIR/go/bot" ] && [ -n "${TELEGRAM_BOT_TOKEN:-}" ]; then
  log "building go bot"
  if command -v go >/dev/null 2>&1 || pkg install golang -y 2>/dev/null; then
    (cd "$APP_DIR/go/bot" && go build -o "$BIN_DIR/telegram-tui-bot" .)
    ok "installed telegram-tui-bot"
  fi
fi

ok "installed: $BIN_DIR/telegram-tui"
log "run: telegram-tui"
log "get API credentials from https://my.telegram.org/apps and use /setup inside the TUI"
