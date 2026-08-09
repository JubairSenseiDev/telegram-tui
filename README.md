# 💬 Telegram TUI

> A fast, keyboard-first Telegram client that runs right in your terminal — on **Termux**, **Linux**, **macOS** and **Windows**. No TDLib, no GUI, no browser — pure MTProto over the terminal.

![Platform](https://img.shields.io/badge/platform-Termux%20%7C%20Linux%20%7C%20macOS%20%7C%20Windows-00b8ff)
![Lang](https://img.shields.io/badge/language-Rust%20%7C%20Go%20%7C%20Python-ee4c2c)
![Release](https://img.shields.io/github/v/release/JubairSenseiDev/telegram-tui?label=release&color=27b859)
![Downloads](https://img.shields.io/github/downloads/JubairSenseiDev/telegram-tui/total?color=9b59b6)
![License](https://img.shields.io/github/license/JubairSenseiDev/telegram-tui?color=9b59b6)

---

## ✨ Features

- 🖥️ **Telegram-style layout** — chat list beside the conversation, always-visible composer, overlays for everything else
- 👥 **Multi-account** — login, switch, delete sessions, and check every account's status at once
- 📨 **Messaging** — send, reply, edit, delete, pin, unpin, forward, mark read
- 📎 **Files** — upload & send files from your device
- ⬇️ **Private downloads** — download videos / photos / media from any chat, group or channel — **including private ones** you're a member of — with `s` on a message or `D` for a `t.me/...` link
- 🔎 **Search** — global message search + in-chat search
- 👤 **Profile & groups** — edit name, bio, username, photo · join, leave, report
- 📦 **Exports** — chats, dialogs and members to CSV / text, written incrementally with a row limit
- 🛟 **Rate-limit aware** — short `FLOOD_WAIT`s are absorbed and retried instead of dropping your work
- 🐍 **Python tools** — export summaries & chat stats (stdlib only)
- 🤖 **Go bot** — broadcasts, channel mirroring, keyword auto-replies, scheduled messages and a **grab-and-share assistant** (`/getpost`, `/afwd`, watched channels — no downloads, server-side copies)

---

## 🚀 Install

### One line (Termux / Linux / macOS)

```sh
curl -fsSL https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/install.sh | sh
```

The installer **downloads the latest release binary** for your platform, verifies its SHA-256 checksum and installs it. Then just run:

```sh
telegram-tui
```

> On Termux it installs to `$PREFIX/bin`; on Linux to `/usr/local/bin` (root) or `~/.local/bin` (user). It also updates an already-installed copy.

### Uninstall

```sh
curl -fsSL https://raw.githubusercontent.com/JubairSenseiDev/telegram-tui/main/uninstall.sh | sh
```

Removes the binary and leaves your data alone. To delete credentials, login sessions,
exports and downloads too:

```sh
curl -fsSL .../uninstall.sh | sh -s -- --purge
```

`--purge` asks you to type `DELETE` first, since removing sessions signs this device out
of Telegram. Add `--yes` to skip the prompt in a script. If you installed the `.deb`, the
uninstaller points you at `apt remove` instead of deleting the file behind dpkg's back.

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

## 🖥️ Layout

Chat list on the left, conversation and composer on the right — both always visible.
`Tab` walks the three panes; everything that is not a conversation (accounts, members,
profile, exports, search results) opens as an overlay over the conversation pane.

```
┌ telegram-tui v4.1.0 ────────── Jubair @jubair · 3 accounts ─┐
│ Chats (42)          │ Rakib                                 │
│ · Saved Messages    │  14:01 Rakib                          │
│ · Rakib          2  │    kal ashbi?                         │
│ # Dev Group         │  14:02 You                            │
│ @ Updates           │    ha, bikale                         │
│                     ├───────────────────────────────────────┤
│                     │ Message (Enter sends, Esc leaves)     │
└─────────────────────┴───────────────────────────────────────┘
```

## 🎹 Keys

Press `?` in the app for this table. Uppercase letters are separate bindings.

| Where | Keys |
| --- | --- |
| Chat list | `↑↓`/`jk` move · `Enter` open · `/` filter · `r` reload |
| Conversation | `↑↓` move · `o` older · `Home` load older · `End` newest |
| Write | `i` or `Enter` focus composer · `Enter` send · `Esc` back |
| Message | `r` reply · `f` forward · `e` edit · `d` delete · `p`/`P` pin · `s` save media |
| Find | `/` search this chat · `g` search everywhere · `m` mark read |
| Send | `n` message someone · `u` send file · `w` note to self · `D` download link |
| Chats | `J` join · `L` leave groups · `m` members · `R` report |
| Export | `e` chats CSV · `M` members CSV · `X` chat history · `E` open exports |
| Accounts | `a` accounts · `S` account status · `p` profile · `Ctrl+N` add account |
| General | `Tab` switch pane · `Esc` cancel running work · `?` help · `q` quit |

Destructive actions confirm first: delete asks you to type `DELETE` and shows the message
preview plus the chat it lives in; leaving a group asks for `LEAVE`. Groups you created
are refused. Member scrapes and history exports ask for a limit before they start.

## ⬇️ Downloads

Two ways to grab media:

- Select a message with media and press **`s`** — downloads to `~/.config/telegram-tui/downloads/`.
- Press **`D`** and paste any post link: `https://t.me/c/123456789/42` or `https://t.me/channel_name/42`.

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
src/main.rs       entry point, terminal setup, event loop
src/app.rs        state, async task queue, toasts
src/actions.rs    every Telegram call the UI can start
src/keys.rs       key dispatch by focus
src/keys_modal.rs overlay and prompt keys
src/submit.rs     prompt submission
src/ui.rs         two-pane frame
src/ui_overlay.rs overlay panels
src/tg.rs         grammers wrapper
src/config.rs     credentials, sessions, paths
src/text.rs       display-width text helpers
src/input.rs      cursor-aware text field
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
