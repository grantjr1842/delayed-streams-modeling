"""Text-to-speech quantization benchmark utilities."""
from __future__ import annotations

import json
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Sequence

import psutil
import soundfile as sf

DEFAULT_SCRIPT = Path("scripts/tts_mlx.py")


@dataclass
class TTSArguments:
    script: Path = DEFAULT_SCRIPT
    quantize: int = 8
    text: str = ""
    outfile: Path = Path("out.wav")
    voice: Optional[str] = None
    speaker: Optional[str] = None
    extra_args: Sequence[str] | None = None


@dataclass
class TTSMetrics:
    wall_time_s: float
    audio_sec: float
    rtf: float
    peak_rss_bytes: int
    stdout: str
    stderr: str

    def to_print_dict(self) -> dict[str, str]:
        return {
            "wall_time_s": f"{self.wall_time_s:.2f}",
            "audio_sec": f"{self.audio_sec:.2f}",
            "rtf": f"{self.rtf:.2f}",
            "peak_rss": f"{self.peak_rss_bytes / (1024 ** 2):.2f} MiB",
        }

    def to_json(self) -> dict[str, object]:
        return {
            "wall_time_s": self.wall_time_s,
            "audio_sec": self.audio_sec,
            "rtf": self.rtf,
            "peak_rss_bytes": self.peak_rss_bytes,
            "stdout": self.stdout,
            "stderr": self.stderr,
        }


class TTSBenchmark:
    def __init__(self, args: TTSArguments) -> None:
        self.args = args

    def ensure_script(self) -> Path:
        if not self.args.script.exists():
            raise FileNotFoundError(f"Cannot find MLX script at {self.args.script}.")
        return self.args.script

    def _build_command(self) -> list[str]:
        cmd = [
            sys.executable,
            str(self.ensure_script()),
            "-",
            str(self.args.outfile),
            "--quantize",
            str(self.args.quantize),
        ]
        if self.args.voice:
            cmd.extend(["--voice", self.args.voice])
        if self.args.speaker:
            cmd.extend(["--speaker", self.args.speaker])
        if self.args.extra_args:
            cmd.extend(self.args.extra_args)
        return cmd

    def run(self) -> TTSMetrics:
        cmd = self._build_command()
        self.args.outfile.parent.mkdir(parents=True, exist_ok=True)
        process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        ps_proc = psutil.Process(process.pid)
        peak_rss = 0
        stop_event = threading.Event()

        def monitor_memory() -> None:
            nonlocal peak_rss
            while not stop_event.is_set():
                try:
                    mem_info = ps_proc.memory_info()
                    peak_rss = max(peak_rss, mem_info.rss)
                except psutil.Error:
                    break
                time.sleep(0.05)

        monitor_thread = threading.Thread(target=monitor_memory, daemon=True)
        monitor_thread.start()

        start = time.perf_counter()
        try:
            stdout, stderr = process.communicate(self.args.text + "\n")
        finally:
            stop_event.set()
            monitor_thread.join(timeout=1.0)
            end = time.perf_counter()

        if process.returncode != 0:
            raise RuntimeError(
                "TTS script failed with exit code "
                f"{process.returncode}.\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
            )
        if not self.args.outfile.exists():
            raise FileNotFoundError(f"Expected output file {self.args.outfile} was not created.")

        audio, sample_rate = sf.read(self.args.outfile)
        frames = audio.shape[0]
        if audio.ndim == 2:
            frames = audio.shape[0]
        duration = frames / float(sample_rate) if sample_rate else 0.0
        wall_time = end - start
        rtf = wall_time / duration if duration > 0 else float("inf")

        return TTSMetrics(
            wall_time_s=wall_time,
            audio_sec=duration,
            rtf=rtf,
            peak_rss_bytes=peak_rss,
            stdout=stdout,
            stderr=stderr,
        )

    @staticmethod
    def dump_json(metrics: TTSMetrics, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("w") as f:
            json.dump(metrics.to_json(), f, indent=2)
