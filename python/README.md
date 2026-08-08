# 🐍 Telegram TUI — Python tools

Export summary + chat statistics, using **only the standard library** (no dependencies).

| Script | What it does |
| --- | --- |
| `export_summary.py` | Summarize a dialogs/members CSV export from the TUI |
| `chat_stats.py` | Hourly / daily stats for a text chat export |

## Usage

```sh
python3 export_summary.py ~/.config/telegram-tui/exports/dialogs-*.csv
python3 chat_stats.py ~/.config/telegram-tui/exports/chat-*.txt
```

Both are runnable on any system with Python 3.8+.
