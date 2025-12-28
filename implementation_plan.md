# Implementation Plan: Performance Optimizations (Refined)

## Master Issue: Performance Optimization Suite

This plan covers the implementation of targeted performance optimizations to reduce latency and improve throughput of the Moshi server.

### 1. Configurable Hardware Optimizations
- **Goal**: Allow users to toggle CUDA event tracking and TF32.
- **Location**: `server/rust/moshi/moshi-server/src/main.rs`.
- **Implementation**:
    - Verify `--disable-cuda-events` and `--enable-tf32` flags in `WorkerArgs`.
    - Ensure they are applied to the CUDA device during startup.

### 2. Verified Flash Attention
- **Goal**: Confirm and log Flash Attention usage.
- **Location**: `server/rust/moshi/moshi-core/src/transformer.rs`.
- **Implementation**:
    - Add a `tracing::debug!` or `info!` message when Flash Attention is invoked.
    - Check if `use_flash_attn` can be made configurable.

### 3. Optimized Device Transfers for Logging
- **Goal**: Minimize blocking during GPU -> CPU transfers for token logging.
- **Location**: `server/rust/moshi/moshi-server/src/asr.rs`, `server/rust/moshi/moshi-server/src/tts.rs`.
- **Implementation**:
    - Move `to_device(&Device::Cpu)` calls into background tasks.
    - Use asynchronous transfers if available in candle or ensure they don't block the main inference loop.

### 4. Advanced Audio Pipeline Overlapping
- **Goal**: Reduce inter-token latency.
- **Location**: `server/rust/moshi/moshi-server/src/tts.rs` and `asr.rs`.
- **Implementation**:
    - Review `inference_loop` and `audio_processing_loop` in `tts.rs`.
    - Ensure Mimi decoding/encoding is not serializing with the next LM step.

## Verification Plan
- Run existing benchmarks: `cargo bench` or equivalent if available.
- Use `nsys` profiles (existing files: `moshi-profile.nsys-rep`).
- Document results in `walkthrough.md`.
