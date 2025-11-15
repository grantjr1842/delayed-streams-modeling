# /// script
# requires-python = ">=3.12"
# ///
#!/usr/bin/env python3
"""
One-shot helper that detects the active CUDA devices and runs the bf16->fp16
conversion workflow when a pre-Ampere GPU (SM75 and earlier) is detected or
when the operator explicitly forces the conversion.

Usage (installs deps via uv):

    uv run --with torch --with safetensors --with huggingface_hub \
        scripts/prep_sm75_assets.py

This script wraps `scripts/convert_bf16_to_fp16.py` so SM75 operators can keep
their workflow to a single command, and offers simulation/dry-run modes so CI
can exercise the logic without downloading checkpoints.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import List, Sequence

from check_gpu_capability import (
    DeviceCapability,
    detect_with_nvidia_smi,
    detect_with_torch,
    parse_simulated_devices,
)

CONVERTER_SCRIPT = Path(__file__).with_name("convert_bf16_to_fp16.py")
DEFAULT_OUTPUT = "assets/fp16/stt-1b-en_fr-candle.fp16.safetensors"
DEFAULT_HF_REPO = "kyutai/stt-1b-en_fr-candle"


def gather_devices(simulated: Sequence[str] | None) -> List[DeviceCapability]:
    if simulated:
        return parse_simulated_devices(simulated)
    devices = detect_with_torch()
    if not devices:
        devices = detect_with_nvidia_smi()
    return devices


def summarize_detection(devices: Sequence[DeviceCapability]) -> str:
    if not devices:
        return (
            "No CUDA devices detected automatically. "
            "Passing --simulate sm75 will force a pre-Ampere conversion workflow."
        )
    lines = ["Detected CUDA devices:"]
    for dev in devices:
        status = "PRE-AMPERE" if dev.is_pre_ampere else "Ampere+"
        lines.append(
            f"- #{dev.index} {dev.name} ({dev.sm_tag}, compute capability {dev.compute_capability:.1f}, {status})"
        )
    return "\n".join(lines)


def needs_conversion(
    devices: Sequence[DeviceCapability],
    force: bool,
    skip_when_undetected: bool,
) -> bool:
    if force:
        return True
    if devices:
        return any(dev.is_pre_ampere for dev in devices)
    return not skip_when_undetected


def build_converter_cmd(args: argparse.Namespace) -> List[str]:
    cmd = [sys.executable, str(CONVERTER_SCRIPT)]
    cmd.extend(["--hf-repo", args.hf_repo])
    cmd.extend(["--model-file", args.model_file])
    cmd.extend(["--output", args.output])
    if args.input_path:
        cmd.extend(["--input-path", args.input_path])
    if args.dtype:
        cmd.extend(["--dtype", args.dtype])
    return cmd


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Detect GPUs and run the bf16->fp16 conversion workflow automatically "
            "so SM75 operators can prepare assets with a single command."
        )
    )
    parser.add_argument(
        "--hf-repo",
        default=DEFAULT_HF_REPO,
        help=f"Hugging Face repo containing the bf16 checkpoint (default: {DEFAULT_HF_REPO}).",
    )
    parser.add_argument(
        "--model-file",
        default="model.safetensors",
        help="Filename inside the HF repo (default: model.safetensors).",
    )
    parser.add_argument(
        "--input-path",
        help="Optional local bf16 checkpoint to convert (skips the Hugging Face download).",
    )
    parser.add_argument(
        "--output",
        default=DEFAULT_OUTPUT,
        help=f"Destination path for the fp16 checkpoint (default: {DEFAULT_OUTPUT}).",
    )
    parser.add_argument(
        "--dtype",
        choices=("float16", "float32"),
        default="float16",
        help="Target dtype for converted tensors (default: float16).",
    )
    parser.add_argument(
        "--simulate",
        action="append",
        metavar="smXY",
        help=(
            "Simulate CUDA devices (e.g., --simulate sm75). "
            "Useful for CI and dry-runs on CPU-only hosts."
        ),
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Always run the conversion, even when Ampere GPUs are detected.",
    )
    parser.add_argument(
        "--skip-when-undetected",
        action="store_true",
        help=(
            "Skip the conversion when GPU detection fails. "
            "By default, the script converts anyway to keep SM75 assets ready."
        ),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Only print the converter command instead of executing it.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not CONVERTER_SCRIPT.exists():
        raise SystemExit(
            f"Converter script {CONVERTER_SCRIPT} is missing. "
            "Ensure you run this helper from the repo root."
        )
    devices = gather_devices(args.simulate)
    print(summarize_detection(devices), flush=True)
    if not needs_conversion(devices, args.force, args.skip_when_undetected):
        print(
            "No pre-Ampere GPUs detected and --force not set; skipping the conversion.",
            flush=True,
        )
        return 0
    cmd = build_converter_cmd(args)
    if args.dry_run:
        print("[dry-run] Would execute:", " ".join(cmd))
        return 0
    env = os.environ.copy()
    print("Running:", " ".join(cmd), flush=True)
    subprocess.run(cmd, check=True, env=env)
    print(
        f"fp16 checkpoint ready at {args.output}. "
        "Point configs/config-stt-en_fr-lowram-sm75.toml at this path before launching moshi-server.",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
