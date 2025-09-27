#!/usr/bin/env python3
"""Backward-compatible CLI for the MLX TTS quantization benchmark."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from quant_bench import TTSArguments, TTSBenchmark


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--script", type=Path, default=Path("scripts/tts_mlx.py"), help="Path to tts_mlx.py script.")
    parser.add_argument("--quantize", type=int, choices=(4, 8), default=8, help="Quantization precision passed to MLX script.")
    parser.add_argument("--text", required=True, help="Text to synthesize.")
    parser.add_argument("--outfile", type=Path, required=True, help="Output WAV file path.")
    parser.add_argument("--voice", type=str, default=None, help="Optional voice argument passed to MLX script.")
    parser.add_argument("--speaker", type=str, default=None, help="Optional speaker argument passed to MLX script.")
    parser.add_argument(
        "--extra-args",
        nargs=argparse.REMAINDER,
        help="Additional arguments forwarded to the MLX script (e.g. --rate 1.1).",
    )
    parser.add_argument("--json", type=Path, help="Optional JSON output file for structured metrics.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    benchmark_args = TTSArguments(
        script=args.script,
        quantize=args.quantize,
        text=args.text,
        outfile=args.outfile,
        voice=args.voice,
        speaker=args.speaker,
        extra_args=args.extra_args,
    )
    benchmark = TTSBenchmark(benchmark_args)
    metrics = benchmark.run()
    if args.json:
        TTSBenchmark.dump_json(metrics, args.json)
    print(json.dumps(metrics.to_print_dict(), indent=2))
    if metrics.stdout:
        print("\n--- Child STDOUT ---\n" + metrics.stdout)
    if metrics.stderr:
        print("\n--- Child STDERR ---\n" + metrics.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
