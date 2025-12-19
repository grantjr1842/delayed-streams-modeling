# Benchmark Scripts

This directory contains benchmark scripts for evaluating model performance under different quantization schemes.

## Available Benchmarks

### stt_bnb_quant_bench.py

Benchmarks the Speech-to-Text model with BitsAndBytes quantization.

**Usage:**
```bash
uv run tools/benchmarks/stt_bnb_quant_bench.py
```

### tts_mlx_quant_bench.py

Benchmarks the Text-to-Speech model with MLX quantization (for Apple Silicon).

**Usage:**
```bash
uv run tools/benchmarks/tts_mlx_quant_bench.py
```

## Running Benchmarks

All benchmark scripts can be run with `uv run` to automatically manage dependencies:

```bash
cd /path/to/delayed-streams-modeling
uv run tools/benchmarks/<script_name>.py
```

## Results

Benchmark results typically include:
- Model load time
- Inference latency
- Real-time factor (RTF)
- Memory usage
- Output quality metrics

See individual script help text for detailed options:

```bash
uv run tools/benchmarks/<script_name>.py --help
```
