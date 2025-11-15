#!/usr/bin/env bash
# Set up Python environments for Kyutai quantization demos.
set -euo pipefail

python_bin="${PYTHON:-python3}"

if ! command -v "$python_bin" >/dev/null 2>&1; then
  echo "Python executable '$python_bin' not found." >&2
  exit 1
fi

# CUDA + bitsandbytes requirements for STT benchmarking
"$python_bin" -m pip install -U \
  "transformers>=4.56.0" \
  accelerate \
  "bitsandbytes>=0.43.0" \
  soundfile \
  scipy \
  psutil

# MLX TTS requirements
"$python_bin" -m pip install -U \
  "moshi-mlx>=0.2.9" \
  sentencepiece \
  soundfile \
  sounddevice \
  psutil

echo "Dependencies installed. Use stt_bnb_quant_bench.py for CUDA STT and tts_mlx_quant_bench.py for MLX TTS." 
