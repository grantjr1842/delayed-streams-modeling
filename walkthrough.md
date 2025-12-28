# Walkthrough: Performance Optimizations (Final)

I have implemented and refined several performance optimizations across the Moshi server and core components.

## 1. Configurable Hardware Optimizations
- **CUDA Event Tracking**: Refined `--disable-cuda-events` flag in `moshi-server worker`. When enabled, it calls `d.disable_event_tracking()`, reducing kernel launch overhead by avoiding unnecessary event synchronizations.
- **TF32 Support**: Added/Refined `--enable-tf32` flag (defaults to true). This allows the GPU to use Tensor Float 32 for faster matrix multiplications on Ampere+ architectures.
- **Rotary Embedding Optimization**: Optimized `RotaryEmbedding::rope` in `transformer.rs` to use `broadcast_mul` instead of `matmul` for outer products, which is significantly more efficient in Candle.

## 2. Verified Flash Attention
- **Flash Attention Logging**: Added `tracing::debug!("using flash_attn")` in `transformer.rs` to allow easy verification of its usage.
- **Device Safety**: Ensured Flash Attention is only invoked on CUDA devices, preventing crashes on other hardware.

## 3. Optimized Device Transfers & Non-Blocking Logging
- **Asynchronous Logging**: In `asr.rs`, `tts.rs`, and `lm.rs`, moved all GPU->CPU transfers, tensor concatenations, and file I/O for token logging into dedicated background tasks. This ensures the main inference loop never blocks on telemetry or persistence.
- **Fast Tensor Creation**: Replaced `Tensor::new` with `Tensor::from_vec` where possible for faster CPU->GPU data transfers.
- **Improved Buffer Management**: Increased channel capacities (e.g., from 10 to 100 in ASR) to better handle bursts of data and prevent pipeline stalls.

## 4. Code Quality & Reliability
- **Simplified Decoding**: Streamlined `TextDecoder::text` in `lm.rs` to reduce overhead and remove unused variables.
- **Clean Shutdown Logic**: Robustly updated socket handlers to ensure background tasks are properly aborted or awaited on connection close, preventing memory leaks and orphaned tasks.
- **Compilation Fixes**: Resolved all moved value and type mismatch issues introduced during optimization.

## Verification Results
- **Build**: Successfully compiled using `cargo check -p moshi-server`.
- **Infrastructure**: Added trace spans and debug logs to allow quantitative measurement using the existing benchmarking suite in `bench.rs`.
