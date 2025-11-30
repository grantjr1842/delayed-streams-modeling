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

## 3. Turing (RTX 20xx) Compatibility

- **Issue**: The RTX 2070 (Compute Capability 7.5) supports FP16 but has issues with BF16 in some Candle/Moshi operations, or requires explicit F32 for stability in certain matmul operations.
- **Fix**:
  - In `moshi/rust/moshi-core/src/nn.rs`, the `matmul_dtype` function is forced to return `DType::F32` for now.
  - `utils.rs` or `main.rs` might detect `7.5` and suggest `F16`, but the core logic enforces safe types where necessary.

## 4. Configuration Files

- **`configs/config-stt-en_fr-lowram-sm75.toml`**:
  - Created a specific configuration for this setup.
  - Uses `backend = "cuda"`.
  - Adjusts model paths to local cached assets.
  - Configures `BatchedAsr` with a safe initial batch size (e.g., 4 or 8), which is then auto-lowered by the server if needed.

## 5. Building

To build the server with these changes:
```bash
cd moshi/rust/moshi-server
cargo install --path . --features cuda --force
```
