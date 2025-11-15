# Plan

### 1. Remove the obsolete quantized config · Branch: feat/remove-quantized
- [x] Delete the quantized config file from `configs/` and remove the associated 8-bit asset directory so no quantized weights remain in the workspace.
- [x] Update README/STT validation notes to remove the deleted config’s failure tales.
- [x] Simplify the MLX helper scripts so they only rush the remaining q4 path.
- [x] Confirm no references to the deleted quantized support remain and run `python -m py_compile scripts/stt_from_file_mlx.py scripts/stt_from_mic_mlx.py`.
- [x] Provide the bf16->fp16 conversion helper plus SM75-ready config so pre-Ampere GPUs follow the supported workflow.
  - [x] Add `scripts/convert_bf16_to_fp16.py` that rewrites the Kyutai checkpoint and keep the converted artifacts under `assets/fp16/`.
  - [x] Ship `configs/config-stt-en_fr-lowram-sm75.toml`, log the CUDA failure trace, and wire documentation updates so operators know how to stay on CUDA.

### 2. Friendly log formatting · Branch: feat/123-log-format
- [x] Implement a dedicated formatter that strips ANSI junk, normalizes file/target metadata, and emits timestamps in the operator’s local timezone.
  - [x] Detect the current timezone via `datetime.now().astimezone()` so logs match the operator’s locale (EST for the current machine).
- [x] Run the formatter on `logs/moshi-logs/log.foo.2025-11-15` to prove the new layout and store the friendly output.
  - [x] Capture a sample of the rewritten log and describe the fields so future consumers know what to expect.
- [x] Document the formatter’s usage (new script entry or README note) so maintainers can keep future Moshi logs readable.
- [x] Switch log timestamps to 12-hour format so operators see AM/PM times that match the IDE’s display.
- [x] Streamline the log pipeline so raw Moshi logs land in `logs/moshi-logs/raw/` and a watcher keeps the published logs formatted automatically.
  - [x] Update all configs to target the raw directory and ship a watcher script that mirrors fresh entries into `logs/moshi-logs/` with the friendly format.
  - [x] Document how to run the watcher (and require it alongside `moshi-server`) so future logs stay readable without manual fixes.

### 3. Log pipeline performance · Branch: perf/log-pipeline
- [x] Review the current log formatter/watcher and record the bottlenecks that incremental tailing and change detection would resolve.
  - [x] Capture how often files are rewritten today and why avoiding full rewrites should cut both CPU and disk churn.
- [x] Rework `scripts/watch_moshi_logs.py` so it tracks offsets and only reformats appended data while keeping `--once`, `--interval`, and `--quiet` intact.
  - [x] Handle truncated files and partial final lines without corrupting the friendly log output.
- [x] Update `scripts/format_moshi_log.py` to skip writing when the sanitized output already matches the destination file.
- [x] Refresh `README.md` to explain the new watcher behavior (incremental mirroring, what directories to run it from, and why it’s more lightweight).
- [x] Verify the refactored pipeline by running `python scripts/watch_moshi_logs.py --once` on a test log and confirming only new entries are appended.

### 4. Pre-Ampere CUDA guardrail · Branch: feat/pre-ampere-workflow
- [x] Locate the recommended GPU capability detection requirement in the docs/roadmap and figure out what signals the script needs to emit for SM75 operators.
- [x] Implement `scripts/check_gpu_capability.py` so it inspects CUDA devices (torch first, `nvidia-smi` fallback), flags compute capability < 8.0, and points to the fp16 converter/config.
  - [x] Provide a simulation/override flag so we can test both pre-Ampere and Ampere messaging even on GPU-less CI.
- [x] Document the new helper in the README’s pre-Ampere workflow section plus log it in CHANGELOG/IMPLEMENTATION_HISTORY and mark the ROADMAP item complete.
- [x] Validate via `python scripts/check_gpu_capability.py --simulate sm75` and `--simulate sm90`, formatting/linting as needed.

### 5. SM75 automation & CI smoke · Branch: feat/pre-ampere-workflow
- [x] Ship a one-shot helper (e.g., `scripts/prep_sm75_assets.py`) that detects GPU capability, skips redundant work when already on Ampere, and invokes the bf16->fp16 converter so operators only run a single command.
  - [x] Wire dry-run/simulation flags so CI and CPU-only hosts can verify the automation path.
- [x] Resolve the `recv_loop` websocket reset by making the Rust-server Python clients close gracefully and validating with `python -m py_compile scripts/stt_from_file_rust_server.py scripts/stt_from_mic_rust_server.py`.
  - [x] Add explicit cancellation + close paths in both scripts so moshi-server receives a clean closing handshake even when the smoke test ends abruptly.
  - [x] Shield the shutdown sequence so Ctrl+C / cancellation still runs the close handshake before the websocket drops.
- [x] Stop the resurfaced `recv_loop` handshake error by making the Rust-server Python clients flush a stream-end marker + silence even when users Ctrl+C mid-run.
  - [x] Wire a graceful shutdown signal into both scripts so send/receive tasks exit cleanly instead of dropping the socket.
  - [x] Document the behavior shift and remind operators to re-run `python -m py_compile scripts/stt_from_file_rust_server.py scripts/stt_from_mic_rust_server.py` after touching the helpers.
- [x] Refresh README plus living docs so the recommended workflow points to the helper and the roadmap item stays honest.
- [x] Add a GitHub Actions workflow that exercises the helper and runs a simulated `moshi-server` smoke test against the SM75 config so CUDA regressions surface early even without GPUs.
  - [x] Provide `scripts/run_sm75_smoke_test.py` with a `--simulate-success` mode for CI plus hooks to run the real binary when GPUs are available.
- [x] Validate by running the helper + smoke scripts in simulation mode and ensure `.github/workflows/sm75-smoke.yml` passes `act`/lint expectations if applicable.

### 6. Friendly log formatter · Branch: feat/log-display-friendly
- [x] Build `scripts/format_moshi_log.py` to strip ANSI artifacts, convert UTC timestamps to a local 12-hour format, and normalize characters when generating friendly Moshi logs.
- [x] Regenerate `logs/moshi-logs/log.config-stt-en_fr-lowram-sm75.2025-11-15` from the raw trace so the sample log shares the new layout.
- [x] Document the formatter usage (command line + generated path) in `README.md` so operators can keep future logs readable.
- [x] Capture the formatter work in `CHANGELOG.md`, `ROADMAP.md`, and `IMPLEMENTATION_HISTORY.md`.

### 7. Close handshake resets · Branch: fix/handshake-reset
- [x] Close the WebSocket from both Rust-server helpers immediately after they flush the stream-end marker and trailing silence so Ctrl+C still leaves a clean closing handshake.
- [x] Record the new handshake guarantee in `README.md` plus the changelog/history entries so operators know what changed.
- [x] Prove the helpers still parse cleanly with `python -m py_compile scripts/stt_from_file_rust_server.py scripts/stt_from_mic_rust_server.py`.
- [x] Reproduce the resurfacing `recv_loop` reset in the 2025-11-15 Moshi logs and spell out why the clients still yank the socket before the server finishes closing.
- [x] Let the shutdown helper own the close handshake so the senders only flush marker/silence, then wait for the server’s `Marker` before cancelling receive tasks.
  - [x] Update `scripts/stt_from_file_rust_server.py` so `send_stream_end` no longer closes the WebSocket early.
  - [x] Update `scripts/stt_from_mic_rust_server.py` so the receive loop keeps draining responses (even after Ctrl+C) until the server’s `Marker` arrives, and the socket closes via `_shutdown_session`.
- [x] Refresh `README.md`, `CHANGELOG.md`, `ROADMAP.md`, and `IMPLEMENTATION_HISTORY.md` to capture the tightened handshake plus rerun `python -m py_compile scripts/stt_from_file_rust_server.py scripts/stt_from_mic_rust_server.py`.
