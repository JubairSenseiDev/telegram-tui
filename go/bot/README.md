# 🤖 telegram-tui Go bot

A real Telegram bot service: **broadcasts**, **channel mirroring**, **keyword auto-replies**, **scheduled messages** and a **grab-and-share assistant**. State is kept in JSON files, so the bot survives restarts.

## Configure

```sh
export TELEGRAM_BOT_TOKEN="123:abc"        # required, from @BotFather
export ADMIN_USER_IDS="123456,789012"      # comma-separated user IDs allowed to use admin commands
export SOURCE_CHAT_ID="-100123456789"      # optional: chat/channel to mirror FROM
export TARGET_CHAT_ID="-100987654321"      # optional: chat/channel to mirror INTO
export DATA_DIR="data"                     # optional: state directory (default: ./data)
export ASSISTANT_SAVE_MEDIA="false"        # optional: auto-download media bytes (default: metadata only)
export ASSISTANT_MAX_MEDIA_MB="50"         # optional: per-media download cap to keep data low (default: 50)
```

> Chat IDs are signed 64-bit integers (negative for groups/channels, positive for users). Get them with `/start` on [@userinfobot](https://t.me/userinfobot).

## Run

```sh
go run .
```

## Get all user IDs from a bot token

One-shot mode — reads the bot's pending updates and prints every user and
chat ID it finds, then exits. Use it to discover `ADMIN_USER_IDS`,
`SOURCE_CHAT_ID`, `TARGET_CHAT_ID`, etc.:

```sh
TELEGRAM_BOT_TOKEN="123:abc" go run . --getids
```

While running, the `/ids` admin command also lists every user & chat ID the
bot has seen (persisted in `data/seen.json`).

## Commands

| Command | Who | Action |
| --- | --- | --- |
| `/start` | all | subscribe to broadcasts |
| `/help` | all | show help |
| `/ping` | all | latency check |
| `/subcount` | all | number of subscribers |
| `/broadcast <text>` | admin | send a message to every subscriber |
| `/list` | admin | list subscribers |
| `/ids` | admin | list every user & chat ID the bot has seen |
| `/addkeyword <word>\|<reply>` | admin | add a keyword auto-reply |
| `/delkeyword <word>` | admin | remove a keyword auto-reply |
| `/keywords` | admin | list keyword auto-replies |
| `/schedule <seconds> <text>` | admin | repeat a message every N seconds (min 10) |
| `/schedules` | admin | list schedules |
| `/scheduledel <id>` | admin | delete a schedule |
| `/addchannel <id>\|<@user>\|<t.me/...>` | admin | watch a channel/group, archive every post locally |
| `/removechannel <id>` | admin | stop watching a channel |
| `/listchannels` | admin | list watched channels |
| `/getpost <t.me/...> [target]` | admin | copy a post straight from a t.me link into a chat |
| `/afwd <id> [target]` | admin | forward a saved post to a chat — **no download, full media included** |

## Assistant — grab & share posts, no download

The assistant keeps a local **index of every watched post** (text, caption,
media info, link) in `DATA_DIR/assistant_posts.jsonl` (append-only, one post
per line). Sharing is done **server-side** — Telegram copies or forwards the
post's own copy, so even huge media moves **instantly with zero download**.

- **Watch channels** — `/addchannel` a public or private channel you own/admin (add the bot as a channel admin so it receives posts). Every new post is archived locally.
- **`/getpost <link> [target]`** — paste any `t.me/...` post link (e.g. `t.me/c/1550117445/42`) and the bot copies that post into the current chat (or `target`). Text-only posts stay free; media is copied server-side too.
- **`/afwd <id> [target]`** — share an archived post by *forwarding*. Run it in the target chat (or pass a target id/`@username`). Telegram reuses its own copy server-side — instant, no download.
- **`/addchannel` / `/removechannel` / `/listchannels`** — manage which channels get archived automatically.
- Media *bytes* are only downloaded to `DATA_DIR/assistant/media/` if you set `ASSISTANT_SAVE_MEDIA=true` (capped by `ASSISTANT_MAX_MEDIA_MB`, default 50).

## Automation (no commands needed)

- **Mirror** — every new message in `SOURCE_CHAT_ID` is forwarded to `TARGET_CHAT_ID` (text, media, everything). Set both IDs to enable.
- **Keyword replies** — when a plain message contains a keyword, the bot replies with the configured text (case-insensitive).
- **New-member greeting** — greets the chat when the bot is added to a group.

## Build & test

```sh
go build -o telegram-tui-bot .
go test ./...
```
