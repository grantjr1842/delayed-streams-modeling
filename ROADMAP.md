# Roadmap

## 1. Pre-Ampere CUDA Enablement · Branch: feat/pre-ampere-workflow
- [x] Automate bf16->fp16 conversion inside a helper command so operators do not run separate scripts (`scripts/prep_sm75_assets.py`).
- [x] Ship CI smoke tests that launch `moshi-server` with the SM75 config and assert CUDA stays up (see `.github/workflows/sm75-smoke.yml` + `scripts/run_sm75_smoke_test.py`).
- [x] Add GPU capability detection to warn when bf16 checkpoints will fail and point to the converter (`scripts/check_gpu_capability.py`).
- [x] Stop Moshi's `recv_loop` handshake resets by making the Rust-server helpers flush their marker/silence payload even when users hit Ctrl+C mid-stream.
- [x] Keep the helpers connected until Moshi returns its own `Marker`, then let `_shutdown_session` close the socket so the 2025-11-15 `recv_loop` reset regression stays gone.

## 2. Log Health Monitoring · Branch: perf/log-pipeline
- [ ] Extend the incremental watcher to emit metrics (latency, throughput) for alerting.
- [ ] Publish formatted logs to an S3 bucket so operators can share traces without copying files manually.
- [x] Offer `scripts/format_moshi_log.py` so operators can emit ANSI-free, local 12-hour timestamps and readable characters alongside the raw Moshi traces.
