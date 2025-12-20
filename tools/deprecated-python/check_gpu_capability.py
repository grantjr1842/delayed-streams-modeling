# /// script
# requires-python = ">=3.12"
# ///
#!/usr/bin/env python3
"""
Inspect the available CUDA devices and warn operators when the GPU is
pre-Ampere (compute capability < 8.0), which cannot execute Kyutai's bf16
checkpoint without first converting it to fp16.

Typical usage (torch path, installs deps via uv):

    uv run --with torch scripts/check_gpu_capability.py

Use --simulate to exercise the warnings on a machine without CUDA:

    python scripts/check_gpu_capability.py --simulate sm75 --simulate sm90
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from typing import List, Sequence, Tuple

try:
    import torch  # type: ignore
except ImportError:  # pragma: no cover - optional dependency
    torch = None

AMPERE_MAJOR = 8
CONVERTER_CMD = (
    "uv run --with torch --with huggingface_hub --with safetensors "
    "scripts/convert_bf16_to_fp16.py "
    "--output assets/fp16/stt-1b-en_fr-candle.fp16.safetensors"
)
SM75_CONFIG = "configs/config-stt-en_fr-lowram-sm75.toml"


@dataclass
class DeviceCapability:
    index: int
    name: str
    major: int
    minor: int
    source: str

    @property
    def sm_tag(self) -> str:
        return f"sm{self.major}{self.minor}"

    @property
    def compute_capability(self) -> float:
        return self.major + self.minor / 10.0

    @property
    def is_pre_ampere(self) -> bool:
        return self.major < AMPERE_MAJOR


def parse_capability(raw: str) -> Tuple[int | None, int | None]:
    value = raw.strip().lower().replace(" ", "")
    if value.startswith("sm"):
        value = value[2:]
    if not value:
        return None, None
    if "." in value:
        major_str, minor_str = value.split(".", 1)
    else:
        if len(value) == 1:
            major_str, minor_str = value, "0"
        else:
            major_str, minor_str = value[:-1], value[-1]
    if not major_str.isdigit() or not minor_str.isdigit():
        return None, None
    return int(major_str), int(minor_str)


def detect_with_torch() -> List[DeviceCapability]:
    if torch is None or not torch.cuda.is_available():
        return []
    devices: List[DeviceCapability] = []
    for idx in range(torch.cuda.device_count()):
        major, minor = torch.cuda.get_device_capability(idx)
        devices.append(
            DeviceCapability(
                index=idx,
                name=torch.cuda.get_device_name(idx),
                major=major,
                minor=minor,
                source="torch",
            )
        )
    return devices


def detect_with_nvidia_smi() -> List[DeviceCapability]:
    if shutil.which("nvidia-smi") is None:
        return []
    cmd = [
        "nvidia-smi",
        "--query-gpu=name,compute_cap",
        "--format=csv,noheader",
    ]
    try:
        output = subprocess.check_output(cmd, text=True, timeout=5)
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return []
    devices: List[DeviceCapability] = []
    for idx, line in enumerate(output.strip().splitlines()):
        line = line.strip()
        if not line:
            continue
        parts = [part.strip() for part in line.split(",", maxsplit=1)]
        if len(parts) != 2:
            continue
        name, cap = parts
        major, minor = parse_capability(cap)
        if major is None or minor is None:
            continue
        devices.append(
            DeviceCapability(
                index=idx,
                name=name,
                major=major,
                minor=minor,
                source="nvidia-smi",
            )
        )
    return devices


def parse_simulated_devices(values: Sequence[str]) -> List[DeviceCapability]:
    devices: List[DeviceCapability] = []
    for idx, raw in enumerate(values):
        if ":" in raw:
            sm_str, name = raw.split(":", 1)
            name = name.strip() or f"Simulated {sm_str.strip()}"
        else:
            sm_str, name = raw, f"Simulated {raw.strip()}"
        major, minor = parse_capability(sm_str)
        if major is None or minor is None:
            raise ValueError(f"Invalid --simulate value '{raw}'. Expected formats: sm75, 8.0, 86:Name")
        devices.append(
            DeviceCapability(
                index=idx,
                name=name,
                major=major,
                minor=minor,
                source="simulate",
            )
        )
    return devices


def summarize_devices(devices: Sequence[DeviceCapability]) -> str:
    if not devices:
        return (
            "No CUDA-capable GPU detected (torch import and nvidia-smi both "
            "failed). Run this helper on the machine that executes moshi-server "
            "or supply --simulate to test the warnings locally."
        )
    lines = [f"Detected {len(devices)} CUDA device(s):"]
    for dev in devices:
        status = "PRE-AMPERE" if dev.is_pre_ampere else "Ampere+"
        lines.append(
            f"- #{dev.index} {dev.name} [{dev.sm_tag}] "
            f"(compute capability {dev.compute_capability:.1f}, source={dev.source}, {status})"
        )
    risky = [dev for dev in devices if dev.is_pre_ampere]
    if risky:
        lines.append("")
        lines.append(
            "The devices above with compute capability < 8.0 cannot load the "
            "bf16 checkpoint. Convert the model first and switch to the SM75 config:"
        )
        lines.append(f"  1. {CONVERTER_CMD}")
        lines.append(f"  2. Use {SM75_CONFIG} so moshi-server forces float16 weights.")
    return "\n".join(lines)


def devices_to_json(devices: Sequence[DeviceCapability]) -> str:
    payload = [
        {
            "index": dev.index,
            "name": dev.name,
            "sm_tag": dev.sm_tag,
            "compute_capability": dev.compute_capability,
            "is_pre_ampere": dev.is_pre_ampere,
            "source": dev.source,
        }
        for dev in devices
    ]
    return json.dumps(payload, indent=2)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Warn operators when the active CUDA device predates Ampere so they "
            "know to run scripts/convert_bf16_to_fp16.py before launching moshi-server."
        )
    )
    parser.add_argument(
        "--simulate",
        action="append",
        metavar="SM",
        help=(
            "Simulate a GPU with the provided compute capability (e.g. sm75, 8.6, "
            "75:Tesla T4). When present, simulated entries replace auto-detected devices."
        ),
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON instead of a friendly summary.",
    )
    parser.add_argument(
        "--fail-on-pre-ampere",
        action="store_true",
        help="Exit with status 2 if any detected GPU requires the fp16 conversion.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.simulate:
        try:
            devices = parse_simulated_devices(args.simulate)
        except ValueError as exc:
            print(exc, file=sys.stderr)
            return 1
    else:
        devices = detect_with_torch()
        if not devices:
            devices = detect_with_nvidia_smi()
    if args.json:
        print(devices_to_json(devices))
    else:
        print(summarize_devices(devices))
    if args.fail_on_pre_ampere and any(dev.is_pre_ampere for dev in devices):
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
