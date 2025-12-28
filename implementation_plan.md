# Implementation Plan: Comprehensive Performance Optimizations

## Master Issue: #99

This plan covers the final stages of the performance optimization suite, focusing on pipelining and final verification.

### 1. Pipelining Mimi and LM in ASR
- **Goal**: Reduce inter-token latency by overlapping Mimi encoding and LM inference.
- **Status**:
  - Completed for single-stream `asr.rs`.
  - Pending for `batched_asr.rs`.
- **Implementation**:
  - Refactor `BatchedAsrInner::start_model_loop` to use a multi-stage pipeline.
  - Ensure `pre_process` and `post_process` work correctly with the pipeline delay.

### 2. Final Verification & Benchmarking
- **Goal**: Quantify improvements and ensure no regressions.
- **Implementation**:
  - Run `moshi-server` with various configurations.
  - Use `nsys profile` to capture new traces.
  - Document results in `walkthrough.md`.

### 3. Pull Request & Merging
- **Goal**: Merge all optimizations into `main`.
- **Implementation**:
  - Create a single comprehensive PR linking all sub-issues.
  - Merge after successful CI/verification.

## Verification Plan
- Run existing benchmarks: `cargo bench` or equivalent if available.
- Use `nsys` profiles (existing files: `moshi-profile.nsys-rep`).
- Document results in `walkthrough.md`.
