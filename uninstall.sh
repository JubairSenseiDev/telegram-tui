#!/bin/sh
# Uninstaller for telegram-tui.
#   curl -fsSL https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/uninstall.sh | sh
# POSIX sh: the documented entry point pipes into `sh`, which is dash on
# Debian and Termux, so no bashisms (pipefail, $'..', arrays, local).
set -eu

# --- presentation -----------------------------------------------------------
# Colour only when stderr is a terminal, so `curl | sh` in a log stays plain.
if [ -t 2 ] && [ -z "${NO_COLOR:-}" ]; then
  ESC=$(printf '\033')
  C_RESET="${ESC}[0m"; C_DIM="${ESC}[2m"; C_CYAN="${ESC}[36m"
  C_GREEN="${ESC}[32m"; C_RED="${ESC}[31m"; C_YELLOW="${ESC}[33m"; C_BOLD="${ESC}[1m"
else
  C_RESET=""; C_DIM=""; C_CYAN=""; C_GREEN=""; C_RED=""; C_YELLOW=""; C_BOLD=""
fi

step() { printf '  %s→%s %s\n' "$C_CYAN" "$C_RESET" "$1" >&2; }
ok()   { printf '  %s✓%s %s\n' "$C_GREEN" "$C_RESET" "$1" >&2; }
warn() { printf '  %s!%s %s\n' "$C_YELLOW" "$C_RESET" "$1" >&2; }
die()  { printf '  %s✗%s %s\n' "$C_RED" "$C_RESET" "$1" >&2; exit 1; }

usage() {
  cat >&2 <<EOF
  ${C_CYAN}${C_BOLD}telegram-tui uninstaller${C_RESET}

  usage: uninstall.sh [--purge] [--yes] [--help]

    ${C_BOLD}--purge${C_RESET}  also delete credentials, login sessions, exports and
             downloads. Without it, only the binary is removed and your
             data is left untouched.
    ${C_BOLD}--yes${C_RESET}    skip the confirmation prompt (required for --purge
             when there is no terminal to prompt on).
EOF
  exit 0
}

PURGE=0
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --purge) PURGE=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    --help|-h) usage ;;
    *) die "unknown option: $arg" ;;
  esac
done

printf '%s\n' "" >&2
printf '%s\n' "  ${C_CYAN}${C_BOLD}telegram-tui${C_RESET}  ${C_DIM}uninstaller${C_RESET}" >&2
printf '%s\n' "  ${C_DIM}────────────────────────────────────${C_RESET}" >&2

# --- packaged installs win, because removing the file behind dpkg's back
# --- leaves the package database claiming telegram-tui is still installed.
if command -v dpkg-query >/dev/null 2>&1 && \
   dpkg-query -W -f='${Status}' telegram-tui 2>/dev/null | grep -q "install ok installed"; then
  warn "installed as a Debian package"
  printf '  %srun:%s sudo apt remove telegram-tui\n' "$C_DIM" "$C_RESET" >&2
  [ "$PURGE" = 1 ] || exit 0
  printf '  %s(continuing to the data step)%s\n' "$C_DIM" "$C_RESET" >&2
fi

# --- find the binary in every directory the installer might have used -------
CANDIDATES="
${PREFIX:-/data/data/com.termux/files/usr}/bin
/usr/local/bin
${XDG_BIN_HOME:-$HOME/.local/bin}
${USERPROFILE:-$HOME}/.local/bin
/usr/bin
"

found=""
for dir in $CANDIDATES; do
  for name in telegram-tui telegram-tui.exe; do
    bin="$dir/$name"
    # A directory can appear twice (Termux PREFIX and /usr/bin), so skip dupes.
    case " $found " in *" $bin "*) continue ;; esac
    [ -f "$bin" ] && found="$found $bin"
  done
done

# Anything on PATH the loop above missed, e.g. a hand-placed copy.
if command -v telegram-tui >/dev/null 2>&1; then
  which_bin=$(command -v telegram-tui)
  case " $found " in *" $which_bin "*) ;; *) found="$found $which_bin" ;; esac
fi

if [ -z "$found" ]; then
  step "no telegram-tui binary found"
else
  for bin in $found; do
    if rm -f "$bin" 2>/dev/null; then
      ok "removed $bin"
    elif command -v sudo >/dev/null 2>&1 && sudo rm -f "$bin" 2>/dev/null; then
      ok "removed $bin ${C_DIM}(sudo)${C_RESET}"
    else
      die "could not remove $bin — try: sudo rm -f $bin"
    fi
  done
fi

# --- data ------------------------------------------------------------------
DATA="${XDG_CONFIG_HOME:-$HOME/.config}/telegram-tui"

if [ ! -d "$DATA" ]; then
  printf '%s\n' "  ${C_GREEN}${C_BOLD}done.${C_RESET}" >&2
  exit 0
fi

count_in() { [ -d "$1" ] && find "$1" -maxdepth 1 -type f 2>/dev/null | wc -l | tr -d ' ' || echo 0; }
n_sessions=$(count_in "$DATA/sessions")
n_exports=$(count_in "$DATA/exports")
n_downloads=$(count_in "$DATA/downloads")

if [ "$PURGE" != 1 ]; then
  printf '\n  %skept:%s %s\n' "$C_DIM" "$C_RESET" "$DATA" >&2
  printf '  %s      %s logins, %s exports, %s downloads%s\n' \
    "$C_DIM" "$n_sessions" "$n_exports" "$n_downloads" "$C_RESET" >&2
  printf '  %s      re-run with --purge to delete them%s\n' "$C_DIM" "$C_RESET" >&2
  printf '%s\n' "  ${C_GREEN}${C_BOLD}done.${C_RESET}" >&2
  exit 0
fi

printf '\n  %sthis will permanently delete:%s\n' "$C_YELLOW" "$C_RESET" >&2
printf '    %s\n' "$DATA" >&2
printf '    %s API credentials, %s login sessions%s\n' "$C_DIM" "$n_sessions" "$C_RESET" >&2
printf '    %s %s exported files, %s downloaded files%s\n' "$C_DIM" "$n_exports" "$n_downloads" "$C_RESET" >&2
printf '  %sdeleting sessions signs this device out of Telegram.%s\n' "$C_DIM" "$C_RESET" >&2

if [ "$ASSUME_YES" != 1 ]; then
  # stdin is the script itself under `curl | sh`, so ask the terminal directly.
  # /dev/tty can exist yet fail to open (cron, containers), so try the read
  # rather than trusting a -r test on it.
  printf '\n  type %sDELETE%s to confirm: ' "$C_BOLD" "$C_RESET" >&2
  if reply=$(head -n 1 < /dev/tty 2>/dev/null); then
    [ "$reply" = "DELETE" ] || die "cancelled, nothing was deleted"
  else
    printf '\n' >&2
    die "no terminal to confirm on — re-run with --purge --yes if you are sure"
  fi
fi

rm -rf "$DATA" || die "could not remove $DATA"
ok "removed $DATA"
printf '%s\n' "  ${C_GREEN}${C_BOLD}done.${C_RESET}" >&2
