# Performance Benchmarking Guide

This document outlines the methodology for benchmarking and optimizing moshi-server performance.

## 1. Benchmarking Harness

The `bench` module (`moshi-server/src/bench.rs`) provides utilities for measuring inference latency:

### Latency Recorders

Global recorders track key performance metrics:

```rust
use crate::bench::{INFERENCE_LATENCY, ENCODE_LATENCY, DECODE_LATENCY, REQUEST_LATENCY};

// Record a latency sample
INFERENCE_LATENCY.record(duration);

// Get statistics
let stats = INFERENCE_LATENCY.stats();
println!("{}", stats);
// Output: inference: count=1000, mean=45.23ms, min=42.10ms, max=52.00ms, p50=44.50ms, p95=49.20ms, p99=51.80ms
```

### Scoped Timer

Automatically records timing when dropped:

```rust
use crate::bench::ScopedTimer;

{
    let _timer = ScopedTimer::with_recorder("inference", &INFERENCE_LATENCY);
    // ... inference code ...
} // Timer automatically recorded on drop
```

### Statistics

- **count**: Number of samples
- **mean**: Average latency
- **min/max**: Minimum/maximum latency
- **p50/p95/p99**: Percentile latencies

## 2. CUDA Profiling with Nsight Systems

### Setup

```bash
# Install NVIDIA Nsight Systems
sudo apt install nsight-systems

# Or download from NVIDIA Developer
```

### Profile moshi-server

```bash
# Basic profiling
nsys profile --output=moshi-profile \
  moshi-server worker --config configs/config-stt-en-hf.toml

# With CUDA API tracing
nsys profile --trace=cuda,nvtx,osrt \
  --output=moshi-cuda-profile \
  moshi-server worker --config configs/config-stt-en-hf.toml

# Analyze results
nsys stats moshi-profile.nsys-rep
```

### Key Metrics to Monitor

1. **Kernel Launch Overhead**: Time between kernel launches
2. **Memory Transfer Time**: Host-to-device and device-to-host copies
3. **GPU Utilization**: Percentage of time GPU is active
4. **Memory Bandwidth**: How efficiently memory is being used

## 3. Profiling with nvprof (Legacy)

```bash
# Basic profiling
nvprof moshi-server worker --config configs/config-stt-en-hf.toml

# Detailed kernel analysis
nvprof --metrics achieved_occupancy,sm_efficiency \
  moshi-server worker --config configs/config-stt-en-hf.toml
```

## 4. Optimization Targets

### 4.1 Event Tracking Impact

Candle's event tracking can impact performance. To benchmark:

```rust
// In your code, toggle event tracking
candle::cuda_backend::set_event_tracking(false);
```

Measure:
- Baseline with tracking enabled
- Latency with tracking disabled
- Memory usage difference

### 4.2 Flash Attention Integration

The `candle-flash-attn` crate provides optimized attention:

```toml
[dependencies]
candle-flash-attn = "0.9.1"
```

Benchmark:
- Standard attention vs flash attention
- Memory usage reduction
- Throughput improvement

### 4.3 CUDA Graphs

CUDA graphs can reduce kernel launch overhead:

```rust
// Capture graph during warmup
// Replay graph during inference
```

Benefits:
- Reduced CPU overhead
- Lower latency variance
- Better GPU utilization

### 4.4 Memory Pooling

Implement CUDA memory pooling to avoid allocation overhead:

```rust
// Use a memory pool for tensor allocations
// Reuse memory across inference steps
```

### 4.5 Tensor Device Transfers

Audit and minimize `.to_device()` calls:

```bash
# Search for device transfers
grep -r "to_device" moshi/rust/moshi-server/src/
```

### 4.6 Audio Pipeline Optimization

Overlap audio decoding with inference:

```rust
// Pipeline stages:
// 1. Decode audio chunk N
// 2. Run inference on chunk N-1 (parallel with step 1)
// 3. Encode results from chunk N-2 (parallel with steps 1-2)
```

### 4.7 WebSocket Compression

Enable `permessage-deflate` for bandwidth reduction:

```rust
// In axum WebSocket upgrade
ws.protocols(["permessage-deflate"])
```

Measure:
- Bandwidth reduction
- CPU overhead
- Latency impact

## 5. Baseline Measurements

Before optimization, establish baselines:

```bash
# Create baseline measurements
moshi-server worker --config configs/config-stt-en-hf.toml &

# Run test workload
# Record:
# - Inference latency (p50, p95, p99)
# - Throughput (requests/second)
# - GPU utilization
# - Memory usage
```

## 6. Reporting Results

Document all optimizations with:

1. **Baseline**: Pre-optimization metrics
2. **Change**: What was modified
3. **Result**: Post-optimization metrics
4. **Tradeoffs**: Any negative impacts

### Template

```markdown
## Optimization: [Name]

### Baseline
- Inference p50: X ms
- Inference p99: Y ms
- GPU utilization: Z%

### Change
[Description of change]

### Result
- Inference p50: X' ms (Δ%)
- Inference p99: Y' ms (Δ%)
- GPU utilization: Z'%

### Tradeoffs
- [Any negative impacts]
```

## 7. Prometheus Metrics

The benchmarking module integrates with Prometheus:

- `inference_latency_seconds` (histogram)
- `encode_latency_seconds` (histogram)
- `decode_latency_seconds` (histogram)
- `request_latency_seconds` (histogram)

Access at `/metrics` endpoint.

## 8. Initial Audit Findings

### Device Transfer Audit (`.to_device()` calls)

Found in `moshi-server/src/`:

| File | Location | Transfer | Purpose |
|------|----------|----------|---------|
| `tts.rs` | audio_tokens processing | GPU → CPU | Token conversion for output |
| `asr.rs` | text_tokens output | GPU → CPU | Text token decoding |
| `asr.rs` | logits processing | GPU → CPU | Output processing |
| `batched_asr.rs` | text_tokens | GPU → CPU | Text token decoding |
| `batched_asr.rs` | logits | GPU → CPU | Output processing |

**Observation**: All transfers are GPU → CPU for output processing, which is necessary for returning results to clients. These are not redundant transfers.

**Potential Optimization**: 
- Batch multiple outputs before transfer
- Use pinned memory for faster transfers
- Consider async transfers with CUDA streams

### Next Steps (Runtime Testing Required)

The following tasks require runtime profiling on GPU hardware:

1. **CUDA Kernel Profiling** (nsys/nvprof)
   - Measure kernel execution time
   - Identify hotspots
   - Check for kernel serialization

2. **Event Tracking Impact**
   - Benchmark with `candle::cuda_backend::set_event_tracking(false)`
   - Compare latency and memory usage

3. **Flash Attention Evaluation**
   - Check candle-flash-attn compatibility with moshi models
   - Benchmark attention-heavy operations

4. **CUDA Graphs**
   - Identify inference loops suitable for graph capture
   - Measure launch overhead reduction

5. **Memory Pooling**
   - Profile allocation patterns
   - Implement pooling for frequently allocated tensors

6. **WebSocket Compression**
   - Enable permessage-deflate
   - Measure bandwidth vs CPU tradeoff
