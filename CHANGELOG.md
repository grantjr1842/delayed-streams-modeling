# Changelog

## Unreleased

### Added
- Added `pyproject.toml`/`uv.lock` so `uv sync --group dev` provisions `.venv` with autopep8, msgpack, numpy, sounddevice, and websockets for editor tooling.
- Added `scripts/format_moshi_log.py` so operators can regenerate friendly Moshi logs with ANSI removed, localized 12-hour timestamps, and normalized character rendering before sharing traces.
- Added `scripts/prep_sm75_assets.py` so SM75 operators can detect their GPU and invoke the bf16->fp16 converter with a single command (supports simulation/dry-run for CI).
- Created `scripts/run_sm75_smoke_test.py` plus the `.github/workflows/sm75-smoke.yml` workflow to keep a moshi-server SM75 smoke test (or simulation) running in CI.
- Added `scripts/check_gpu_capability.py` to probe CUDA devices and warn SM75 operators to run the fp16 conversion workflow before launching Moshi.
- Documented the fp16 conversion workflow for pre-Ampere GPUs so STT operators can unblock CUDA on SM75 hardware.
- Added `scripts/convert_bf16_to_fp16.py` to download Kyutai's bf16 checkpoint and rewrite it as fp16/fp32 assets under `assets/fp16/`.
- Introduced `configs/config-stt-en_fr-lowram-sm75.toml` that loads the converted checkpoint and overrides dtype to float16.
- Captured the friendly Moshi log at `logs/moshi-logs/log.config-stt-en_fr-lowram-sm75.2025-11-15` (reformatted via `scripts/format_moshi_log.py`) plus its raw trace to document the CUDA failure path.

### Changed
- Simplified the MLX STT helpers so they only quantize q4 checkpoints to match the supported configs.
- Hardened `scripts/stt_from_file_rust_server.py` and `scripts/stt_from_mic_rust_server.py` to cancel their send/receive tasks, shield the shutdown path (even on Ctrl+C), and close the WebSocket cleanly so moshi-server stops logging `recv_loop` connection reset errors.
- Further strengthened the Rust-server Python clients so they emit a stream-end marker plus trailing silence even on Ctrl+C, preventing the resurfacing `recv_loop` "Connection reset without closing handshake" errors in Moshi logs.
- Added strict typing and explicit queue/websocket annotations to `scripts/stt_from_mic_rust_server.py` so Pylance strict mode passes without Unknown type warnings.
- Ensured `scripts/stt_from_file_rust_server.py` and `scripts/stt_from_mic_rust_server.py` now keep draining server responses until Moshi emits its own `Marker`, then close the WebSocket via the shared shutdown helper so the 2025-11-15 `recv_loop` resets disappear even when an interrupt arrives mid-stream.
- Documented the `uv sync --group dev` bootstrap flow and pointed `.vscode/settings.json` at `.venv` so VS Code's Python extension and autopep8 formatter stay wired automatically.
