---
description: Build and install moshi-server from vendored source with CUDA support
auto_execution_mode: 3
---

# Build moshi-server from Source

This workflow compiles `moshi-server` from the vendored source in `moshi/rust/` and installs it to `~/.cargo/bin/`, replacing any existing binary.

## Why Build from Source?

The vendored source includes custom patches:
- **Auto GPU detection**: Detects compute capability and selects optimal dtype
- **Auto batch size**: Adjusts batch size based on available VRAM
- **SM 7.x support**: Automatically uses F32 for Turing GPUs (RTX 20xx)

## Prerequisites

- CUDA toolkit installed and in PATH
- Rust toolchain (rustup)
- ~10GB disk space for build artifacts

## Quick Build

// turbo
```bash
cargo install --path moshi/rust/moshi-server --features cuda --force --verbose --verbose
```

## Detailed Steps

### 1. Verify CUDA is available

```bash
nvcc --version
```

### 2. Build and install with CUDA support

// turbo
```bash
cargo install --path moshi/rust/moshi-server --features cuda --force --verbose --verbose
```

This command:
- Compiles the vendored `moshi-server` source (~30-60 seconds)
- Enables CUDA GPU acceleration (`--features cuda`)
- Replaces any existing installation (`--force`)
- Installs to `~/.cargo/bin/moshi-server`

### 3. Verify the new binary is active

// turbo
```bash
which moshi-server
```

Should output: `~/.cargo/bin/moshi-server`

### 4. Test GPU detection

```bash
moshi-server worker --config configs/config-stt-en_fr-hf.toml
```

Look for these log lines indicating successful GPU detection:
```
Detected GPU capabilities gpu=NVIDIA GeForce RTX 2070 sm_version=75 supports_bf16=false
Auto-detected dtype recommended_dtype="f32"
Auto-setting dtype_override for BatchedAsr module="asr" dtype="f32"
starting asr loop 1
```

## Expected Behavior by GPU

| GPU Series | SM Version | Auto dtype | Notes |
|------------|------------|------------|-------|
| RTX 40xx (Ada) | 8.9 | bf16 | Native BF16 support |
| RTX 30xx (Ampere) | 8.6 | bf16 | Native BF16 support |
| RTX 20xx (Turing) | 7.5 | f32 | No native BF16 |
| GTX 16xx (Turing) | 7.5 | f32 | No native BF16 |

## Troubleshooting

### No GPU detection logs
If you don't see "Detected GPU capabilities" in the output, you're running an old binary. Rebuild with `--force`.

### CUDA_ERROR_NOT_FOUND after detection
The auto-detection failed or config has explicit `dtype_override`. Check:
1. Remove any `dtype_override` from your config to use auto-detection
2. Verify the detected dtype matches your GPU capability

### Build fails with OpenSSL errors
```bash
sudo apt install libssl-dev pkg-config
```

### Build fails with sentencepiece errors
```bash
sudo apt install cmake
```

## Environment Variables

Tune batch size calculation:
- `MOSHI_MODEL_PARAMS_BILLIONS` - Model size (default: 1.0)
- `MOSHI_MEMORY_OVERHEAD_FACTOR` - Memory multiplier (default: 2.0)

Example for larger batch sizes:
```bash
MOSHI_MEMORY_OVERHEAD_FACTOR=1.5 moshi-server worker --config configs/config-stt-en_fr-hf.toml
```