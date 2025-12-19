#!/usr/bin/env python3
"""Benchmark Kyutai STT models with bitsandbytes weight-only quantization.

This script loads the Transformers variant of the Kyutai STT model using
bitsandbytes 4-bit or 8-bit weight-only quantization and records
real-time factor (RTF) and GPU memory usage for both the loading phase and
per-file inference runs.
"""
from __future__ import annotations

import argparse
import csv
import json
import math
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Optional

import torch
from transformers import (
    BitsAndBytesConfig,
    KyutaiSpeechToTextForConditionalGeneration,
    KyutaiSpeechToTextProcessor,
)

from quant_bench.audio_loader import AudioLoader


TARGET_SAMPLE_RATE = 24_000


def _format_bytes(num_bytes: int) -> str:
    if num_bytes <= 0:
        return "0 B"
    units = ["B", "KiB", "MiB", "GiB", "TiB"]
    exponent = min(int(math.log(num_bytes, 1024)), len(units) - 1)
    value = num_bytes / (1024**exponent)
    return f"{value:.2f} {units[exponent]}"


@dataclass
class LoadMetrics:
    load_time_s: float
    peak_alloc_bytes: int
    peak_reserved_bytes: int

    def to_print_dict(self) -> dict[str, str]:
        return {
            "load_time_s": f"{self.load_time_s:.2f}",
            "peak_alloc": _format_bytes(self.peak_alloc_bytes),
            "peak_reserved": _format_bytes(self.peak_reserved_bytes),
        }


@dataclass
class InferenceMetrics:
    audio_path: str
    audio_sec: float
    generation_time_s: float
    total_time_s: float
    rtf_generation: float
    rtf_total: float
    peak_alloc_bytes: int
    peak_reserved_bytes: int
    transcript: str

    def to_print_dict(self) -> dict[str, str]:
        return {
            "audio": self.audio_path,
            "audio_sec": f"{self.audio_sec:.2f}",
            "gen_time_s": f"{self.generation_time_s:.2f}",
            "total_time_s": f"{self.total_time_s:.2f}",
            "rtf_gen": f"{self.rtf_generation:.2f}",
            "rtf_total": f"{self.rtf_total:.2f}",
            "peak_alloc": _format_bytes(self.peak_alloc_bytes),
            "peak_reserved": _format_bytes(self.peak_reserved_bytes),
        }


class CudaMemoryMonitor:
    def __init__(self, clear_cache: bool = True) -> None:
        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is required for this benchmark.")
        self._clear_cache = clear_cache
        self.start_alloc = 0
        self.start_reserved = 0

    def __enter__(self):
        torch.cuda.reset_peak_memory_stats()
        if self._clear_cache:
            torch.cuda.empty_cache()
        self.start_alloc = torch.cuda.memory_allocated()
        self.start_reserved = torch.cuda.memory_reserved()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        return False

    @property
    def peak(self) -> tuple[int, int]:
        return (
            torch.cuda.max_memory_allocated(),
            torch.cuda.max_memory_reserved(),
        )


@dataclass
class Config:
    model_id: str
    bits: int
    dtype: Optional[str]
    device_map: str
    max_new_tokens: Optional[int]
    temperature: float
    top_p: float
    beam_size: int
    no_repeat_ngram_size: int


@dataclass
class Arguments:
    audio_files: List[Path]
    csv_out: Optional[Path]
    config: Config


def parse_args() -> Arguments:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "audio_files",
        nargs="+",
        type=Path,
        help="Audio files to transcribe.",
    )
    parser.add_argument(
        "--model-id",
        default="kyutai/stt-2.6b-en-trfs",
        help="Transformers model identifier.",
    )
    parser.add_argument(
        "--bits",
        type=int,
        choices=(4, 8),
        default=4,
        help="Quantization precision in bits (weight-only).",
    )
    parser.add_argument(
        "--dtype",
        default=None,
        help="Override compute dtype (torch dtype string).",
    )
    parser.add_argument(
        "--device-map",
        default="auto",
        help="Device map passed to from_pretrained.",
    )
    parser.add_argument(
        "--max-new-tokens",
        type=int,
        default=None,
        help="Optional maximum number of tokens to generate.",
    )
    parser.add_argument(
        "--temperature",
        type=float,
        default=0.0,
        help="Sampling temperature.",
    )
    parser.add_argument(
        "--top-p",
        type=float,
        default=1.0,
        help="Top-p nucleus sampling parameter.",
    )
    parser.add_argument(
        "--beam-size",
        type=int,
        default=1,
        help="Beam search width.",
    )
    parser.add_argument(
        "--no-repeat-ngram-size",
        type=int,
        default=0,
        help="Disallow repeating ngrams of this size if > 0.",
    )
    parser.add_argument(
        "--csv",
        type=Path,
        default=None,
        help="Optional CSV output path for metrics.",
    )
    args = parser.parse_args()

    config = Config(
        model_id=args.model_id,
        bits=args.bits,
        dtype=args.dtype,
        device_map=args.device_map,
        max_new_tokens=args.max_new_tokens,
        temperature=args.temperature,
        top_p=args.top_p,
        beam_size=args.beam_size,
        no_repeat_ngram_size=args.no_repeat_ngram_size,
    )
    return Arguments(audio_files=args.audio_files, csv_out=args.csv, config=config)


def build_bnb_config(bits: int, dtype: Optional[str]) -> BitsAndBytesConfig:
    if bits == 4:
        return BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_use_double_quant=True,
            bnb_4bit_compute_dtype=getattr(torch, dtype) if dtype else torch.bfloat16,
        )
    if bits == 8:
        return BitsAndBytesConfig(
            load_in_8bit=True,
            llm_int8_enable_fp32_cpu_offload=False,
        )
    raise ValueError("bits must be 4 or 8")


def load_model(config: Config) -> tuple[
    KyutaiSpeechToTextProcessor, KyutaiSpeechToTextForConditionalGeneration, LoadMetrics
]:
    bnb_config = build_bnb_config(config.bits, config.dtype)
    dtype = getattr(torch, config.dtype) if config.dtype else "auto"

    processor = KyutaiSpeechToTextProcessor.from_pretrained(config.model_id)

    with CudaMemoryMonitor() as monitor:
        start = time.perf_counter()
        model = KyutaiSpeechToTextForConditionalGeneration.from_pretrained(
            config.model_id,
            quantization_config=bnb_config,
            torch_dtype=dtype,
            device_map=config.device_map,
        )
        torch.cuda.synchronize()
        load_time = time.perf_counter() - start
        peak_alloc, peak_reserved = monitor.peak

    metrics = LoadMetrics(load_time_s=load_time, peak_alloc_bytes=peak_alloc, peak_reserved_bytes=peak_reserved)
    return processor, model, metrics


def transcribe(
    audio_loader: AudioLoader,
    processor: KyutaiSpeechToTextProcessor,
    model: KyutaiSpeechToTextForConditionalGeneration,
    config: Config,
    audio_paths: Iterable[Path],
) -> List[InferenceMetrics]:
    results: List[InferenceMetrics] = []

    generation_kwargs = {
        "temperature": config.temperature,
        "top_p": config.top_p,
        "num_beams": config.beam_size,
    }
    if config.max_new_tokens is not None:
        generation_kwargs["max_new_tokens"] = config.max_new_tokens
    if config.no_repeat_ngram_size > 0:
        generation_kwargs["no_repeat_ngram_size"] = config.no_repeat_ngram_size

    for path in audio_paths:
        audio, duration = audio_loader.load(path)
        inputs = processor(audio, sampling_rate=TARGET_SAMPLE_RATE, return_tensors="pt")
        inputs = inputs.to(model.device)

        torch.cuda.synchronize()
        preprocess_done = time.perf_counter()
        with CudaMemoryMonitor(clear_cache=False) as monitor:
            gen_start_event = torch.cuda.Event(enable_timing=True)
            gen_end_event = torch.cuda.Event(enable_timing=True)
            gen_start_event.record()
            with torch.inference_mode():
                generated = model.generate(**inputs, **generation_kwargs)
            gen_end_event.record()
            gen_end_event.synchronize()
            gen_time = gen_start_event.elapsed_time(gen_end_event) / 1000
            peak_alloc, peak_reserved = monitor.peak
        total_time = time.perf_counter() - preprocess_done

        transcript = processor.batch_decode(generated, skip_special_tokens=True)[0]
        rtf_gen = gen_time / duration if duration > 0 else float("inf")
        rtf_total = total_time / duration if duration > 0 else float("inf")

        metrics = InferenceMetrics(
            audio_path=str(path),
            audio_sec=duration,
            generation_time_s=gen_time,
            total_time_s=total_time,
            rtf_generation=rtf_gen,
            rtf_total=rtf_total,
            peak_alloc_bytes=peak_alloc,
            peak_reserved_bytes=peak_reserved,
            transcript=transcript,
        )
        results.append(metrics)
    return results


def write_csv(path: Path, load_metrics: LoadMetrics, inference_metrics: List[InferenceMetrics]) -> None:
    fieldnames = [
        "audio",
        "audio_sec",
        "generation_time_s",
        "total_time_s",
        "rtf_generation",
        "rtf_total",
        "peak_alloc_bytes",
        "peak_reserved_bytes",
        "transcript",
    ]
    with path.open("w", newline="") as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=["phase", *fieldnames])
        writer.writeheader()
        writer.writerow(
            {
                "phase": "load",
                "audio": "model_load",
                "audio_sec": "",
                "generation_time_s": load_metrics.load_time_s,
                "total_time_s": load_metrics.load_time_s,
                "rtf_generation": "",
                "rtf_total": "",
                "peak_alloc_bytes": load_metrics.peak_alloc_bytes,
                "peak_reserved_bytes": load_metrics.peak_reserved_bytes,
                "transcript": json.dumps(load_metrics.to_print_dict()),
            }
        )
        for metrics in inference_metrics:
            writer.writerow(
                {
                    "phase": "inference",
                    "audio": metrics.audio_path,
                    "audio_sec": metrics.audio_sec,
                    "generation_time_s": metrics.generation_time_s,
                    "total_time_s": metrics.total_time_s,
                    "rtf_generation": metrics.rtf_generation,
                    "rtf_total": metrics.rtf_total,
                    "peak_alloc_bytes": metrics.peak_alloc_bytes,
                    "peak_reserved_bytes": metrics.peak_reserved_bytes,
                    "transcript": metrics.transcript,
                }
            )


def main() -> None:
    args = parse_args()
    audio_loader = AudioLoader()
    audio_loader.warmup(args.audio_files)

    processor, model, load_metrics = load_model(args.config)
    print("Model load metrics:")
    print(json.dumps(load_metrics.to_print_dict(), indent=2))

    inference_metrics = transcribe(audio_loader, processor, model, args.config, args.audio_files)

    print("\nInference metrics:")
    for metrics in inference_metrics:
        data = metrics.to_print_dict()
        print(json.dumps(data, indent=2))
        print(f"Transcript: {metrics.transcript}\n")

    if args.csv_out:
        write_csv(args.csv_out, load_metrics, inference_metrics)
        print(f"Metrics written to {args.csv_out}")


if __name__ == "__main__":
    main()
