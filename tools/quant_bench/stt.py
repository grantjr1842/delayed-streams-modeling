"""Speech-to-text quantization benchmark utilities."""
from __future__ import annotations

import csv
import json
import math
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Sequence

import torch
from transformers import (
    BitsAndBytesConfig,
    KyutaiSpeechToTextForConditionalGeneration,
    KyutaiSpeechToTextProcessor,
)

from .audio_loader import AudioLoader

TARGET_SAMPLE_RATE = 24_000


def _format_bytes(num_bytes: int) -> str:
    if num_bytes <= 0:
        return "0 B"
    units = ["B", "KiB", "MiB", "GiB", "TiB"]
    exponent = min(int(math.log(num_bytes, 1024)), len(units) - 1)
    value = num_bytes / (1024**exponent)
    return f"{value:.2f} {units[exponent]}"


@dataclass
class STTConfig:
    """Configuration for running the STT benchmark."""

    model_id: str = "kyutai/stt-2.6b-en-trfs"
    bits: int = 4
    dtype: Optional[str] = None
    device_map: str = "auto"
    max_new_tokens: Optional[int] = None
    temperature: float = 0.0
    top_p: float = 0.95
    beam_size: int = 1
    no_repeat_ngram_size: int = 0
    local_files_only: bool = False

    def as_dict(self) -> dict:
        return {
            "model_id": self.model_id,
            "bits": self.bits,
            "dtype": self.dtype,
            "device_map": self.device_map,
            "max_new_tokens": self.max_new_tokens,
            "temperature": self.temperature,
            "top_p": self.top_p,
            "beam_size": self.beam_size,
            "no_repeat_ngram_size": self.no_repeat_ngram_size,
            "local_files_only": self.local_files_only,
        }


@dataclass
class STTLoadMetrics:
    load_time_s: float
    peak_alloc_bytes: int
    peak_reserved_bytes: int

    def to_print_dict(self) -> dict[str, str]:
        return {
            "load_time_s": f"{self.load_time_s:.2f}",
            "peak_alloc": _format_bytes(self.peak_alloc_bytes),
            "peak_reserved": _format_bytes(self.peak_reserved_bytes),
        }

    def to_json(self) -> dict[str, float]:
        return {
            "load_time_s": self.load_time_s,
            "peak_alloc_bytes": self.peak_alloc_bytes,
            "peak_reserved_bytes": self.peak_reserved_bytes,
        }


@dataclass
class STTInferenceMetrics:
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
            "transcript": self.transcript,
        }

    def to_json(self) -> dict[str, object]:
        return {
            "audio_path": self.audio_path,
            "audio_sec": self.audio_sec,
            "generation_time_s": self.generation_time_s,
            "total_time_s": self.total_time_s,
            "rtf_generation": self.rtf_generation,
            "rtf_total": self.rtf_total,
            "peak_alloc_bytes": self.peak_alloc_bytes,
            "peak_reserved_bytes": self.peak_reserved_bytes,
            "transcript": self.transcript,
        }


class CudaMemoryMonitor:
    def __enter__(self):
        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is required for this benchmark.")
        torch.cuda.reset_peak_memory_stats()
        torch.cuda.empty_cache()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        return False

    @property
    def peak(self) -> tuple[int, int]:
        torch.cuda.synchronize()
        return (
            torch.cuda.max_memory_allocated(),
            torch.cuda.max_memory_reserved(),
        )


class STTBenchmark:
    def __init__(self, config: STTConfig) -> None:
        self.config = config
        self.audio_loader = AudioLoader()

    def _create_bnb_config(self) -> BitsAndBytesConfig:
        if self.config.bits not in (4, 8):
            raise ValueError("bits must be either 4 or 8.")
        kwargs = {
            "load_in_4bit": self.config.bits == 4,
            "load_in_8bit": self.config.bits == 8,
        }
        if self.config.bits == 4:
            kwargs.update(
                {
                    "bnb_4bit_quant_type": "nf4",
                    "bnb_4bit_use_double_quant": True,
                    "bnb_4bit_compute_dtype": torch.bfloat16,
                }
            )
        return BitsAndBytesConfig(**kwargs)

    def _load_model(self) -> tuple[
        KyutaiSpeechToTextProcessor,
        KyutaiSpeechToTextForConditionalGeneration,
        STTLoadMetrics,
    ]:
        dtype = None
        if self.config.dtype:
            try:
                dtype = getattr(torch, self.config.dtype)
            except AttributeError as exc:
                raise ValueError(f"Invalid torch dtype: {self.config.dtype}") from exc

        with CudaMemoryMonitor() as monitor:
            start_time = time.perf_counter()
            processor = KyutaiSpeechToTextProcessor.from_pretrained(
                self.config.model_id,
                local_files_only=self.config.local_files_only,
            )
            model = KyutaiSpeechToTextForConditionalGeneration.from_pretrained(
                self.config.model_id,
                device_map=self.config.device_map,
                quantization_config=self._create_bnb_config(),
                torch_dtype="auto" if dtype is None else dtype,
                local_files_only=self.config.local_files_only,
            )
            load_time = time.perf_counter() - start_time
            peak_alloc, peak_reserved = monitor.peak
            load_metrics = STTLoadMetrics(load_time, peak_alloc, peak_reserved)
        return processor, model, load_metrics

    def _generate(self, model, processor, audio: np.ndarray) -> tuple[str, float]:
        inputs = processor(audio, sampling_rate=TARGET_SAMPLE_RATE, return_tensors="pt")
        inputs = inputs.to(model.device)
        gen_kwargs = {
            "temperature": self.config.temperature,
            "top_p": self.config.top_p,
            "num_beams": self.config.beam_size,
            "no_repeat_ngram_size": self.config.no_repeat_ngram_size,
        }
        if self.config.max_new_tokens:
            gen_kwargs["max_new_tokens"] = self.config.max_new_tokens
        start_time = time.perf_counter()
        tokens = model.generate(**inputs, **gen_kwargs)
        gen_time = time.perf_counter() - start_time
        transcript = processor.batch_decode(tokens, skip_special_tokens=True)[0]
        return transcript, gen_time

    def run(
        self,
        audio_files: Sequence[Path],
        csv_out: Optional[Path] = None,
        json_out: Optional[Path] = None,
    ) -> tuple[STTLoadMetrics, list[STTInferenceMetrics]]:
        processor, model, load_metrics = self._load_model()
        self.audio_loader.warmup(audio_files)
        metrics: list[STTInferenceMetrics] = []
        csv_rows: list[dict[str, object]] = []

        for audio_path in audio_files:
            audio, duration = self.audio_loader.load(audio_path)
            with CudaMemoryMonitor() as monitor:
                preprocess_start = time.perf_counter()
                transcript, gen_time = self._generate(model, processor, audio)
                total_time = time.perf_counter() - preprocess_start
                peak_alloc, peak_reserved = monitor.peak
            metrics.append(
                STTInferenceMetrics(
                    audio_path=str(audio_path),
                    audio_sec=duration,
                    generation_time_s=gen_time,
                    total_time_s=total_time,
                    rtf_generation=gen_time / duration if duration > 0 else float("inf"),
                    rtf_total=total_time / duration if duration > 0 else float("inf"),
                    peak_alloc_bytes=peak_alloc,
                    peak_reserved_bytes=peak_reserved,
                    transcript=transcript,
                )
            )
            csv_rows.append(metrics[-1].to_json())

        if csv_out and csv_rows:
            csv_out.parent.mkdir(parents=True, exist_ok=True)
            with csv_out.open("w", newline="") as f:
                writer = csv.DictWriter(f, fieldnames=csv_rows[0].keys())
                writer.writeheader()
                writer.writerows(csv_rows)

        if json_out:
            json_out.parent.mkdir(parents=True, exist_ok=True)
            with json_out.open("w") as f:
                json.dump(
                    {
                        "config": self.config.as_dict(),
                        "load_metrics": load_metrics.to_json(),
                        "inference": [m.to_json() for m in metrics],
                    },
                    f,
                    indent=2,
                )

        return load_metrics, metrics
