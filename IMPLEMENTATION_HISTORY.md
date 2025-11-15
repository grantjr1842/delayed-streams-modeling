# Implementation History

## 2025-11-16 · feat/pre-ampere-workflow
- Added `scripts/check_gpu_capability.py` so operators can confirm whether their CUDA device needs the fp16 checkpoint before launching Moshi.
- Folded the helper into the documented pre-Ampere workflow and marked the roadmap item complete to keep the enablement story current.
- Wrapped the bf16->fp16 conversion inside `scripts/prep_sm75_assets.py` and shipped the SM75 smoke tooling (`scripts/run_sm75_smoke_test.py` + `.github/workflows/sm75-smoke.yml`) so CI surfaces CUDA regressions even without GPUs.
- Hardened the Rust-server Python clients to cancel their tasks, shield websocket shutdown (even on Ctrl+C), and close the connection cleanly, stopping the `recv_loop` reset errors captured in the SM75 Moshi logs.
- Revisited the Rust-server helpers so they flush the final marker/silence even when interrupted, eliminating the lingering `moshi_server::utils` "Connection reset without closing handshake" errors from the SM75 traces.
- Ensured both helpers now close the WebSocket as part of that flush sequence so the handshake completes before the process exits, preventing any residual `recv_loop` resets when Ctrl+C arrives.
- Let the shared shutdown helper own websocket closing for both Rust-server clients and kept the microphone helper draining responses until Moshi emits its `Marker`, eliminating the 2025-11-15 `recv_loop` reset regression that reappeared in the SM75 logs.
- Added `pyproject.toml`/`uv.lock` plus updated README/dev tooling guidance so `uv sync --group dev` yields a ready `.venv` for VS Code, autopep8, and strict Pylance usage.
- Annotated `scripts/stt_from_mic_rust_server.py` with concrete queue/audio/websocket types to eliminate the Unknown warnings reported by Pylance.

## 2025-11-15 · feat/remove-quantized
- Removed the lingering q8 quantization path from the MLX STT helpers to ensure only supported checkpoints are exposed.
- Authored the bf16->fp16 conversion utility (`scripts/convert_bf16_to_fp16.py`) and the SM75-specific Moshi config so SM75 GPUs can stay on CUDA.
- Documented the recommended workflow plus captured friendly/raw Moshi logs that prove the CUDA error path when the fp16 asset is missing.

## 2025-11-17 · feat/log-display-friendly
- Added `scripts/format_moshi_log.py` to strip ANSI artifacts, normalize characters, and render UTC timestamps as local 12-hour times before emitting a friendly Moshi log.
- Regenerated `logs/moshi-logs/log.config-stt-en_fr-lowram-sm75.2025-11-15` from the raw trace so the sample log shares the cleaned layout.
- Documented how to run the formatter in `README.md` and noted the formatter’s arrival in the changelog/roadmap to keep the living docs aligned with the new workflow.
