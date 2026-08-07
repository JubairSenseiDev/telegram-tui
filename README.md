# telegram-tui

A keyboard-first Telegram TUI for Termux and Linux, in the spirit of MovieBox-Tui.
Multi-language workspace: **Rust** (main TUI client), **Go** (bot service),
**Python** (data/export tools). Pure MTProto via [grammers](https://codeberg.org/Lonami/grammers) — no TDLib needed.

## Features

- Full TUI: dashboard, dialogs, chat, accounts, setup, login, prompts
- Multi-account login, switching, and session deletion
- Send messages, reply, save notes to Saved Messages
- Global message search
- Export dialogs/members as CSV, chat history as text
- Join public groups/channels by username or t.me link
- Profile viewer
- Go Bot API service for automation
- Python export summarizers/stats (stdlib only)

## Layout

```
src/          Rust TUI client (grammers, ratatui, crossterm)
go/bot        Go Telegram Bot API service (gotgbot)
python/       Python export/analysis scripts
install.sh    unattended Termux/Linux install
```

## Commands (Rust TUI)

| Command | Action |
| --- | --- |
| `/setup` | Save Telegram API credentials |
| `/login` | Login a new account |
| `/inbox` | Open recent chats |
| `/send` | Send one message |
| `/sendfile` | Upload and send a file |
| `/note` | Save text to Saved Messages |
| `/search` | Search messages |
| `/dialogs` | Export dialog list |
| `/members` | Export members CSV |
| `/chat` | Export chat history |
| `/profile` | View profile |
| `/join` | Join public group/channel |
| `/accounts` | Switch/delete sessions |
| `/exports` | List exported files |
| `/help` | Show help |
| `/quit` | Exit |

## Install

### Termux / Linux one-liner

```sh
bash <(curl -s https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh)
```

The installer builds the Rust TUI (`~/.local/bin/telegram-tui`) and, if the
Bot token is present in the environment, the Go bot (`~/.local/bin/telegram-tui-bot`).

### Manual

```sh
# Rust TUI
pkg install rust -y            # Linux: apt/dnf install rustc cargo
git clone https://github.com/JubairSenseiDev/telegram-tui.git
cd telegram-tui
cargo build --release
install -Dm755 target/release/telegram-tui ~/.local/bin/telegram-tui

# Go bot
cd go/bot
export TELEGRAM_BOT_TOKEN="123:abc"   # from @BotFather
go run .
```

### Get API credentials

Create an app at https://my.telegram.org/apps, then inside the TUI run `/setup`.
Credentials are stored in `~/.config/telegram-tui/.env` (mode 600). Sessions live in
`~/.config/telegram-tui/sessions/` and exports in `~/.config/telegram-tui/exports/`.

## Keys

- Esc = back, Ctrl+C = quit
- Lists: j/k or arrows, PageUp/PageDown, Enter
- Chat: s = send, r = reply, e = export chat, m = export members,
  o/l = older messages, f = search in chat, d = delete (type `DELETE`),
  E = edit, p/P = pin/unpin, v = members, M = mark read, R = refresh
- Members: j/k scroll, x = export CSV, e = export chat
- Dialogs: r = reload, x = members, e = export chat
- Accounts: Enter = switch, l = login, d = delete (type `DELETE`)

## Releases

Prebuilt binaries are built automatically on every `v*` tag via
[GitHub Actions](.github/workflows/release.yml):

- `x86_64-linux`, `aarch64-linux`, `armv7-linux`
- `x86_64-windows.exe`
- `aarch64-macos`
- `aarch64-termux` (best-effort; Termux users can also build on-device with `install.sh`)

Tag a release with:

```sh
git tag v4.0.0
git push origin v4.0.0
```

## License

MIT
