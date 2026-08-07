# Telegram Sensei Toolkit

A modern, safer Telegram command-line toolkit inspired by `jubairbro/telegram`, with a MovieBox-TUI-style command palette. It is built for Termux and desktop Python 3.11+ with clean packaging, local credentials, ignored session files, and a Rich-powered interface.

## Features

- Multi-account login, switching, status checks, and session deletion
- Interactive API credential setup wizard
- Recent inbox reader with quick replies
- Send a single message to a username, phone, link, or ID
- Save notes directly to Telegram Saved Messages
- Search messages in one chat or across dialogs
- Export dialog list to CSV
- Chat history export to `exports/`
- Group/channel member export to CSV
- Profile name, bio, username, and photo updates
- Join public groups or channels by username/link
- Local `.env` credential loading instead of remote config files
- No committed Telegram sessions or private config files
- Keyboard-first dashboard with numbered actions and slash commands

## TUI Commands

| Command | Action |
| --- | --- |
| `/setup` | Save Telegram API credentials |
| `/login` | Login a new account |
| `/inbox` | Open recent chats |
| `/reply` | Reply to recent chats |
| `/send` | Send one message |
| `/note` | Save text to Saved Messages |
| `/search` | Search messages |
| `/dialogs` | Export dialog list |
| `/members` | Export members CSV |
| `/chat` | Export chat history |
| `/profile` | Edit profile |
| `/join` | Join public group/channel |
| `/status` | Show account status |
| `?` or `/help` | Show help |
| `q` or `/quit` | Exit |

## Install

### Termux Quick Install

```sh
bash <(curl -s https://raw.githubusercontent.com/JubairSenseiDev/telegram-sensei-toolkit/main/install.sh)
```

### Manual Install

```sh
pkg install git python -y
git clone https://github.com/JubairSenseiDev/telegram-sensei-toolkit.git
cd telegram-sensei-toolkit
python -m pip install -e .
```

Run `telegram-sensei`, choose `Setup API credentials`, and add your Telegram API credentials from https://my.telegram.org/apps.

You can also create `.env` manually:

```sh
TELEGRAM_API_ID=123456
TELEGRAM_API_HASH=your_api_hash_here
```

Run:

```sh
telegram-sensei
```

## Safety Notes

Use this only for accounts you own and chats where you have permission. Telegram rate limits and Terms of Service still apply. Session files, exports, and `.env` are intentionally ignored by git.

## Credit

Built for `JubairSenseiDev`, inspired by the original Telegram Advanced Toolkit by `jubairbro`.
