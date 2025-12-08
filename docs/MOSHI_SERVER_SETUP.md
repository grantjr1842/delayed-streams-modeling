# Moshi Server Setup and Compilation Notes

This document outlines the changes required to compile and run `moshi-server` (and related components) in this environment, specifically targeting an NVIDIA RTX 2070 (8GB VRAM) on Linux.

## 1. Dependency Fixes

Several dependencies had to be pinned or adjusted to resolve conflicts and compilation errors.

### `rand` and `getrandom`
- **Issue**: `moshi` code (specifically `moshi-backend`) relies on `rand` with the `getrandom` feature. The latest `rand` 0.9.0 removed/refactored this feature, causing compilation failures.
- **Fix**: Pinned `rand` to version `0.8.5` in `moshi/rust/Cargo.toml`.
  ```toml
  rand = { version = "0.8.5", features = ["getrandom"] }
  ```

### `vergen`
- **Issue**: `moshi-server` uses `vergen` for build metadata. `vergen` 9.x introduced breaking API changes (removing the `git` feature in favor of separate crates). The existing code uses the 8.x API.
- **Fix**: Pinned `vergen` to version `8.3.2` (specifically `=8.3.2` to avoid 9.x updates) in `moshi/rust/Cargo.toml`.
  ```toml
  vergen = { version = "=8.3.2", features = ["build", "cargo", "git", "gitcl", "rustc", "si"] }
  ```

### PyO3 0.27 Migration
- **Issue**: The `pyo3` crate was updated to 0.27, which introduced breaking changes to the API.
- **Fixes Applied** (primarily in `moshi/rust/moshi-server/src/py_module.rs`):
  - `prepare_freethreaded_python()` replaced with `Python::initialize()`.
  - `Python::with_gil(...)` replaced with `Python::attach(...)`.
  - `PyObject` replaced with `Py<PyAny>`.
  - `downcast_bound` replaced with `cast_bound` (in other locations if applicable).
  - Usage of `IntoPyObject` trait updates.

## 2. VRAM Detection and Auto-Configuration

To prevent `CUDA_ERROR_OUT_OF_MEMORY` on the 8GB card, we implemented automatic VRAM detection and batch size adjustment.

### Implementation Details
- **NVML Wrapper**: Added `nvml-wrapper` dependency to query GPU status.
- **VRAM Detection (`src/utils.rs`)**:
  - Added `get_gpu_info()` to retrieve available VRAM, compute capability, and other metrics.
- **Auto-Config Logic (`src/main.rs`)**:
  - In `Command::Worker`, the server now detects the GPU before starting.
  - **Batch Size Calculation**:
    - `available_vram - reserved_vram (default 512MB)`
    - `max_batch_size = free_vram / per_item_vram (default 400MB)`
  - **Config Adjustment**:
    - Iterates through `BatchedAsr` modules and reduces `batch_size` if it exceeds the calculated safe limit.
    - Sets `dtype_override` based on compute capability (though see "Turing Compatibility" below).

### Environment Variables
- `MOSHI_MODEL_PARAMS_BILLIONS`: Override model size hint (default 7.0).
- `MOSHI_PER_BATCH_ITEM_MB`: Override VRAM per batch item estimate (default 400).
- `MOSHI_API_KEY`: Comma-separated list of authorized API keys (replaces hardcoded `authorized_ids` in config).

## 3. Environment Configuration

The server automatically loads environment variables from a `.env` file in the current working directory at startup using `dotenvy`. This eliminates the need to manually source the file before running.

### Usage
Simply create a `.env` file (see `.env.example` for template):
```bash
# .env
MOSHI_API_KEY=my_secret_token,another_token
BETTER_AUTH_SECRET=your_jwt_secret_here
```

Then run the server directly:
```bash
moshi-server worker --config configs/config-stt-en-hf.toml
```

The `.env` file is loaded before any configuration parsing, so all environment variables are available for config resolution.

### Authentication

The server supports loading authorized keys from the `MOSHI_API_KEY` environment variable. This is preferred over hardcoding tokens in the configuration file.

## 4. Turing (RTX 20xx) Compatibility

- **Issue**: The RTX 2070 (Compute Capability 7.5) supports FP16 but has issues with BF16 in some Candle/Moshi operations, or requires explicit F32 for stability in certain matmul operations.
- **Fix**:
  - In `moshi/rust/moshi-core/src/nn.rs`, the `matmul_dtype` function is forced to return `DType::F32` for now.
  - `utils.rs` or `main.rs` might detect `7.5` and suggest `F16`, but the core logic enforces safe types where necessary.

## 5. Configuration Files

- **`configs/config-stt-en_fr-lowram-sm75.toml`**:
  - Created a specific configuration for this setup.
  - Uses `backend = "cuda"`.
  - Adjusts model paths to local cached assets.
  - Configures `BatchedAsr` with a safe initial batch size (e.g., 4 or 8), which is then auto-lowered by the server if needed.

## 6. Logging

The server uses `tracing` for structured logging with the following features:

### Log Format
- **Timestamps**: Human-readable format `YYYY-MM-DD HH:MM:SS.mmm` (e.g., `2025-12-02 01:36:42.113`)
- **File output**: Clean text without ANSI color codes
- **Console output**: Colored output for terminal readability

### Log Rotation
Logs are automatically rotated based on:
- **Daily rotation**: New log file each day
- **Size-based rotation**: Rotates when file exceeds `--log-max-size-mb` (default: 100 MB)
- **File cleanup**: Keeps only `--log-max-files` rotated files (default: 10)

Log files follow Debian-style naming: `log.instance`, `log.instance.1`, `log.instance.2`, etc.

### CLI Options
```bash
moshi-server worker --config config.toml \
  --log info \                    # Log level (trace, debug, info, warn, error)
  --log-max-size-mb 100 \         # Max size per log file in MB
  --log-max-files 10 \            # Max number of rotated log files to keep
  --silent                        # Disable console output (file only)
```

### Log Directory
Logs are written to the `log_dir` specified in the config file (e.g., `logs/moshi-server-rust/stt/`).

## 7. Building

To build the server with these changes:
```bash
cd moshi/rust/moshi-server
cargo install --path . --features cuda --force
```

## 8. Warmup Behavior & Observability

- **Config toggle**: Warmup is controlled by the top-level `[warmup]` block in the TOML config and is **enabled by default**.
  ```toml
  [warmup]
  enabled = true  # set to false to skip eager warmup
  ```
- **What runs**:
  - `Asr` and `Tts` modules run a one-time warmup at startup.
  - `BatchedAsr` warms up inside its model loop using the same toggle.
  - Other modules currently skip warmup.
  - TTS skips warmup automatically if no voices are configured.
- **Logging**: On startup, each warmed module logs start/end and duration; failures are logged with errors, and skips are logged when warmup is disabled.
- **Metrics** (Prometheus):
  - `warmup_duration_seconds` (histogram)
  - `warmup_success_total`
  - `warmup_failure_total`
  - `warmup_skipped_total`
- **When to disable**: If startup time is critical or running on limited resources, set `warmup.enabled = false` to start serving immediately (metrics will record the skip).
