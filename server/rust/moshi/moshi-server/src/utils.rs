// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use anyhow::Result;
use candle::{DType, Device};

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
pub struct BuildInfo {
    build_timestamp: String,
    build_date: String,
    git_branch: String,
    git_timestamp: String,
    git_date: String,
    git_hash: String,
    git_describe: String,
    rustc_host_triple: String,
    rustc_version: String,
    cargo_target_triple: String,
}

impl BuildInfo {
    pub fn new() -> BuildInfo {
        BuildInfo {
            build_timestamp: String::from(env!("VERGEN_BUILD_TIMESTAMP")),
            build_date: String::from(env!("VERGEN_BUILD_DATE")),
            git_branch: String::from(env!("VERGEN_GIT_BRANCH")),
            git_timestamp: String::from(env!("VERGEN_GIT_COMMIT_TIMESTAMP")),
            git_date: String::from(env!("VERGEN_GIT_COMMIT_DATE")),
            git_hash: String::from(env!("VERGEN_GIT_SHA")),
            git_describe: String::from(env!("VERGEN_GIT_DESCRIBE")),
            rustc_host_triple: String::from(env!("VERGEN_RUSTC_HOST_TRIPLE")),
            rustc_version: String::from(env!("VERGEN_RUSTC_SEMVER")),
            cargo_target_triple: String::from(env!("VERGEN_CARGO_TARGET_TRIPLE")),
        }
    }

    /// Returns the git describe version string (e.g., "v0.6.4-5-gabcdef1")
    pub fn git_describe(&self) -> String {
        self.git_describe.clone()
    }
}

pub fn replace_env_vars(input: &str) -> String {
    let re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        std::env::var(var_name).unwrap_or_else(|_| "".to_string())
    })
    .to_string()
}

pub fn resolve_or_download(input: &str) -> Result<String> {
    let path = if let Some(path) = input.strip_prefix("hf://") {
        // Single file download from Hugging Face
        let s: Vec<&str> = path.split('/').collect();
        if s.len() < 3 {
            anyhow::bail!("unexpected format for hf path {input}")
        }
        let repo = format!("{}/{}", s[0], s[1]);
        let file = s[2..].join("/");
        let api = hf_hub::api::sync::ApiBuilder::from_env().build()?.model(repo);
        api.get(&file)?.to_string_lossy().to_string()
    } else if let Some(path) = input.strip_prefix("hf-snapshot://") {
        // Snapshot download from Hugging Face with optional glob pattern
        // Format: hf-snapshot://org/repo or hf-snapshot://org/repo/**/*.safetensors
        resolve_hf_snapshot(path)?
    } else {
        replace_env_vars(input)
    };
    Ok(path)
}

/// Resolve an hf-snapshot:// path, downloading matching files into the HF cache
/// and returning the local cache directory path.
///
/// Supports glob patterns like "org/repo/**/*.safetensors" to filter which files are downloaded.
pub fn resolve_hf_snapshot(input: &str) -> Result<String> {
    // Parse the repo/org and optional glob pattern
    // Examples:
    //   "kyutai/tts-voices" -> download all, return repo dir
    //   "kyutai/tts-voices/**/*.safetensors" -> download only matching files, return repo dir
    
    // Find where the glob pattern starts (first *, ?, or [)
    let glob_chars = ['*', '?', '['];
    let glob_start = input.find(|c| glob_chars.contains(&c));
    
    let (repo_path, glob_pattern) = match glob_start {
        Some(pos) => {
            // Find the last '/' before the glob pattern
            let repo_end = input[..pos].rfind('/').unwrap_or(pos);
            let repo_path = &input[..repo_end];
            let glob = &input[repo_end..].trim_start_matches('/');
            (repo_path.to_string(), Some(glob.to_string()))
        }
        None => (input.to_string(), None),
    };
    
    // Parse repo org/name
    let parts: Vec<&str> = repo_path.split('/').collect();
    if parts.len() < 2 {
        anyhow::bail!("unexpected format for hf-snapshot path, expected org/repo: {input}")
    }
    let repo = format!("{}/{}", parts[0], parts[1]);
    
    // Build the HF API client
    let api = hf_hub::api::sync::ApiBuilder::from_env().build()?.model(repo.clone());
    
    // Get the repo info to find all files
    let repo_info = api.info()?;
    
    // Collect files to download, applying glob pattern if specified
    let files_to_download: Vec<String> = if let Some(ref pattern) = glob_pattern {
        let glob = glob::Pattern::new(pattern)
            .map_err(|e| anyhow::anyhow!("invalid glob pattern '{}': {}", pattern, e))?;
        
        repo_info
            .siblings
            .iter()
            .filter_map(|sibling| {
                let rfilename = &sibling.rfilename;
                if glob.matches(rfilename) {
                    Some(rfilename.clone())
                } else {
                    None
                }
            })
            .collect()
    } else {
        // Download all files
        repo_info
            .siblings
            .iter()
            .map(|sibling| sibling.rfilename.clone())
            .collect()
    };
    
    if files_to_download.is_empty() {
        if glob_pattern.is_some() {
            tracing::warn!(
                repo = %repo,
                pattern = ?glob_pattern,
                "no files matched the glob pattern in hf-snapshot"
            );
        }
    } else {
        tracing::info!(
            repo = %repo,
            file_count = files_to_download.len(),
            pattern = ?glob_pattern,
            "downloading files from HuggingFace snapshot"
        );
        
        // Download each matching file
        for file in &files_to_download {
            tracing::debug!(file = %file, "downloading from HF");
            api.get(file)?;
        }
    }
    
    // Return the local cache directory for this repo
    // The HF hub caches files under ~/.cache/huggingface/hub/models--org--repo/snapshots/<revision>/
    // We can get the path by downloading any file and getting its parent directory
    if let Some(first_file) = files_to_download.first() {
        let local_path = api.get(first_file)?;
        // Walk up to find the snapshot directory (parent of the file's relative path)
        let mut snapshot_dir = local_path.clone();
        let depth = first_file.matches('/').count() + 1;
        for _ in 0..depth {
            snapshot_dir = snapshot_dir
                .parent()
                .ok_or_else(|| anyhow::anyhow!("failed to find snapshot directory"))?
                .to_path_buf();
        }
        Ok(snapshot_dir.to_string_lossy().to_string())
    } else {
        // No files to download, just return the repo cache path
        // We need to create a dummy request to get the cache location
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine cache directory"))?
            .join("huggingface")
            .join("hub")
            .join(format!("models--{}--{}", parts[0], parts[1]));
        Ok(cache_dir.to_string_lossy().to_string())
    }
}

#[allow(dead_code)]
fn walk_toml(t: &mut toml::Value, f: &impl Fn(&mut String) -> Result<()>) -> Result<()> {
    match t {
        toml::Value::Table(t) => {
            for (_, t) in t.iter_mut() {
                walk_toml(t, f)?;
            }
        }
        toml::Value::Array(a) => {
            for t in a.iter_mut() {
                walk_toml(t, f)?
            }
        }
        toml::Value::String(s) => f(s)?,
        toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
    Ok(())
}

#[allow(dead_code)]
pub fn resolve_or_download_toml(t: &mut toml::Table) -> Result<()> {
    for (_, t) in t.iter_mut() {
        walk_toml(t, &|s: &mut String| -> Result<()> {
            *s = resolve_or_download(s)?;
            Ok(())
        })?;
    }
    Ok(())
}

pub struct WrapJson<T>(pub Result<T>);

impl<T: serde::Serialize> axum::response::IntoResponse for WrapJson<T> {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Ok(v) => axum::Json(v).into_response(),
            Err(err) => {
                tracing::error!(?err, "returning internal server error 500");
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("{err}")).into_response()
            }
        }
    }
}

pub struct AxumError(anyhow::Error);

impl axum::response::IntoResponse for AxumError {
    fn into_response(self) -> axum::response::Response {
        let err = self.0;
        tracing::error!(?err);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}")).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AxumError {
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

pub type AxumResult<R> = std::result::Result<R, AxumError>;

fn conv<T>(samples: &mut Vec<f32>, data: std::borrow::Cow<symphonia::core::audio::AudioBuffer<T>>)
where
    T: symphonia::core::sample::Sample,
    f32: symphonia::core::conv::FromSample<T>,
{
    use symphonia::core::audio::Signal;
    use symphonia::core::conv::FromSample;
    samples.extend(data.chan(0).iter().map(|v| f32::from_sample(*v)))
}

pub fn pcm_decode(bytes: axum::body::Bytes) -> anyhow::Result<(Vec<f32>, u32)> {
    use symphonia::core::audio::{AudioBufferRef, Signal};

    let source = std::io::Cursor::new(bytes);
    let mss = symphonia::core::io::MediaSourceStream::new(Box::new(source), Default::default());
    let hint = symphonia::core::probe::Hint::new();
    let meta_opts: symphonia::core::meta::MetadataOptions = Default::default();
    let fmt_opts: symphonia::core::formats::FormatOptions = Default::default();
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .expect("no supported audio tracks");
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .expect("unsupported codec");
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut pcm_data = Vec::new();
    while let Ok(packet) = format.next_packet() {
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet)? {
            AudioBufferRef::F32(buf) => pcm_data.extend(buf.chan(0)),
            AudioBufferRef::U8(data) => conv(&mut pcm_data, data),
            AudioBufferRef::U16(data) => conv(&mut pcm_data, data),
            AudioBufferRef::U24(data) => conv(&mut pcm_data, data),
            AudioBufferRef::U32(data) => conv(&mut pcm_data, data),
            AudioBufferRef::S8(data) => conv(&mut pcm_data, data),
            AudioBufferRef::S16(data) => conv(&mut pcm_data, data),
            AudioBufferRef::S24(data) => conv(&mut pcm_data, data),
            AudioBufferRef::S32(data) => conv(&mut pcm_data, data),
            AudioBufferRef::F64(data) => conv(&mut pcm_data, data),
        }
    }
    Ok((pcm_data, sample_rate))
}

pub fn spawn<F>(name: &'static str, future: F) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = Result<()>> + Send + 'static,
{
    tokio::task::spawn(async move {
        match future.await {
            Ok(_) => tracing::debug!(?name, "task completed successfully"),
            Err(err) => tracing::error!(?name, ?err, "task failed"),
        }
    })
}

// ============================================================================
// WebSocket Close Helpers
// ============================================================================

use crate::protocol::CloseCode;
use axum::extract::ws;
use futures_util::SinkExt;

/// Closes a WebSocket connection with a specific close code and reason.
/// This is a helper to ensure consistent close frame handling across all handlers.
///
/// # Arguments
/// * `sender` - The WebSocket sender (split from the socket)
/// * `code` - The close code to send
/// * `reason` - Optional custom reason message (uses default if None)
///
/// # Returns
/// Result indicating if the close frame was sent successfully
pub async fn close_with_reason<S>(
    sender: &mut S,
    code: CloseCode,
    reason: Option<&str>,
) -> Result<()>
where
    S: SinkExt<ws::Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let frame = match reason {
        Some(r) => code.with_reason(r),
        None => code.to_close_frame(),
    };

    tracing::info!(
        code = code.code(),
        reason = %frame.reason,
        retryable = code.is_retryable(),
        "closing WebSocket connection"
    );

    sender
        .send(ws::Message::Close(Some(frame)))
        .await
        .map_err(|e| anyhow::anyhow!("failed to send close frame: {}", e))?;

    Ok(())
}

/// Closes a WebSocket connection with a close code using the default reason.
#[allow(dead_code)]
pub async fn close_connection<S>(sender: &mut S, code: CloseCode) -> Result<()>
where
    S: SinkExt<ws::Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    close_with_reason(sender, code, None).await
}

pub fn spawn_blocking<F>(name: &'static str, f: F) -> tokio::task::JoinHandle<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    tokio::task::spawn_blocking(move || match f() {
        Ok(_) => tracing::debug!(?name, "task completed successfully"),
        Err(err) => tracing::error!(?name, ?err, "task failed"),
    })
}

pub fn model_dtype(over: Option<&str>, dev: &Device) -> Result<DType> {
    let dtype = match over {
        None => dev.bf16_default_to_f32(),
        Some(s) => {
            use std::str::FromStr;
            DType::from_str(s)?
        }
    };
    Ok(dtype)
}

// ============================================================================
// GPU Auto-Configuration Constants
// ============================================================================

/// Reserved VRAM for CUDA runtime, driver overhead, and safety margin.
/// This accounts for CUDA context, kernel launches, and other system allocations.
/// Default set to 2560MB (2.5GB) to reduce the risk of OOM on 8GB cards.
pub const DEFAULT_VRAM_RESERVED_MB: u64 = 2560;

/// Default per-batch-item memory cost in MB.
/// This covers activations, KV cache, and intermediate tensors per concurrent stream.
/// Empirically tuned for Moshi ASR/TTS workloads.
pub const DEFAULT_PER_BATCH_ITEM_MB: u64 = 600;

/// Default model size in billions of parameters.
/// Use 1.0 for stt-1b-* models, 2.6 for stt-2.6b-* models.
/// Override with MOSHI_MODEL_PARAMS_BILLIONS env var.
pub const DEFAULT_MODEL_PARAMS_BILLIONS: f64 = 1.0;

/// Estimated memory usage for Mimi audio tokenizer in MB.
/// Mimi (~200M params) + decoder + buffers roughly take 1GB in F32.
pub const DEFAULT_MIMI_ESTIMATE_MB: u64 = 1024;

// ============================================================================
// GPU Information Struct
// ============================================================================

/// GPU information for auto-configuration
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// Free VRAM in bytes
    pub free_vram: u64,
    /// Total VRAM in bytes
    pub total_vram: u64,
    /// GPU name (e.g., "NVIDIA GeForce RTX 2070")
    pub name: String,
    /// Compute capability major version (e.g., 7 for SM 7.5)
    pub compute_major: u32,
    /// Compute capability minor version (e.g., 5 for SM 7.5)
    pub compute_minor: u32,
    /// GPU utilization (percent)
    pub utilization: u32,
}

impl GpuInfo {
    /// Returns the SM version as a single number (e.g., 75 for SM 7.5)
    pub fn sm_version(&self) -> u32 {
        self.compute_major * 10 + self.compute_minor
    }

    /// Returns free VRAM in MB
    pub fn free_vram_mb(&self) -> u64 {
        self.free_vram / (1024 * 1024)
    }

    /// Returns total VRAM in MB
    pub fn total_vram_mb(&self) -> u64 {
        self.total_vram / (1024 * 1024)
    }

    /// Returns true if the GPU supports native BF16 operations.
    /// Ampere (SM 8.0+) and later have native BF16 tensor core support.
    /// However, candle's CUDA kernels may lack BF16 RMS Norm implementation,
    /// so we're conservative and require SM 8.0+ for BF16.
    pub fn supports_bf16(&self) -> bool {
        // SM 8.0 (Ampere) and above have native BF16 support
        // SM 7.x (Turing/Volta) can emulate but lacks native support
        self.compute_major >= 8
    }

    /// Returns the recommended dtype string for this GPU.
    /// Returns "bf16" for SM 8.0+, "f16" for SM 7.0+, "f32" for older GPUs.
    pub fn recommended_dtype(&self) -> &'static str {
        if self.supports_bf16() {
            "bf16"
        } else if self.compute_major >= 7 {
            "f16"
        } else {
            "f32"
        }
    }

    /// Returns bytes per parameter for the recommended dtype.
    pub fn dtype_bytes(&self) -> u64 {
        match self.recommended_dtype() {
            "bf16" | "f16" => 2,
            _ => 4,
        }
    }

    /// Calculates the recommended batch size based on available VRAM.
    ///
    /// # Memory Layout
    /// ```text
    /// Total Free VRAM
    /// ├── Reserved (CUDA runtime, driver)     = MOSHI_VRAM_RESERVED_MB
    /// ├── Model Weights (fixed cost)          = model_params × dtype_bytes
    /// └── Per-Batch Memory (scales with N)    = N × per_batch_item_mb
    /// ```
    ///
    /// # Arguments
    /// * `model_params_billions` - Model size in billions of parameters (e.g., 1.0 for 1B)
    /// * `per_batch_item_mb` - Memory per batch item in MB (activations, KV cache)
    ///
    /// # Returns
    /// A `BatchSizeCalculation` struct with the breakdown and recommended batch size.
    pub fn calculate_batch_size(
        &self,
        model_params_billions: f64,
        per_batch_item_mb: u64,
    ) -> BatchSizeCalculation {
        let dtype_bytes = self.dtype_bytes();
        let free_vram_mb = self.free_vram_mb();

        // Read configuration from env or use defaults
        let reserved_mb: u64 = std::env::var("MOSHI_VRAM_RESERVED_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_VRAM_RESERVED_MB);

        let mimi_mb: u64 = std::env::var("MOSHI_MIMI_ESTIMATE_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MIMI_ESTIMATE_MB);

        // Adjust per_batch_item_mb based on dtype
        // We assume the default (600MB) is tuned for F16/BF16 (2 bytes).
        // For F32 (4 bytes), we double it.
        // For Q8/Q4 (1 byte or less), we could halve it, but we'll be conservative and floor at 2 bytes divisor.
        let adjusted_per_batch_item_mb = (per_batch_item_mb * dtype_bytes) / 2;

        // Model weights in MB: params × bytes_per_param / 1M
        let model_weights_mb = ((model_params_billions * 1e9) as u64 * dtype_bytes) / (1024 * 1024);

        // Available for batching = free - reserved - model_weights - mimi
        let available_for_batching_mb = free_vram_mb
            .saturating_sub(reserved_mb)
            .saturating_sub(model_weights_mb)
            .saturating_sub(mimi_mb);

        // Batch size = available / per_item_cost
        let max_batch_size = if adjusted_per_batch_item_mb > 0 {
            (available_for_batching_mb / adjusted_per_batch_item_mb) as usize
        } else {
            1
        };

        let recommended_batch_size = max_batch_size.max(1);

        // Warn if we are potentially oversubscribing VRAM even at batch size 1
        if available_for_batching_mb == 0 {
            tracing::warn!(
                "Available VRAM for batching is 0MB. Defaulting to batch size 1, but OOM is likely. \
                 Consider reducing VRAM_RESERVED_MB or using a smaller model."
            );
        }

        BatchSizeCalculation {
            free_vram_mb,
            reserved_mb,
            model_weights_mb,
            per_batch_item_mb: adjusted_per_batch_item_mb,
            available_for_batching_mb,
            recommended_batch_size,
            dtype_bytes,
            model_params_billions,
            mimi_mb,
        }
    }

    /// Prints a formatted summary of GPU capabilities to the tracing log.
    /// Note: Prefer `log_combined_summary` when model info is available.
    #[allow(dead_code)]
    pub fn log_summary(&self) {
        let header = "═".repeat(60);
        let line = "─".repeat(60);

        tracing::info!("\n{}", header);
        tracing::info!("  GPU AUTO-DETECTION SUMMARY");
        tracing::info!("{}", line);
        tracing::info!("  Device:          {}", self.name);
        tracing::info!(
            "  Compute:         SM {} (CUDA {}.{})",
            self.sm_version(),
            self.compute_major,
            self.compute_minor
        );
        tracing::info!("  Total VRAM:      {} MB", self.total_vram_mb());
        tracing::info!("  Free VRAM:       {} MB", self.free_vram_mb());
        tracing::info!("{}", line);
        tracing::info!(
            "  BF16 Support:    {}",
            if self.supports_bf16() { "Yes (Ampere+)" } else { "No (Turing/older)" }
        );
        tracing::info!(
            "  Selected DType:  {} ({} bytes/param)",
            self.recommended_dtype(),
            self.dtype_bytes()
        );
        tracing::info!("{}", header);
    }
}

// ============================================================================
// Batch Size Calculation Result
// ============================================================================

/// Detailed breakdown of batch size calculation for transparency.
#[derive(Debug, Clone)]
pub struct BatchSizeCalculation {
    /// Free VRAM detected (MB)
    pub free_vram_mb: u64,
    /// Reserved for CUDA runtime (MB)
    pub reserved_mb: u64,
    /// Model weights memory footprint (MB)
    pub model_weights_mb: u64,
    /// Per-batch-item memory cost (MB)
    pub per_batch_item_mb: u64,
    /// VRAM available for batch items (MB)
    pub available_for_batching_mb: u64,
    /// Recommended batch size
    pub recommended_batch_size: usize,
    /// Bytes per parameter
    pub dtype_bytes: u64,
    /// Model size in billions of parameters
    pub model_params_billions: f64,
    /// Mimi estimate used (MB)
    pub mimi_mb: u64,
}

impl BatchSizeCalculation {
    /// Logs the calculation breakdown with visual formatting.
    pub fn log_breakdown(&self) {
        let line = "─".repeat(60);

        tracing::info!("\n{}", line);
        tracing::info!("  BATCH SIZE CALCULATION");
        tracing::info!("{}", line);
        tracing::info!(
            "  Model: {:.1}B params × {} bytes = {} MB",
            self.model_params_billions,
            self.dtype_bytes,
            self.model_weights_mb
        );
        tracing::info!("  + Mimi (est):      {:>6} MB", self.mimi_mb);
        tracing::info!("");
        tracing::info!("  Free VRAM:         {:>6} MB", self.free_vram_mb);
        tracing::info!("  − Reserved:        {:>6} MB  (CUDA runtime)", self.reserved_mb);
        tracing::info!("  − Model weights:   {:>6} MB  (fixed cost)", self.model_weights_mb);
        tracing::info!("  ────────────────────────────");
        tracing::info!(
            "  = Available:       {:>6} MB  (for batching)",
            self.available_for_batching_mb
        );
        tracing::info!("");
        tracing::info!(
            "  Per-batch cost:    {:>6} MB  (activations + KV cache)",
            self.per_batch_item_mb
        );
        tracing::info!(
            "  Max batch size:    {:>6}     ({} MB ÷ {} MB)",
            self.recommended_batch_size,
            self.available_for_batching_mb,
            self.per_batch_item_mb
        );
        tracing::info!("{}", line);
    }
}

// ============================================================================
// Model Information for Logging
// ============================================================================

/// Model information extracted from config for logging purposes.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Path to the LM model file
    pub model_file: String,
    /// Model dimensions (d_model from transformer config)
    pub d_model: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Number of transformer layers
    pub num_layers: usize,
    /// Feed-forward dimension
    pub dim_feedforward: usize,
    /// Number of audio codebooks
    pub audio_codebooks: usize,
    /// Whether depformer is present
    pub has_depformer: bool,
    /// Depformer layers (if present)
    pub depformer_layers: Option<usize>,
}

impl ModelInfo {
    /// Creates ModelInfo from LmConfig
    pub fn from_lm_config(config: &crate::LmConfig) -> Self {
        let model = &config.model;
        Self {
            model_file: config.lm_model_file.clone(),
            d_model: model.transformer.d_model,
            num_heads: model.transformer.num_heads,
            num_layers: model.transformer.num_layers,
            dim_feedforward: model.transformer.dim_feedforward,
            audio_codebooks: model.audio_codebooks,
            has_depformer: model.depformer.is_some(),
            depformer_layers: model.depformer.as_ref().map(|d| d.transformer.num_layers),
        }
    }

    /// Creates ModelInfo from AsrConfig
    pub fn from_asr_config(config: &crate::AsrConfig) -> Self {
        let model = &config.model;
        Self {
            model_file: config.lm_model_file.clone(),
            d_model: model.transformer.d_model,
            num_heads: model.transformer.num_heads,
            num_layers: model.transformer.num_layers,
            dim_feedforward: model.transformer.dim_feedforward,
            audio_codebooks: model.audio_codebooks,
            has_depformer: model.depformer.is_some(),
            depformer_layers: model.depformer.as_ref().map(|d| d.transformer.num_layers),
        }
    }

    /// Creates ModelInfo from TtsConfig
    pub fn from_tts_config(config: &crate::TtsConfig) -> Self {
        let model = &config.model;
        Self {
            model_file: config.lm_model_file.clone(),
            d_model: model.transformer.d_model,
            num_heads: model.transformer.num_heads,
            num_layers: model.transformer.num_layers,
            dim_feedforward: model.transformer.dim_feedforward,
            audio_codebooks: model.audio_codebooks,
            has_depformer: model.depformer.is_some(),
            depformer_layers: model.depformer.as_ref().map(|d| d.transformer.num_layers),
        }
    }

    /// Extracts just the filename from the model path
    pub fn model_name(&self) -> &str {
        std::path::Path::new(&self.model_file)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.model_file)
    }

    /// Estimates parameter count in billions based on architecture
    /// Formula: 12 * L * d² (approximate for transformer-based LMs)
    /// This is a rough estimate; actual param count depends on exact architecture
    pub fn estimated_params_billions(&self) -> f64 {
        let l = self.num_layers as f64;
        let d = self.d_model as f64;
        // Standard transformer: ~12 * L * d² params (embeddings, attention, FFN)
        let main_params = 12.0 * l * d * d;

        // Add depformer params if present
        let depformer_params = if let Some(dep_layers) = self.depformer_layers {
            // Depformer typically has smaller d_model (1024)
            12.0 * (dep_layers as f64) * 1024.0 * 1024.0
        } else {
            0.0
        };

        (main_params + depformer_params) / 1e9
    }

    /// Logs model details with visual formatting matching GpuInfo::log_summary
    pub fn log_summary(&self) {
        let line = "─".repeat(60);

        tracing::info!("  MODEL CONFIGURATION");
        tracing::info!("{}", line);
        tracing::info!("  Model File:      {}", self.model_name());
        tracing::info!(
            "  Architecture:    Transformer (d={}, h={}, L={})",
            self.d_model,
            self.num_heads,
            self.num_layers
        );
        tracing::info!("  FFN Dimension:   {}", self.dim_feedforward);
        tracing::info!("  Audio Codebooks: {}", self.audio_codebooks);
        if self.has_depformer {
            tracing::info!(
                "  Depformer:       Yes ({} layers)",
                self.depformer_layers.unwrap_or(0)
            );
        } else {
            tracing::info!("  Depformer:       No");
        }
        tracing::info!("  Est. Params:     {:.2}B", self.estimated_params_billions());
    }
}

/// Extended GPU summary that includes model info
impl GpuInfo {
    /// Prints a combined summary of GPU capabilities and model details.
    pub fn log_combined_summary(&self, model_info: Option<&ModelInfo>) {
        let header = "═".repeat(60);
        let line = "─".repeat(60);

        tracing::info!("\n{}", header);
        tracing::info!("  GPU AUTO-DETECTION SUMMARY");
        tracing::info!("{}", line);
        tracing::info!("  Device:          {}", self.name);
        tracing::info!(
            "  Compute:         SM {} (CUDA {}.{})",
            self.sm_version(),
            self.compute_major,
            self.compute_minor
        );
        tracing::info!("  Total VRAM:      {} MB", self.total_vram_mb());
        tracing::info!("  Free VRAM:       {} MB", self.free_vram_mb());
        tracing::info!("{}", line);
        tracing::info!(
            "  BF16 Support:    {}",
            if self.supports_bf16() { "Yes (Ampere+)" } else { "No (Turing/older)" }
        );
        tracing::info!(
            "  Selected DType:  {} ({} bytes/param)",
            self.recommended_dtype(),
            self.dtype_bytes()
        );

        if let Some(info) = model_info {
            tracing::info!("{}", line);
            info.log_summary();
        }

        tracing::info!("{}", header);
    }
}

#[cfg(feature = "cuda")]
pub fn get_gpu_info() -> Result<GpuInfo> {
    use nvml_wrapper::Nvml;
    let nvml = Nvml::init()?;
    let device = nvml.device_by_index(0)?;
    let memory_info = device.memory_info()?;
    let utilization = device.utilization_rates().map(|u| u.gpu).unwrap_or(0);
    let name = device.name()?;
    let compute_cap = device.cuda_compute_capability()?;

    Ok(GpuInfo {
        free_vram: memory_info.free,
        total_vram: memory_info.total,
        name,
        compute_major: compute_cap.major as u32,
        compute_minor: compute_cap.minor as u32,
        utilization,
    })
}

#[cfg(not(feature = "cuda"))]
pub fn get_gpu_info() -> Result<GpuInfo> {
    anyhow::bail!("CUDA not available")
}

#[cfg(feature = "cuda")]
#[allow(dead_code)]
pub fn get_available_vram() -> Result<u64> {
    get_gpu_info().map(|info| info.free_vram)
}

#[cfg(not(feature = "cuda"))]
#[allow(dead_code)]
pub fn get_available_vram() -> Result<u64> {
    anyhow::bail!("CUDA not available")
}
