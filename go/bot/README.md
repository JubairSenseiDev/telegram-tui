# 🤖 telegram-tui Go bot

A real Telegram bot service: **broadcasts**, **channel mirroring**, **keyword auto-replies**, **scheduled messages** and a **low-data post-saver assistant**. State is kept in JSON files, so the bot survives restarts.

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
| `/asave` | admin | save the replied-to message |
| `/ashow <id>` | admin | copy the full saved text of a post (free, no download) |
| `/asearch <q>` | admin | search the local archive |
| `/aget <id>` | admin | download that post's media to disk |
| `/astats` | admin | archive stats (posts, media, data used) |
| `/aexport` | admin | write all posts to one offline HTML file |

## Assistant — save posts with little data

The assistant archives the **full text + caption + media info** of every post,
instantly, using almost no internet. Media *bytes* are only downloaded when you
actually want them (never by default).

- **Watch channels** — `/addchannel` a public or private channel you own/admin (add the bot as a channel admin so it receives posts). Every new post is saved locally.
- **Forward to save** — forward any post into the bot's private chat; it is archived automatically.
- **`/asave` on a reply** — save a single message in any chat on demand.
- Saved posts live in `DATA_DIR/assistant_posts.jsonl` (append-only, one post per line). Reading the archive, searching and exporting are **free** — no internet at all.
- **`/ashow <id>`** shows the full saved text + caption of a post so you can copy it — completely free, no download needed. Only media *bytes* (photos/videos/files) ever require a download (`/aget`).
- **`/aget <id>`** downloads just that one post's media to `DATA_DIR/assistant/media/` (respects `ASSISTANT_MAX_MEDIA_MB`).
- Set `ASSISTANT_SAVE_MEDIA=true` to auto-download media under the size cap as posts arrive.
- **`/aexport`** writes every saved post into a single offline HTML file — the complete archive, readable in any browser with zero data use.

## Automation (no commands needed)

- **Mirror** — every new message in `SOURCE_CHAT_ID` is forwarded to `TARGET_CHAT_ID` (text, media, everything). Set both IDs to enable.
- **Keyword replies** — when a plain message contains a keyword, the bot replies with the configured text (case-insensitive).
- **New-member greeting** — greets the chat when the bot is added to a group.

## Build & test

```sh
go build -o telegram-tui-bot .
go test ./...
```
