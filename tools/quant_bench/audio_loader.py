from __future__ import annotations

import math
import os
import threading
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Iterable, Optional

import numpy as np
import soundfile as sf
from scipy import signal

TARGET_SAMPLE_RATE = 24_000


class AudioLoader:
    """Load audio files with optional prefetching and caching."""

    def __init__(self, target_sample_rate: int = TARGET_SAMPLE_RATE, max_workers: Optional[int] = None) -> None:
        self.target_sample_rate = target_sample_rate
        self._cache: dict[Path, tuple[np.ndarray, float]] = {}
        self._cache_lock = threading.Lock()
        self._max_workers = max_workers

    def load(self, path: Path) -> tuple[np.ndarray, float]:
        """Load a single file, using the cache when available."""

        return self._load_and_cache(path)

    def warmup(self, paths: Iterable[Path]) -> None:
        """Load a batch of files in parallel before inference."""

        ordered_paths: list[Path] = []
        seen: set[Path] = set()
        for path in paths:
            resolved = path.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            ordered_paths.append(resolved)

        if not ordered_paths:
            return

        max_workers = self._max_workers or min(32, max(1, (os.cpu_count() or 1) * 5))
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            list(executor.map(self._load_and_cache, ordered_paths))

    def _load_and_cache(self, path: Path) -> tuple[np.ndarray, float]:
        resolved = path.resolve()
        with self._cache_lock:
            cached = self._cache.get(resolved)
        if cached is not None:
            return cached

        array, duration = self._read_file(resolved)

        with self._cache_lock:
            cached = self._cache.get(resolved)
            if cached is None:
                cached = (array, duration)
                self._cache[resolved] = cached
        return cached

    def _read_file(self, path: Path) -> tuple[np.ndarray, float]:
        data, sample_rate = sf.read(path)
        if data.ndim == 2:
            data = np.mean(data, axis=1)
        if sample_rate != self.target_sample_rate:
            data = self._resample(data, sample_rate)
        duration = float(len(data) / self.target_sample_rate)
        return np.ascontiguousarray(data.astype(np.float32)), duration

    def _resample(self, data: np.ndarray, sample_rate: int) -> np.ndarray:
        gcd = math.gcd(sample_rate, self.target_sample_rate)
        up = self.target_sample_rate // gcd
        down = sample_rate // gcd
        return signal.resample_poly(data, up, down)
