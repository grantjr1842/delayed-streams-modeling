#!/usr/bin/env python3
"""Wrapper around Kyutai's MLX TTS script with quantization benchmarking.

The script launches ``scripts/tts_mlx.py`` in a subprocess, feeds text via
stdin, and records wall-clock time, produced audio duration, and peak RSS
(memory) of the child process. It is intended for Apple Silicon where MLX
quantization is supported.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import List, Optional

import psutil
import soundfile as sf


DEFAULT_SCRIPT = Path("scripts/tts_mlx.py")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--script", type=Path, default=DEFAULT_SCRIPT, help="Path to tts_mlx.py script.")
    parser.add_argument("--quantize", type=int, choices=(4, 8), default=8, help="Quantization precision for MLX script.")
    parser.add_argument("--text", required=True, help="Text to synthesize.")
    parser.add_argument("--outfile", type=Path, required=True, help="Output WAV file path.")
    parser.add_argument("--voice", type=str, default=None, help="Optional voice argument passed to the script.")
    parser.add_argument("--speaker", type=str, default=None, help="Optional speaker argument passed to the script.")
    parser.add_argument(
        "--extra-args",
        nargs=argparse.REMAINDER,
        help="Additional arguments forwarded to the MLX script (e.g. --rate 1.1).",
    )
    return parser.parse_args()


def ensure_script(script_path: Path) -> Path:
    if not script_path.exists():
        raise FileNotFoundError(f"Cannot find MLX script at {script_path}.")
    return script_path


def run_tts(
    script_path: Path,
    text: str,
    outfile: Path,
    quantize: int,
    voice: Optional[str],
    speaker: Optional[str],
    extra_args: Optional[List[str]],
) -> dict:
    cmd = [sys.executable, str(script_path), "-", str(outfile), "--quantize", str(quantize)]
    if voice:
        cmd.extend(["--voice", voice])
    if speaker:
        cmd.extend(["--speaker", speaker])
    if extra_args:
        cmd.extend(extra_args)

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
        out, err = process.communicate(text + "\n")
    finally:
        stop_event.set()
        monitor_thread.join(timeout=1.0)
        end = time.perf_counter()

    if process.returncode != 0:
        raise RuntimeError(
            f"TTS script failed with exit code {process.returncode}.\nSTDOUT:\n{out}\nSTDERR:\n{err}"
        )

    if not outfile.exists():
        raise FileNotFoundError(f"Expected output file {outfile} was not created.")

    audio, sample_rate = sf.read(outfile)
    duration = audio.shape[0] / float(sample_rate)
    if audio.ndim == 2:
        duration = len(audio) / float(sample_rate)

    wall_time = end - start
    rtf = wall_time / duration if duration > 0 else float("inf")

    return {
        "wall_time_s": wall_time,
        "audio_sec": duration,
        "rtf": rtf,
        "peak_rss_bytes": peak_rss,
        "stdout": out,
        "stderr": err,
    }


def main() -> None:
    args = parse_args()
    script_path = ensure_script(args.script)
    outfile = args.outfile
    outfile.parent.mkdir(parents=True, exist_ok=True)

    metrics = run_tts(
        script_path=script_path,
        text=args.text,
        outfile=outfile,
        quantize=args.quantize,
        voice=args.voice,
        speaker=args.speaker,
        extra_args=args.extra_args,
    )

    printable = {
        "wall_time_s": f"{metrics['wall_time_s']:.2f}",
        "audio_sec": f"{metrics['audio_sec']:.2f}",
        "rtf": f"{metrics['rtf']:.2f}",
        "peak_rss": f"{metrics['peak_rss_bytes'] / (1024 ** 2):.2f} MiB",
    }
    print(json.dumps(printable, indent=2))

    if metrics["stdout"]:
        print("\n--- Child STDOUT ---\n" + metrics["stdout"])
    if metrics["stderr"]:
        print("\n--- Child STDERR ---\n" + metrics["stderr"])


if __name__ == "__main__":
    main()
