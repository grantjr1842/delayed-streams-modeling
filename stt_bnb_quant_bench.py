#!/usr/bin/env python3
"""Backward-compatible CLI for the CUDA STT quantization benchmark."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from quant_bench import STTBenchmark, STTConfig


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("audio_files", nargs="+", type=Path, help="Audio files to transcribe.")
    parser.add_argument("--model-id", default="kyutai/stt-2.6b-en-trfs", help="Transformers model identifier.")
    parser.add_argument("--bits", type=int, choices=(4, 8), default=4, help="Weight-only quantization precision.")
    parser.add_argument("--dtype", type=str, default=None, help="Optional torch dtype name (e.g. float16).")
    parser.add_argument("--device-map", default="auto", help="Device map passed to from_pretrained.")
    parser.add_argument("--max-new-tokens", type=int, default=None, help="Maximum generated tokens.")
    parser.add_argument("--temperature", type=float, default=0.0, help="Generation temperature.")
    parser.add_argument("--top-p", type=float, default=0.95, help="Top-p sampling value.")
    parser.add_argument("--beam-size", type=int, default=1, help="Beam search size.")
    parser.add_argument("--no-repeat-ngram-size", type=int, default=0, help="No repeat n-gram size.")
    parser.add_argument("--csv", type=Path, help="Optional CSV output path for inference metrics.")
    parser.add_argument("--json", type=Path, help="Optional JSON output path for combined metrics.")
    parser.add_argument(
        "--local-files-only",
        action="store_true",
        help="Only load models from the local cache without contacting Hugging Face.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    config = STTConfig(
        model_id=args.model_id,
        bits=args.bits,
        dtype=args.dtype,
        device_map=args.device_map,
        max_new_tokens=args.max_new_tokens,
        temperature=args.temperature,
        top_p=args.top_p,
        beam_size=args.beam_size,
        no_repeat_ngram_size=args.no_repeat_ngram_size,
        local_files_only=args.local_files_only,
    )
    benchmark = STTBenchmark(config)
    load_metrics, inference_metrics = benchmark.run(
        audio_files=args.audio_files,
        csv_out=args.csv,
        json_out=args.json,
    )
    printable = {
        "load": load_metrics.to_print_dict(),
        "inference": [metric.to_print_dict() for metric in inference_metrics],
    }
    print(json.dumps(printable, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
