#!/usr/bin/env python3
"""Summarize a telegram-tui export file (CSV dialogs/members or plain text chat).

Usage:
    python3 export_summary.py <file> [--top N]
"""

import argparse
import csv
import re
import sys
from collections import Counter
from pathlib import Path

LINE_RE = re.compile(r"^\[(\d{4}-\d{2}-\d{2}) (\d{2}):(\d{2})\] (.*)$")


def summarize_text(path: Path, top: int) -> None:
    senders = Counter()
    total = 0
    with path.open(encoding="utf-8", errors="replace") as fh:
        for line in fh:
            m = LINE_RE.match(line.strip())
            if not m:
                continue
            total += 1
            body = m.group(4)
            sender = body.split(":", 1)[0].strip() or "?"
            senders[sender] += 1
    print(f"file:   {path.name}")
    print(f"type:   text chat export")
    print(f"lines:  {total}")
    print(f"senders:{len(senders)}")
    print("\ntop senders:")
    for name, count in senders.most_common(top):
        print(f"  {count:6d}  {name}")


def summarize_csv(path: Path, top: int) -> None:
    with path.open(encoding="utf-8", errors="replace", newline="") as fh:
        reader = csv.reader(fh)
        try:
            header = next(reader)
        except StopIteration:
            print("empty CSV")
            return
    rows = []
    with path.open(encoding="utf-8", errors="replace", newline="") as fh:
        rows = list(csv.reader(fh))[1:]

    kind_idx = header.index("type") if "type" in header else None
    kinds = Counter()
    for row in rows:
        if kind_idx is not None and len(row) > kind_idx:
            kinds[row[kind_idx]] += 1
    print(f"file:   {path.name}")
    print(f"type:   csv ({','.join(header)})")
    print(f"rows:   {len(rows)}")
    if kinds:
        print("\nby type:")
        for kind, count in kinds.most_common(top):
            print(f"  {count:6d}  {kind}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path, help="path to an export file")
    parser.add_argument("--top", type=int, default=10, help="rows to show (default 10)")
    args = parser.parse_args()

    if not args.file.is_file():
        print(f"not a file: {args.file}", file=sys.stderr)
        sys.exit(1)
    if args.file.suffix.lower() == ".csv":
        summarize_csv(args.file, args.top)
    else:
        summarize_text(args.file, args.top)


if __name__ == "__main__":
    main()
