# 💬 Telegram TUI

> A fast, keyboard-first Telegram client that runs right in your terminal — on **Termux**, **Linux**, **macOS** and **Windows**.

![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Linux%20%7C%20macOS%20%7C%20Windows-00b8ff)
![Lang](https://img.shields.io/badge/language-Rust%20%7C%20Go%20%7C%20Python-ee4c2c)
![Release](https://img.shields.io/github/v/release/JubairSenseiDev/telegram-tui?label=release&color=27b859)
![License](https://img.shields.io/github/license/JubairSenseiDev/telegram-tui?color=9b59b6)

No TDLib, no GUI, no browser. Pure MTProto over the terminal.

---

## ✨ Features

- 🖥️ **Full TUI** — dashboard, dialogs, chat, search, profile, exports
- 👥 **Multi-account** — login, switch and delete sessions
- 📨 **Messaging** — send, reply, edit, delete, pin, send files
- ⬇️ **Private downloads** — download videos / photos / media from any chat, group or channel (including **private** ones you're a member of) with `g` or `/download`
- 🔎 **Search** — global search + in-chat search
- 📦 **Exports** — chats, dialogs and members to CSV/text
- 🐍 **Python tools** — export summaries & chat stats (stdlib only)
- 🤖 **Go bot** — broadcast to subscribers, mirror chats/channels, keyword auto-replies, scheduled messages, and a **low-data post-saver assistant** (watch channels → archive full posts locally)

---

## 🚀 Install

### Termux / Linux — one line

```sh
bash <(curl -s https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh)
```

The installer **downloads the latest release binary** for your platform, verifies its checksum and installs it — then just run `telegram-tui` to open it. Linux users can also grab the `.deb` (amd64 / arm64 / armhf) from the latest release.

### Manual build

```sh
git clone https://github.com/JubairSenseiDev/telegram-tui.git
cd telegram-tui
cargo build --release
install -m755 target/release/telegram-tui ~/.local/bin/telegram-tui
```

### 🔑 Get API credentials

1. Create an app at [my.telegram.org/apps](https://my.telegram.org/apps)
2. Run the TUI, then use `/setup` and paste your `api_id` / `api_hash`

> Credentials live in `~/.config/telegram-tui/.env` (mode `600`). Sessions → `sessions/`, exports → `exports/`, downloads → `downloads/`.

---

## ⌨️ Commands

| Command | Action |
| --- | --- |
| `/setup` | Save Telegram API credentials |
| `/login` | Login a new account |
| `/inbox` | Open recent chats |
| `/send` | Send a message |
| `/sendfile` | Upload and send a file |
| `/note` | Save text to Saved Messages |
| `/search` | Search messages |
| `/download` | Download media from a `t.me/...` message link |
| `/join` | Join a public group/channel |
| `/accounts` | Switch / delete sessions |
| `/exports` | List exported files |
| `/help` | Show help |
| `/quit` | Exit |

---

## 🎹 Keys

| Key | Action |
| --- | --- |
| `Esc` | Back |
| `Ctrl+C` | Quit |
| `j` / `k` · `↑` / `↓` | Move in lists |
| `Enter` | Open / select |
| `s` | Send · `r` reply · `e` export chat · `m` export members |
| `o` / `l` | Older / newer messages |
| `f` | Search in chat |
| `d` | Delete (type `DELETE`) |
| `E` | Edit message |
| `p` / `P` | Pin / unpin |
| `v` | Members |
| `g` | Download media of the selected message |
| `M` | Mark read |
| `R` | Refresh |

---

## 📦 Releases

Prebuilt binaries for every platform are built automatically on each `v*` tag — see the [latest release](https://github.com/JubairSenseiDev/telegram-tui/releases).

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `telegram-tui-x86_64-linux` |
| Linux ARM64 | `telegram-tui-aarch64-linux` |
| Linux ARMv7 | `telegram-tui-armv7-linux` |
| Windows x86_64 | `telegram-tui-x86_64-windows.exe` |
| macOS ARM64 | `telegram-tui-aarch64-macos` |
| Termux ARM64 | `telegram-tui-aarch64-termux` |

---

## 🧩 Modules

| Module | Description |
| --- | --- |
| `src/` | Rust TUI client (grammers + ratatui) |
| `go/bot/` | Go Telegram Bot API service |
| `python/` | Python export & analysis scripts |
| `.github/` | Release automation (GitHub Actions) |

---

## 📄 License

MIT — see [LICENSE](LICENSE).
