# Telegram Sensei Toolkit

A modern, safer Telegram command-line toolkit inspired by `jubairbro/telegram`. It is built for Termux and desktop Python with clean packaging, local credentials, ignored session files, and a Rich-powered interface.

## Features

- Multi-account login, switching, status checks, and session deletion
- Recent inbox reader with quick replies
- Chat history export to `exports/`
- Group/channel member export to CSV
- Profile name, bio, username, and photo updates
- Join public groups or channels by username/link
- Local `.env` credential loading instead of remote config files
- No committed Telegram sessions or private config files

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
cp .env.example .env
```

Edit `.env` and add your Telegram API credentials from https://my.telegram.org/apps:

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
