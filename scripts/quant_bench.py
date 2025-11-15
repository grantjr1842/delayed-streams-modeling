#!/usr/bin/env python3
"""CLI entry point for Kyutai quantization benchmarks."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Sequence

from quant_bench import STTBenchmark, STTConfig, TTSArguments, TTSBenchmark


def _add_common_output_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--json", type=Path, help="Optional JSON output file for structured metrics.")


def _add_stt_subcommand(subparsers) -> None:
    parser = subparsers.add_parser("stt", help="Run the CUDA STT benchmark.")
    parser.add_argument("audio_files", nargs="+", type=Path, help="Audio files to transcribe.")
    parser.add_argument("--model-id", default="kyutai/stt-2.6b-en-trfs", help="Transformers model identifier.")
    parser.add_argument("--bits", type=int, default=4, choices=(4, 8), help="Weight-only quantization precision.")
    parser.add_argument("--dtype", type=str, default=None, help="Optional torch dtype name (e.g. float16).")
    parser.add_argument("--device-map", default="auto", help="Device map passed to from_pretrained.")
    parser.add_argument("--max-new-tokens", type=int, default=None, help="Maximum generated tokens.")
    parser.add_argument("--temperature", type=float, default=0.0, help="Generation temperature.")
    parser.add_argument("--top-p", type=float, default=0.95, help="Top-p sampling value.")
    parser.add_argument("--beam-size", type=int, default=1, help="Beam search size.")
    parser.add_argument("--no-repeat-ngram-size", type=int, default=0, help="No repeat n-gram size.")
    parser.add_argument("--csv", type=Path, help="Optional CSV output path for inference metrics.")
    parser.add_argument(
        "--local-files-only",
        action="store_true",
        help="Only load models from the local cache without contacting Hugging Face.",
    )
    _add_common_output_args(parser)
    parser.set_defaults(func=_run_stt)


def _add_tts_subcommand(subparsers) -> None:
    parser = subparsers.add_parser("tts", help="Run the MLX TTS benchmark wrapper.")
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
    _add_common_output_args(parser)
    parser.set_defaults(func=_run_tts)


def _run_stt(args: argparse.Namespace) -> int:
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


def _run_tts(args: argparse.Namespace) -> int:
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


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    _add_stt_subcommand(subparsers)
    _add_tts_subcommand(subparsers)
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
