# OOM Analysis for Moshi Server

## Incident
User reported `CUDA_ERROR_OUT_OF_MEMORY` on RTX 2070 (8GB) when using `config-stt-en_fr-hf.toml`.

## Configuration Comparison

### HF Config (`config-stt-en_fr-hf.toml`)
- Model: `hf://kyutai/stt-1b-en_fr-candle/model.safetensors`
- DType: Auto-detect (Resolves to "f16" on Turing/SM7.5)
- Batch Size: 64 (Auto-adjusted)

### Low-RAM Config (`config-stt-en_fr-lowram-sm75.toml`)
- Model: `assets/fp16/stt-1b-en_fr-candle.fp16.safetensors`
- DType: "f32" (Explicit override)
- Batch Size: 4 (Auto-adjusted)

## Memory Calculations (Estimated)

### Scenario A: HF Config (Auto "f16")
- **Total VRAM:** 8192 MB
- **Reserved:** 2048 MB
- **Mimi:** 1024 MB
- **Model (1B @ F16):** 2048 MB
- **Remaining for Batching:** ~3112 MB
- **Per Batch Item:** 600 MB
- **Max Batch Size:** ~5

**Hypothesis:**
The OOM occurs during model loading or initialization.
1. **Casting Overhead:** If the source model is F32 (4GB) and we cast to F16 (2GB), we might momentarily hold both (6GB) plus Reserved (2GB) = 8GB -> OOM.
2. **Fragmentation:** Allocating large tensors (2GB contiguous) might fail if VRAM is fragmented.
3. **Mimi Overhead:** Mimi might use more than 1GB.

### Scenario B: Low-RAM Config
- Works despite potentially using F32 computation?
- If the source file is FP16 and we cast to F32, we use 4GB model.
- 2048 + 4096 + 1024 = 7168 MB.
- Remaining: ~1000 MB.
- Fits Batch Size 1.
- *Why does this work and HF fail?*
  - Maybe `dtype_override="f32"` prevents a costly cast if the file is mapped? (Unlikely).
  - Maybe the pre-converted FP16 file is smaller/cleaner to load?

## Action Plan
1. Increase `VRAM_RESERVED_MB` default to 2560 MB (2.5GB).
2. Allow `VRAM_RESERVED_MB` configuration via environment variable.
3. Add pre-flight check to fail early if VRAM is critically low.
4. Document the `MOSHI_VRAM_RESERVED_MB` variable.
