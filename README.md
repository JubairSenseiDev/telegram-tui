# 💬 Telegram TUI

> A fast, keyboard-first Telegram client that runs right in your terminal — on **Termux**, **Linux**, **macOS** and **Windows**. No TDLib, no GUI, no browser — pure MTProto over the terminal.

![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Linux%20%7C%20macOS%20%7C%20Windows-00b8ff)
![Lang](https://img.shields.io/badge/language-Rust%20%7C%20Go%20%7C%20Python-ee4c2c)
![Release](https://img.shields.io/github/v/release/JubairSenseiDev/telegram-tui?label=release&color=27b859)
![Downloads](https://img.shields.io/github/downloads/JubairSenseiDev/telegram-tui/total?color=9b59b6)
![License](https://img.shields.io/github/license/JubairSenseiDev/telegram-tui?color=9b59b6)

---

## ✨ Features

- 🖥️ **Full TUI** — dashboard, dialogs, chat, search, profile, exports, all mouse-free and keyboard-driven
- 👥 **Multi-account** — login, switch and delete sessions
- 📨 **Messaging** — send, reply, edit, delete, pin, unpin, forward, mark read
- 📎 **Files** — upload & send files from your device
- ⬇️ **Private downloads** — download videos / photos / media from any chat, group or channel — **including private ones** you're a member of — with `g` or `/download`
- 🔎 **Search** — global message search + in-chat search
- 📦 **Exports** — chats, dialogs and members to CSV / text
- 🐍 **Python tools** — export summaries & chat stats (stdlib only)
- 🤖 **Go bot** — broadcasts, channel mirroring, keyword auto-replies, scheduled messages and a **grab-and-share assistant** (`/getpost`, `/afwd`, watched channels — no downloads, server-side copies)

---

## 🚀 Install

### One line (Termux / Linux / macOS)

```sh
bash <(curl -s https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh)
```

The installer **downloads the latest release binary** for your platform, verifies its SHA-256 checksum and installs it. Then just run:

```sh
telegram-tui
```

> On Termux it installs to `$PREFIX/bin`; on Linux to `/usr/local/bin` (root) or `~/.local/bin` (user). It also updates an already-installed copy.

### Debian / Ubuntu — `.deb`

Grab the matching package from the [latest release](https://github.com/JubairSenseiDev/telegram-tui/releases):

```sh
sudo apt install ./telegram-tui-x86_64-linux.deb    # amd64
sudo apt install ./telegram-tui-aarch64-linux.deb   # arm64 (Raspberry Pi 3+)
sudo apt install ./telegram-tui-armv7-linux.deb     # armhf (Pi 0/1/2)
```

### Windows

Download `telegram-tui-x86_64-windows.exe` from the [latest release](https://github.com/JubairSenseiDev/telegram-tui/releases) and run it (or rename it to `telegram-tui.exe` and add it to your PATH). Git Bash / MSYS users can also use the one-liner above.

### Build from source

```sh
git clone https://github.com/JubairSenseiDev/telegram-tui.git
cd telegram-tui
cargo build --release
install -m755 target/release/telegram-tui ~/.local/bin/telegram-tui
```

Cross-builds:

```sh
# Windows (x86_64) on Linux
cargo build --release --target x86_64-pc-windows-gnu

# Termux/Android (aarch64) — needs cargo-ndk
cargo ndk -t arm64-v8a build --release

# Linux arm64 / armv7 — needs the matching cross gcc
cargo build --release --target aarch64-unknown-linux-gnu
cargo build --release --target armv7-unknown-linux-gnueabihf
```

---

## 🔑 First run

1. Create an API app at [my.telegram.org/apps](https://my.telegram.org/apps) — you get an `api_id` and `api_hash`.
2. Run `telegram-tui`, then use `/setup` and paste your `api_id` / `api_hash`.
3. Use `/login`, enter your phone number and the code Telegram sends.

> Credentials are stored in `~/.config/telegram-tui/.env` (mode `600`), sessions in `sessions/`, exports in `exports/`, downloads in `downloads/`. Set `TELEGRAM_TUI_SESSION=<name>` to pin which session starts by default.

---

## ⌨️ Commands

| Command | Action |
| --- | --- |
| `/setup` | Save Telegram API credentials |
| `/login` | Login a new account |
| `/inbox` | Check inbox |
| `/send` | Send a message |
| `/sendfile` | Upload and send a file |
| `/note` | Save text to Saved Messages |
| `/search` | Search messages |
| `/dialogs` | Export dialogs CSV |
| `/members` | Export members CSV |
| `/chat` | Export chat history |
| `/profile` | View profile |
| `/join` | Join a public group/channel |
| `/download <t.me/...>` | Download media from a `t.me/...` message link |
| `/accounts` | Switch / delete sessions |
| `/exports` | List exported files |
| `/help` | Show help |
| `/quit` | Exit |

## 🎹 Keys

| Key | Action |
| --- | --- |
| `Esc` | Back |
| `Ctrl+C` | Quit |
| `j` / `k` · `↑` / `↓` | Move in lists |
| `Enter` | Open / select |
| `s` | Send |
| `r` | Reply |
| `e` | Export chat |
| `m` | Export members |
| `o` / `l` | Older / newer messages |
| `f` | Search in chat |
| `d` | Delete (type `DELETE` to confirm) |
| `E` | Edit message |
| `p` / `P` | Pin / unpin |
| `v` | Members |
| `g` | Download media of the selected message |
| `M` | Mark read |
| `R` | Refresh |

## ⬇️ Downloads

Two ways to grab media:

- Select a message with media and press **`g`** — downloads to `~/.config/telegram-tui/downloads/`.
- Paste any post link: **`/download https://t.me/c/123456789/42`** or `https://t.me/channel_name/42`.

Public and **private** chats, groups and channels work, as long as your logged-in account is a member. File names are sanitized, duplicates get a unique suffix, and a progress toast shows while downloading.

---

## 🤖 Go bot

A full Telegram Bot API service that runs 24/7 alongside the TUI:

- 📢 **Broadcast** — send a message to every subscriber
- 🔁 **Mirror** — forward everything from one chat/channel to another
- 💬 **Keyword auto-replies** & **scheduled messages**
- 🗂️ **Assistant** — watch channels and archive every post; then share any post instantly with **`/getpost <t.me link>`** or **`/afwd <id>`** using server-side copies — zero download, even for huge files

```sh
cd go/bot
export TELEGRAM_BOT_TOKEN="123:abc"     # from @BotFather
export ADMIN_USER_IDS="123456,789012"
go run .
```

Full command reference, config and docs → **[go/bot/README.md](go/bot/README.md)**.

---

## 🐍 Python tools

Export summary and chat statistics using only the standard library:

```sh
python3 python/export_summary.py ~/.config/telegram-tui/exports/dialogs-*.csv
python3 python/chat_stats.py ~/.config/telegram-tui/exports/chat-*.txt
```

See [python/README.md](python/README.md) for details.

---

## 📦 Releases

Every `v*` tag triggers GitHub Actions to build and **auto-publish** all binaries, `.deb` packages, checksums and the `install.sh` installer — see the [latest release](https://github.com/JubairSenseiDev/telegram-tui/releases).

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `telegram-tui-x86_64-linux` + `.deb` |
| Linux ARM64 | `telegram-tui-aarch64-linux` + `.deb` |
| Linux ARMv7 | `telegram-tui-armv7-linux` + `.deb` |
| Windows x86_64 | `telegram-tui-x86_64-windows.exe` |
| macOS ARM64 | `telegram-tui-aarch64-macos` |
| Termux ARM64 | `telegram-tui-aarch64-termux` |
| Installer | `install.sh` |

---

## 🧩 Project structure

```
src/          Rust TUI client (grammers + ratatui)
go/bot/       Go Telegram Bot API service
python/       Python export & analysis scripts
.github/      Release automation (GitHub Actions)
install.sh    One-line installer (curl | sh)
```

## 🛠️ Development

```sh
cargo test --release        # Rust unit tests
cargo clippy --release      # lint
cargo build --release       # build

cd go/bot
go build ./... && go vet ./... && go test ./...   # bot checks
```

---

## 📄 License

MIT — see [LICENSE](LICENSE).
