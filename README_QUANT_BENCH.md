# Kyutai Quantization Benchmarks

This toolkit bundles the benchmarking helpers that were previously outlined in
chat. It covers both Kyutai's speech-to-text (STT) and text-to-speech (TTS)
paths with weight-only quantization and simple performance logging.

## Contents

| File | Purpose |
| --- | --- |
| `scripts/quant_bench.py` | Unified CLI with `stt` and `tts` subcommands that dispatch to the CUDA and MLX benchmarks. |
| `quant_bench/` | Library utilities shared by the CLIs (model loading, metrics helpers, and JSON/CSV export). |
| `stt_bnb_quant_bench.py` | Backwards-compatible wrapper that forwards to the new STT benchmark helpers. |
| `tts_mlx_quant_bench.py` | Backwards-compatible wrapper that forwards to the new TTS benchmark helpers. |
| `setup_quant_env.sh` | Convenience installer for the Python dependencies needed by the two scripts. |

## Quick start

1. Install dependencies:
   ```bash
   ./setup_quant_env.sh
   ```

2. Run the CUDA STT benchmark (4-bit quantization by default):
   ```bash
   python scripts/quant_bench.py stt path/to/audio.wav
   ```

3. Run the MLX TTS benchmark (from the repository root so the script path resolves):
   ```bash
   python scripts/quant_bench.py tts --quantize 8 --text "Hello, Kyutai" --outfile out.wav
   ```

### STT options

The STT subcommand accepts multiple audio files and can emit CSV/JSON summaries:

```bash
python scripts/quant_bench.py stt \
  audio/*.wav \
  --csv results.csv \
  --json results.json
```

Generation parameters such as `--max-new-tokens`, `--temperature`, and
`--beam-size` are forwarded to `model.generate` for experimentation. RTF is
reported for both the pure generation loop and the overall preprocessing +
inference time.

### TTS options

The TTS subcommand proxies convenient arguments to Kyutai's script:

```bash
python scripts/quant_bench.py tts \
  --quantize 4 \
  --text "Make it punchy." \
  --outfile quantized.wav \
  --voice ljspeech
```

Additional flags can be passed after `--extra-args`. The script captures the
child process' peak RSS and prints any stdout/stderr emitted by the MLX helper
for troubleshooting.

## Notes

* bitsandbytes wheels are available on Linux with CUDA GPUs. macOS users should
  prefer the MLX path instead.
* Audio is resampled to 24 kHz to match Kyutai's pretrained STT expectations.
* The benchmark output is intentionally JSON-formatted to make it easy to pipe
  into other tooling for automation.
