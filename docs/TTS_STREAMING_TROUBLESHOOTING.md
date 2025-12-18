# TTS Audio Streaming Troubleshooting Guide

This document provides guidance on resolving audio streaming issues with the moshi-server TTS.

## Common Issues

### Glitchy / Distorted Audio

**Symptoms**: Audio breaks up, stutters, or has gaps during playback.

**Root Cause**: Real-Time Factor (RTF) > 1.0 means the server generates audio slower than playback speed, causing client buffer underruns.

**Diagnosis**:
```bash
# Run benchmark to check RTF
./scripts/tts_rtf_bench.sh ws://127.0.0.1:8080 5
```

Expected output metrics:
- **RTF < 1.0**: Server is fast enough for real-time playback ✓
- **RTF > 1.0**: Server is too slow, causing underruns ✗

### Solutions by Severity

#### Quick Fix (No Restart Required)
Increase client prebuffer to compensate for high RTF:
```bash
cd tts-rs && cargo run --release -- \
  --url ws://127.0.0.1:8080 \
  --prebuffer-ms 3000 \
  --max-buffer-ms 8000 \
  --input /dev/stdin \
  --play
```

#### Recommended Fix (Server Restart Required)
Use a config optimized for real-time performance:

| Config File | n_q | batch_size | Expected RTF | Quality |
|-------------|-----|------------|--------------|---------|
| `config-tts-fast.toml` | 4 | 2 | ~0.5-0.8 | Lower |
| `config-tts-realtime.toml` | 8 | 1 | ~1.0-1.5 | Medium |
| `config-tts.toml` | 16 | 4 | ~2.0-2.5 | High |

```bash
# Start server with fast config
./run-moshi-server.sh configs/config-tts-fast.toml
```

#### Long-term Fix
Issue #79 (Refactor to pure Rust) removes the Python/PyO3 overhead, expected to reduce RTF by 30-50%.

## Key Configuration Parameters

### `n_q` (Quantization Levels)
Higher = better quality, slower inference
- **4-8**: Fast, suitable for real-time
- **12-16**: High quality, requires good GPU
- **Max 32**: Best quality, GPU-intensive

### `batch_size`
Higher = more concurrent connections, slower per-request
- **1**: Best single-user latency
- **4+**: Production multi-user scenarios

### Client Buffer Settings

| Parameter | Default | Recommended for RTF > 1 |
|-----------|---------|------------------------|
| `prebuffer_ms` | 1500 | 3000-5000 |
| `max_buffer_ms` | 6000 | 8000-12000 |

## Audio Integrity Testing

```bash
# Check WAV files for corruption
python scripts/tts_audio_integrity_check.py tmp/tts/

# Run full benchmark suite
./scripts/tts_rtf_bench.sh ws://127.0.0.1:8080 5
```

## Related Issues

- **#78**: Fix glitchy audio streaming
- **#79**: Refactor moshi-server to Rust-only
