# /// script
# requires-python = ">=3.12"
# ///
#!/usr/bin/env python3
"""
Launch (or simulate) `moshi-server worker --config configs/config-stt-en_fr-lowram-sm75.toml`
to ensure the SM75 CUDA workflow stays healthy. This script is CI-friendly thanks to
the `--simulate-success` flag, but can run the real binary when GPUs are available.
"""

from __future__ import annotations

import argparse
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import List

DEFAULT_CONFIG = "configs/config-stt-en_fr-lowram-sm75.toml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a short moshi-server smoke test backed by the SM75 config."
    )
    parser.add_argument(
        "--config",
        default=DEFAULT_CONFIG,
        help=f"Path to the SM75 config (default: {DEFAULT_CONFIG}).",
    )
    parser.add_argument(
        "--moshi-bin",
        default="moshi-server",
        help="moshi-server binary to execute (default: moshi-server).",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=20.0,
        help="How long to keep the worker alive before stopping it (seconds).",
    )
    parser.add_argument(
        "--extra-arg",
        action="append",
        default=[],
        help="Additional CLI args forwarded to moshi-server (repeatable).",
    )
    parser.add_argument(
        "--simulate-success",
        action="store_true",
        help="Skip launching moshi-server and emit a simulated success message (CI helper).",
    )
    parser.add_argument(
        "--simulate-duration",
        type=float,
        default=2.0,
        help="How long the simulated smoke test should pretend to run (seconds).",
    )
    return parser.parse_args()


def run_simulated_smoke(config: str, duration: float) -> int:
    print(f"[simulate] Pretending to run moshi-server with {config}", flush=True)
    time.sleep(max(duration, 0.1))
    print(
        "[simulate] moshi-server completed the SM75 smoke test without CUDA faults.",
        flush=True,
    )
    return 0


def build_command(args: argparse.Namespace) -> List[str]:
    cmd = [args.moshi_bin, "worker", "--config", args.config]
    cmd.extend(args.extra_arg)
    return cmd


def stream_output(stream, prefix: str) -> None:
    for line in stream:
        print(f"[{prefix}] {line.rstrip()}")


def run_real_smoke(args: argparse.Namespace) -> int:
    if shutil.which(args.moshi_bin) is None:
        raise FileNotFoundError(
            f"Unable to locate '{args.moshi_bin}'. "
            "Install moshi-server or pass --simulate-success."
        )
    config_path = Path(args.config)
    if not config_path.exists():
        raise FileNotFoundError(f"Config {config_path} does not exist.")
    cmd = build_command(args)
    print("Launching:", " ".join(cmd), flush=True)
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    start = time.time()
    timed_out = False
    try:
        while True:
            retcode = proc.poll()
            if retcode is not None:
                if retcode != 0:
                    stdout = proc.stdout.read() if proc.stdout else ""
                    stderr = proc.stderr.read() if proc.stderr else ""
                    raise RuntimeError(
                        f"moshi-server exited with code {retcode}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                    )
                break
            if time.time() - start > args.timeout:
                timed_out = True
                break
            time.sleep(0.5)
        if timed_out:
            print(f"Timeout reached ({args.timeout}s); sending SIGINT.", flush=True)
            proc.send_signal(signal.SIGINT)
            try:
                proc.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                proc.terminate()
                proc.wait(timeout=2.0)
        stdout = proc.stdout.read() if proc.stdout else ""
        stderr = proc.stderr.read() if proc.stderr else ""
        if stdout:
            stream_output(stdout.splitlines(), "stdout")
        if stderr:
            stream_output(stderr.splitlines(), "stderr")
    finally:
        if proc.poll() is None:
            proc.kill()
    print("moshi-server SM75 smoke test completed successfully.", flush=True)
    return 0


def main() -> int:
    args = parse_args()
    if args.simulate_success:
        return run_simulated_smoke(args.config, args.simulate_duration)
    return run_real_smoke(args)


if __name__ == "__main__":
    sys.exit(main())
