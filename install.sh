#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/JubairSenseiDev/telegram-tui.git"
APP_DIR="$HOME/telegram-tui"
BIN_DIR="$HOME/.local/bin"

log() { printf '\033[1;36m[telegram-tui]\033[0m %s\n' "$*"; }

log "installing rust + git"
if command -v pkg >/dev/null 2>&1; then
  pkg update -y
  pkg install git rust -y
elif command -v apt-get >/dev/null 2>&1; then
  apt-get update -y
  apt-get install -y git curl build-essential pkg-config
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  export PATH="$HOME/.cargo/bin:$PATH"
else
  echo "unsupported package manager; install rust + git manually" >&2
  exit 1
fi

log "fetching repo"
if [ -d "$APP_DIR/.git" ]; then
  git -C "$APP_DIR" pull --ff-only
else
  git clone "$REPO_URL" "$APP_DIR"
fi

log "building rust TUI (this can take a while on Termux)"
mkdir -p "$BIN_DIR"
cargo build --release --manifest-path "$APP_DIR/Cargo.toml"
install -Dm755 "$APP_DIR/target/release/telegram-tui" "$BIN_DIR/telegram-tui"

if [ -n "${TELEGRAM_BOT_TOKEN:-}" ]; then
  log "building go bot"
  if command -v go >/dev/null 2>&1 || pkg install golang -y 2>/dev/null; then
    (cd "$APP_DIR/go/bot" && go build -o "$BIN_DIR/telegram-tui-bot" .)
  fi
fi

log "installed. run: telegram-tui"
log "get API credentials from https://my.telegram.org/apps and use /setup inside the TUI"
