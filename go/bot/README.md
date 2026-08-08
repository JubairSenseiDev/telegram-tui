# 🤖 telegram-tui Go bot

A real Telegram bot service: **broadcasts**, **channel mirroring**, **keyword auto-replies** and **scheduled messages**. State is kept in JSON files, so the bot survives restarts.

## Configure

```sh
export TELEGRAM_BOT_TOKEN="123:abc"        # required, from @BotFather
export ADMIN_USER_IDS="123456,789012"      # comma-separated user IDs allowed to use admin commands
export SOURCE_CHAT_ID="-100123456789"      # optional: chat/channel to mirror FROM
export TARGET_CHAT_ID="-100987654321"      # optional: chat/channel to mirror INTO
export DATA_DIR="data"                     # optional: state directory (default: ./data)
```

> Chat IDs are signed 64-bit integers (negative for groups/channels, positive for users). Get them with `/start` on [@userinfobot](https://t.me/userinfobot).

## Run

```sh
go run .
```

## Commands

| Command | Who | Action |
| --- | --- | --- |
| `/start` | all | subscribe to broadcasts |
| `/help` | all | show help |
| `/ping` | all | latency check |
| `/subcount` | all | number of subscribers |
| `/broadcast <text>` | admin | send a message to every subscriber |
| `/list` | admin | list subscribers |
| `/addkeyword <word>\|<reply>` | admin | add a keyword auto-reply |
| `/delkeyword <word>` | admin | remove a keyword auto-reply |
| `/keywords` | admin | list keyword auto-replies |
| `/schedule <seconds> <text>` | admin | repeat a message every N seconds (min 10) |
| `/schedules` | admin | list schedules |
| `/scheduledel <id>` | admin | delete a schedule |

## Automation (no commands needed)

- **Mirror** — every new message in `SOURCE_CHAT_ID` is forwarded to `TARGET_CHAT_ID` (text, media, everything). Set both IDs to enable.
- **Keyword replies** — when a plain message contains a keyword, the bot replies with the configured text (case-insensitive).
- **New-member greeting** — greets the chat when the bot is added to a group.

## Build & test

```sh
go build -o telegram-tui-bot .
go test ./...
```
