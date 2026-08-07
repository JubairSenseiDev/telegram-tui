#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/JubairSenseiDev/telegram-sensei-toolkit.git"
APP_DIR="$HOME/telegram-sensei-toolkit"

if command -v pkg >/dev/null 2>&1; then
  pkg update -y
  pkg install git python -y
fi

if [ -d "$APP_DIR/.git" ]; then
  git -C "$APP_DIR" pull --ff-only
else
  rm -rf "$APP_DIR"
  git clone "$REPO_URL" "$APP_DIR"
fi

python -m pip install -e "$APP_DIR"

if [ ! -f "$APP_DIR/.env" ]; then
  cp "$APP_DIR/.env.example" "$APP_DIR/.env"
fi

printf '\nInstalled. Add TELEGRAM_API_ID and TELEGRAM_API_HASH in %s/.env, then run: telegram-sensei\n' "$APP_DIR"
