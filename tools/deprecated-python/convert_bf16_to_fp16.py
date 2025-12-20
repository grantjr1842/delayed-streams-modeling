#!/usr/bin/env python3
"""
Convert the Kyutai STT Candle checkpoint from BF16 to FP16/FP32 so
pre-Ampere CUDA GPUs (compute capability < 8.0) can run the model.

Usage (installs deps via uv):
    uv run --with torch --with huggingface_hub --with safetensors \
        scripts/convert_bf16_to_fp16.py \
        --hf-repo kyutai/stt-1b-en_fr-candle \
        --model-file model.safetensors \
        --output assets/fp16/stt-1b-en_fr-candle.fp16.safetensors
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Dict

import torch
from huggingface_hub import hf_hub_download
from safetensors.torch import load_file, save_file

BF16 = torch.bfloat16
DTYPE_MAP = {
    "float16": torch.float16,
    "float32": torch.float32,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Download Kyutai's Candle checkpoint (bf16) and re-save it with"
            " tensors converted to float16/float32 so SM75 GPUs stop hitting"
            " CUDA_ERROR_NOT_FOUND when Candle tries to load bf16 kernels."
        )
    )
    parser.add_argument(
        "--hf-repo",
        default="kyutai/stt-1b-en_fr-candle",
        help="Hugging Face repo id that hosts the source checkpoint.",
    )
    parser.add_argument(
        "--model-file",
        default="model.safetensors",
        help="Model file inside the repo (bf16 safetensors).",
    )
    parser.add_argument(
        "--input-path",
        help="Optional local path to the bf16 checkpoint (skips HF download).",
    )
    parser.add_argument(
        "--output",
        default="assets/fp16/stt-1b-en_fr-candle.fp16.safetensors",
        help="Destination path for the converted safetensors file.",
    )
    parser.add_argument(
        "--dtype",
        choices=DTYPE_MAP.keys(),
        default="float16",
        help="Target dtype for bf16 tensors.",
    )
    return parser.parse_args()


def resolve_source(args: argparse.Namespace) -> Path:
    if args.input_path:
        src = Path(args.input_path).expanduser()
        if not src.exists():
            raise FileNotFoundError(f"Input checkpoint {src} does not exist")
        return src
    downloaded = hf_hub_download(
        repo_id=args.hf_repo,
        filename=args.model_file,
        repo_type="model",
    )
    return Path(downloaded)


def convert_checkpoint(src: Path, dst: Path, target_dtype: torch.dtype) -> Dict[str, int]:
    tensors = load_file(src)
    converted: Dict[str, torch.Tensor] = {}
    bf16_tensors = 0
    unchanged = 0
    for name, tensor in tensors.items():
        tensor = tensor.to("cpu")
        if tensor.dtype == BF16:
            converted[name] = tensor.to(dtype=target_dtype)
            bf16_tensors += 1
        else:
            converted[name] = tensor
            unchanged += 1
    dst.parent.mkdir(parents=True, exist_ok=True)
    save_file(converted, dst)
    return {
        "converted": bf16_tensors,
        "unchanged": unchanged,
    }


def main() -> None:
    args = parse_args()
    src = resolve_source(args)
    dst = Path(args.output).expanduser()
    target_dtype = DTYPE_MAP[args.dtype]
    stats = convert_checkpoint(src, dst, target_dtype)
    print(
        f"Saved {dst} with dtype={target_dtype} "
        f"(converted {stats['converted']} tensors, {stats['unchanged']} untouched)",
        flush=True,
    )


if __name__ == "__main__":
    main()
