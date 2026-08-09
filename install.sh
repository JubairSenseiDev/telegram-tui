#!/bin/sh
# One-line installer for telegram-tui.
#   curl -fsSL https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh | sh
# POSIX sh: the documented entry point pipes into `sh`, which is dash on
# Debian and Termux, so no bashisms (pipefail, $'..', arrays, local).
set -eu

REPO="JubairSenseiDev/telegram-tui"
BASE="https://github.com/$REPO/releases/latest/download"

# --- presentation -----------------------------------------------------------
# Animation only when stderr is a terminal, so `curl | sh` in a log stays plain.
if [ -t 2 ] && [ -z "${NO_COLOR:-}" ]; then
  ESC=$(printf '\033')
  C_RESET="${ESC}[0m"; C_DIM="${ESC}[2m"; C_CYAN="${ESC}[36m"
  C_GREEN="${ESC}[32m"; C_RED="${ESC}[31m"; C_BOLD="${ESC}[1m"
  TTY=1
else
  C_RESET=""; C_DIM=""; C_CYAN=""; C_GREEN=""; C_RED=""; C_BOLD=""
  TTY=0
fi

hide_cursor() { [ "$TTY" = 1 ] && printf '\033[?25l' >&2 || true; }
show_cursor() { [ "$TTY" = 1 ] && printf '\033[?25h' >&2 || true; }

banner() {
  [ "$TTY" = 1 ] || return 0
  printf '%s\n' "" >&2
  printf '%s\n' "  ${C_CYAN}${C_BOLD}telegram-tui${C_RESET}  ${C_DIM}terminal Telegram client${C_RESET}" >&2
  printf '%s\n' "  ${C_DIM}────────────────────────────────────${C_RESET}" >&2
}

step() { printf '  %s→%s %s\n' "$C_CYAN" "$C_RESET" "$1" >&2; }
ok()   { printf '  %s✓%s %s\n' "$C_GREEN" "$C_RESET" "$1" >&2; }
die()  { printf '  %s✗%s %s\n' "$C_RED" "$C_RESET" "$1" >&2; exit 1; }

# Run a command with a spinner. Returns the command's own exit code.
spin() {
  label="$1"; shift
  if [ "$TTY" != 1 ]; then
    printf '==> %s\n' "$label" >&2
    "$@"
    return $?
  fi
  "$@" & pid=$!
  hide_cursor
  while kill -0 "$pid" 2>/dev/null; do
    # Positional params keep each frame whole; `cut -c` would split UTF-8 bytes.
    set -- '⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏'
    for frame in "$@"; do
      kill -0 "$pid" 2>/dev/null || break
      printf '\r  %s%s%s %s' "$C_CYAN" "$frame" "$C_RESET" "$label" >&2
      sleep 0.08
    done
  done
  wait "$pid"; rc=$?
  printf '\r\033[2K' >&2
  show_cursor
  return $rc
}

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

banner
step "detecting platform: ${C_BOLD}${suffix}${C_RESET}"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"; show_cursor' EXIT INT TERM
spin "downloading telegram-tui ($suffix)" curl -fsSL "$URL" -o "$TMP/telegram-tui" \
  || die "download failed (curl -fsSL \"$URL\")"
ok "downloaded $suffix binary"

# --- verify sha256 if published ---------------------------------------------
if curl -fsSL "$URL.sha256" -o "$TMP/telegram-tui.sha256" 2>/dev/null; then
  expected="$(awk 'NR==1{print $1}' "$TMP/telegram-tui.sha256")"
  actual="$(sha256sum "$TMP/telegram-tui" | awk '{print $1}')"
  [ -n "$expected" ] && [ "$actual" = "$expected" ] \
    || die "checksum mismatch (expected $expected, got $actual)"
  ok "checksum verified"
fi

mkdir -p "$DIR"
# Explicit mode, not `chmod +x`: that honours umask, so a root install under
# umask 077 would leave the binary unreadable to everyone but root.
chmod 755 "$TMP/telegram-tui"
mv -f "$TMP/telegram-tui" "$BIN"

ok "installed: ${C_BOLD}$BIN${C_RESET}"
case ":$PATH:" in
  *":$DIR:"*) ;;
  *) printf '  %snote:%s add %s to your PATH to run %s\n' "$C_DIM" "$C_RESET" "$DIR" "'telegram-tui'" >&2 ;;
esac
printf '%s\n' "  ${C_GREEN}${C_BOLD}done.${C_RESET}" >&2
