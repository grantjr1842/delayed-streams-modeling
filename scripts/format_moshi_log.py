#!/usr/bin/env python3
"""Generate a readable Moshi log from a raw trace."""

from __future__ import annotations

import argparse
import datetime
import pathlib
import re
import sys
from typing import Iterable, Iterator

ANSI_ESCAPE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Strip ANSI junk, convert UTC timestamps to the operator's local timezone, "
            "and normalize characters before writing a friendly Moshi log."
        )
    )
    parser.add_argument(
        "input",
        help="Raw Moshi log (use '-' to read from stdin).",
    )
    parser.add_argument(
        "-o",
        "--output",
        help=(
            "Output path for the formatted log; defaults to the parent of the "
            "'raw' directory so the sanitized log lives alongside the raw trace."
        ),
    )
    parser.add_argument(
        "--strip-raw",
        action="store_true",
        help="Trim whitespace from lines that do not start with a timestamp.",
    )
    return parser.parse_args()


def lines_from_input(path: str) -> Iterator[str]:
    if path == "-":
        yield from sys.stdin
        return
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            yield line


def infer_output_path(input_path: pathlib.Path) -> pathlib.Path:
    if input_path.parent.name == "raw" and input_path.parent.parent.exists():
        return input_path.parent.parent / input_path.name
    return input_path.with_name(f"friendly-{input_path.name}")


def sanitize_text(line: str) -> str:
    text = ANSI_ESCAPE.sub("", line)
    text = text.replace("\r", "")
    return "".join(ch if ch.isprintable() else " " for ch in text).rstrip()


def parse_timestamp(token: str) -> datetime.datetime | None:
    token = token.rstrip().strip()
    if token.endswith("Z"):
        token = token[:-1] + "+00:00"
    try:
        return datetime.datetime.fromisoformat(token)
    except ValueError:
        return None


def format_timestamp(dt: datetime.datetime) -> str:
    local = dt.astimezone()
    ms = local.microsecond // 1000
    base = local.strftime("%Y-%m-%d %I:%M:%S")
    suffix = local.strftime("%p %Z (%z)")
    return f"{base}.{ms:03d} {suffix}"


def reformat_line(line: str, strip_raw: bool) -> str:
    sanitized = sanitize_text(line)
    if not sanitized:
        return ""
    tokens = sanitized.split(None, 3)
    if len(tokens) < 3:
        return sanitized if not strip_raw else sanitized.strip()
    timestamp_str, level, target = tokens[:3]
    rest = tokens[3] if len(tokens) == 4 else ""
    dt = parse_timestamp(timestamp_str)
    if dt is None:
        return sanitized if not strip_raw else sanitized.strip()
    target = target.rstrip(":")
    formatted_ts = format_timestamp(dt)
    body = rest.strip()
    parts = [f"[{formatted_ts}]", f"[{level.upper()}]", f"[{target}]"]
    if body:
        parts.append(body)
    return " ".join(parts)


def write_output(lines: Iterable[str], destination: pathlib.Path | None) -> None:
    if destination is None:
        for line in lines:
            print(line)
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    with open(destination, "w", encoding="utf-8", newline="\n") as fh:
        for line in lines:
            fh.write(line)
            fh.write("\n")


def format_log(
    input_path: str, output_path: pathlib.Path | None, strip_raw: bool
) -> Iterator[str]:
    for line in lines_from_input(input_path):
        yield reformat_line(line, strip_raw)


def main() -> int:
    args = parse_args()
    output_path = (
        pathlib.Path(args.output) if args.output and args.output != "-" else None
    )
    strip_raw = args.strip_raw
    if output_path is None and args.input != "-":
        output_path = infer_output_path(pathlib.Path(args.input))
    formatted_lines = format_log(args.input, output_path, strip_raw)
    write_output(formatted_lines, output_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
