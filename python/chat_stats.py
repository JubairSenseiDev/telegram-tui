#!/usr/bin/env python3
"""Quick stats about a text chat export from telegram-tui.

Reads a chat export file and prints totals plus a breakdown by hour.
"""

import argparse
import re
import sys
from collections import Counter
from pathlib import Path

LINE_RE = re.compile(r"^\[(\d{4}-\d{2}-\d{2}) (\d{2}):(\d{2})\] (.*)$")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path)
    args = parser.parse_args()

    if not args.file.is_file():
        print(f"not a file: {args.file}", file=sys.stderr)
        sys.exit(1)

    total = 0
    hours = Counter()
    days = Counter()
    with args.file.open(encoding="utf-8", errors="replace") as fh:
        for line in fh:
            m = LINE_RE.match(line.strip())
            if not m:
                continue
            total += 1
            days[m.group(1)] += 1
            hours[m.group(2)] += 1

    print(f"messages: {total}")
    print(f"days:     {len(days)}")
    print(f"peak day: {days.most_common(1)[0] if days else ('-', 0)}")
    print("\nhourly activity:")
    for hour in range(24):
        key = f"{hour:02d}"
        bar = "#" * (hours[key] * 40 // max(hours.values(), default=1))
        print(f"  {key}:00 {hours[key]:5d} {bar}")


if __name__ == "__main__":
    main()
