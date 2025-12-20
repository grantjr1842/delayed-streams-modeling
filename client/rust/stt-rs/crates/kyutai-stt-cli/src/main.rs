use anyhow::Result;
use clap::Parser;
use kyutai_stt_client::audio::{
    AudioLevel, LevelMeter, MicCapture, MicCaptureConfig, ResampleQuality,
};
use kyutai_stt_client::protocol::InMsg;
use kyutai_stt_client::{SttClientBuilder, SttEvent};
#[cfg(feature = "hq-resample")]
use rubato::Resampler;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep_until};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod auth;

const OUTPUT_SAMPLE_RATE_HZ: usize = 24_000;
const OUTPUT_CHUNK_SAMPLES: usize = 1920;
const FILE_INPUT_CHUNK_SAMPLES: usize = 4096;
const LEVEL_RENDER_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_RENDER_INTERVAL: Duration = Duration::from_millis(200);
const TRANSCRIPT_FLUSH_INTERVAL: Duration = Duration::from_millis(200);

fn silence_samples_from_ms(prefix_ms: u64, sample_rate_hz: usize) -> usize {
    let samples = (prefix_ms as u128 * sample_rate_hz as u128 + 999) / 1000;
    samples as usize
}

fn ensure_hq_resample_enabled(requested: bool) -> Result<()> {
    if !requested {
        return Ok(());
    }

    #[cfg(feature = "hq-resample")]
    {
        Ok(())
    }

    #[cfg(not(feature = "hq-resample"))]
    {
        Err(anyhow::anyhow!(
            "hq-resample requested but the CLI was built without the hq-resample feature"
        ))
    }
}

async fn send_silence_prefix(
    sender: &kyutai_stt_client::SttSender,
    prefix_ms: u64,
) -> Result<()> {
    let total_samples = silence_samples_from_ms(prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
    if total_samples == 0 {
        return Ok(());
    }

    let chunk_duration =
        Duration::from_secs_f64(OUTPUT_CHUNK_SAMPLES as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);
    let chunk_count = (total_samples + OUTPUT_CHUNK_SAMPLES - 1) / OUTPUT_CHUNK_SAMPLES;
    let silence_chunk = vec![0.0f32; OUTPUT_CHUNK_SAMPLES];

    for idx in 0..chunk_count {
        sender.send(InMsg::Audio { pcm: silence_chunk.clone() }).await?;
        if idx + 1 < chunk_count {
            tokio::time::sleep(chunk_duration).await;
        }
    }

    Ok(())
}

struct LinearResampler {
    in_rate_hz: u32,
    out_rate_hz: u32,
    step: f64,
    pos: f64,
    buf: Vec<f32>,
}

impl LinearResampler {
    fn new(in_rate_hz: u32, out_rate_hz: u32) -> Self {
        Self {
            in_rate_hz,
            out_rate_hz,
            step: in_rate_hz as f64 / out_rate_hz as f64,
            pos: 0.0,
            buf: Vec::new(),
        }
    }

    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) {
        out.clear();
        if input.is_empty() && self.buf.is_empty() {
            return;
        }

        if !input.is_empty() {
            self.buf.extend_from_slice(input);
        }

        let approx_out_len = ((input.len() as u64 * self.out_rate_hz as u64)
            / self.in_rate_hz.max(1) as u64)
            .saturating_add(2) as usize;
        out.reserve(approx_out_len);

        while self.pos + 1.0 < self.buf.len() as f64 {
            let i = self.pos.floor() as usize;
            let frac = self.pos - i as f64;

            let a = self.buf[i];
            let b = self.buf[i + 1];

            out.push(a + (b - a) * frac as f32);
            self.pos += self.step;
        }

        let drain = self.pos.floor() as usize;
        if drain > 0 {
            self.buf.drain(0..drain);
            self.pos -= drain as f64;
        }
    }
}

#[cfg(feature = "hq-resample")]
struct HqResampler {
    resampler: rubato::FftFixedInOut<f32>,
    pending: Vec<f32>,
}

#[cfg(feature = "hq-resample")]
impl HqResampler {
    const CHUNK_SIZE: usize = 1024;

    fn new(in_rate_hz: u32, out_rate_hz: u32) -> Result<Self> {
        let resampler = rubato::FftFixedInOut::<f32>::new(
            in_rate_hz as usize,
            out_rate_hz as usize,
            Self::CHUNK_SIZE,
            1,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        Ok(Self {
            resampler,
            pending: Vec::new(),
        })
    }

    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        out.clear();
        if input.is_empty() {
            return Ok(());
        }

        self.pending.extend_from_slice(input);
        let chunk_size = self.resampler.input_frames_next();
        if chunk_size == 0 {
            return Ok(());
        }

        while self.pending.len() >= chunk_size {
            let chunk = &self.pending[..chunk_size];
            let resampled = self.resampler.process(&[chunk], None)?;
            if let Some(channel) = resampled.get(0) {
                out.extend_from_slice(channel);
            }
            self.pending.drain(..chunk_size);
        }

        Ok(())
    }

    fn flush_into(&mut self, out: &mut Vec<f32>) -> Result<()> {
        out.clear();
        if self.pending.is_empty() {
            return Ok(());
        }

        let resampled = self.resampler.process_partial(Some(&[self.pending.as_slice()]), None)?;
        if let Some(channel) = resampled.get(0) {
            out.extend_from_slice(channel);
        }
        self.pending.clear();
        Ok(())
    }
}

enum FileResampler {
    Linear(LinearResampler),
    #[cfg(feature = "hq-resample")]
    High(HqResampler),
}

impl FileResampler {
    fn new(in_rate_hz: u32, out_rate_hz: u32, hq: bool) -> Result<Option<Self>> {
        if in_rate_hz == out_rate_hz {
            return Ok(None);
        }

        if hq {
            #[cfg(feature = "hq-resample")]
            {
                return Ok(Some(Self::High(HqResampler::new(
                    in_rate_hz,
                    out_rate_hz,
                )?)));
            }
            #[cfg(not(feature = "hq-resample"))]
            {
                return Err(anyhow::anyhow!(
                    "hq-resample requested but the CLI was built without the hq-resample feature"
                ));
            }
        }

        Ok(Some(Self::Linear(LinearResampler::new(
            in_rate_hz,
            out_rate_hz,
        ))))
    }

    fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        match self {
            FileResampler::Linear(resampler) => {
                resampler.process_into(input, out);
                Ok(())
            }
            #[cfg(feature = "hq-resample")]
            FileResampler::High(resampler) => resampler.process_into(input, out),
        }
    }

    fn flush_into(&mut self, out: &mut Vec<f32>) -> Result<()> {
        match self {
            FileResampler::Linear(resampler) => {
                resampler.process_into(&[], out);
                Ok(())
            }
            #[cfg(feature = "hq-resample")]
            FileResampler::High(resampler) => resampler.flush_into(out),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// WebSocket URL for the STT server
    #[arg(long, default_value = "ws://localhost:8080/api/asr-streaming")]
    url: String,

    /// Bearer token for authentication
    #[arg(long)]
    auth_token: Option<String>,

    /// Query parameter token for authentication
    #[arg(long)]
    query_token: Option<String>,

    /// BETTER_AUTH_SECRET for generating JWT tokens
    #[arg(long, env = "BETTER_AUTH_SECRET")]
    secret: Option<String>,

    /// Environment name for loading .env.<env> when generating tokens
    #[arg(long, env = "ENV")]
    env: Option<String>,

    /// Buffer transcript output and flush periodically (reduces overhead)
    #[arg(long)]
    buffered_output: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Stream audio from microphone
    Mic(MicArgs),
    /// Stream audio from file
    File(FileArgs),
    /// Generate a JWT token
    Token(TokenArgs),
    /// Test microphone input (no server required)
    MicTest(MicTestArgs),
}

#[derive(clap::Args, Debug)]
struct MicArgs {
    /// Show timestamps with each word
    #[arg(long)]
    timestamps: bool,

    /// Show real-time input level meter
    #[arg(long)]
    show_level: bool,

    /// Auto-generate a token using --secret/BETTER_AUTH_SECRET
    #[arg(long)]
    auto_token: bool,

    /// Enable verbose logging of model performance parameters (VAD steps)
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Prepend silence before streaming (milliseconds, rounded to 80ms chunks)
    #[arg(long, default_value = "0")]
    silence_prefix_ms: u64,

    /// Use high-quality resampling (requires `--features hq-resample`)
    #[arg(long)]
    hq_resample: bool,
}

#[derive(clap::Args, Debug)]
struct FileArgs {
    path: PathBuf,

    #[arg(long)]
    rtf: Option<f64>,

    /// Show file streaming progress/status line
    #[arg(long)]
    progress: bool,

    /// Auto-generate a token using --secret/BETTER_AUTH_SECRET
    #[arg(long)]
    auto_token: bool,

    /// Enable verbose logging of model performance parameters (VAD steps)
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Prepend silence before streaming (milliseconds, rounded to 80ms chunks)
    #[arg(long, default_value = "0")]
    silence_prefix_ms: u64,

    /// Use high-quality resampling (requires `--features hq-resample`)
    #[arg(long)]
    hq_resample: bool,
}

#[derive(clap::Args, Debug)]
struct TokenArgs {
    /// Token validity in hours
    #[arg(long, default_value = "1.0")]
    hours: f64,
}

#[derive(clap::Args, Debug)]
struct MicTestArgs {
    /// Duration to run the test in seconds (0 = until Ctrl+C)
    #[arg(long, default_value = "0")]
    duration: u64,

    /// Show real-time input level meter
    #[arg(long, default_value = "true")]
    show_level: bool,

    /// Save recorded audio to a WAV file
    #[arg(long)]
    save_wav: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    info!(url = %args.url, "kyutai-stt-cli starting");

    match args.command {
        Command::Mic(mic_args) => {
            let auth_token = resolve_auth_token(
                &args.auth_token,
                &args.secret,
                args.env.as_deref(),
                mic_args.auto_token,
            )?;
            run_mic(
                args.url,
                auth_token,
                args.query_token,
                mic_args,
                args.buffered_output,
            )
            .await?
        }
        Command::File(file_args) => {
            let auth_token = resolve_auth_token(
                &args.auth_token,
                &args.secret,
                args.env.as_deref(),
                file_args.auto_token,
            )?;
            run_file(
                args.url,
                auth_token,
                args.query_token,
                file_args,
                args.buffered_output,
            )
            .await?
        }
        Command::Token(token_args) => run_token(&args.secret, args.env.as_deref(), token_args)?,
        Command::MicTest(mic_test_args) => run_mic_test(mic_test_args).await?,
    }

    Ok(())
}

/// Resolve the auth token: use provided token, or generate one if --auto-token is set.
fn resolve_auth_token(
    auth_token: &Option<String>,
    secret: &Option<String>,
    env_name: Option<&str>,
    auto_token: bool,
) -> Result<Option<String>> {
    if let Some(token) = auth_token {
        return Ok(Some(token.clone()));
    }

    if auto_token {
        let secret = resolve_secret(secret, env_name)?;
        let token = auth::generate_token(&secret, 1.0)
            .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;
        eprintln!("Generated auth token (valid for 1 hour)");
        return Ok(Some(token));
    }

    Ok(None)
}

/// Generate and print a JWT token.
fn run_token(secret: &Option<String>, env_name: Option<&str>, args: TokenArgs) -> Result<()> {
    let secret = resolve_secret(secret, env_name)?;

    let token = auth::generate_token(&secret, args.hours)
        .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;

    println!("{token}");
    Ok(())
}

fn resolve_secret(secret: &Option<String>, env_name: Option<&str>) -> Result<String> {
    if let Some(secret) = secret.as_deref() {
        return Ok(secret.to_string());
    }

    if let Some(secret) = load_secret_from_env_files(env_name)? {
        return Ok(secret);
    }

    Err(anyhow::anyhow!(
        "--secret/BETTER_AUTH_SECRET or .env(.<env>) with BETTER_AUTH_SECRET is required"
    ))
}

fn load_secret_from_env_files(env_name: Option<&str>) -> Result<Option<String>> {
    let env_name = env_name.unwrap_or("development");
    let candidates = [format!(".env.{env_name}"), ".env".to_string()];

    for file_name in candidates {
        let path = std::path::Path::new(&file_name);
        if !path.exists() {
            continue;
        }
        if let Some(secret) = read_env_value(path, "BETTER_AUTH_SECRET")? {
            return Ok(Some(secret));
        }
    }

    Ok(None)
}

fn read_env_value(path: &std::path::Path, key: &str) -> Result<Option<String>> {
    let contents = std::fs::read_to_string(path)?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() == key {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            return Ok(Some(value.to_string()));
        }
    }
    Ok(None)
}

async fn run_mic(
    url: String,
    auth_token: Option<String>,
    query_token: Option<String>,
    mic_args: MicArgs,
    buffered_output: bool,
) -> Result<()> {
    ensure_hq_resample_enabled(mic_args.hq_resample)?;

    let mut builder = SttClientBuilder::new().url(url);

    if let Some(token) = auth_token {
        builder = builder.auth_token(token);
    }
    if let Some(token) = query_token {
        builder = builder.query_token(token);
    }

    eprintln!("Connecting to STT server...");
    let session = builder.connect().await?;
    let mut events = session.into_event_stream();
    eprintln!("Connected! Listening for speech... (Ctrl+C to stop)");

    let sender = events.sender();
    if mic_args.silence_prefix_ms > 0 {
        eprintln!(
            "Sending {}ms of silence before streaming...",
            mic_args.silence_prefix_ms
        );
        send_silence_prefix(&sender, mic_args.silence_prefix_ms).await?;
    }

    let resample_quality = if mic_args.hq_resample {
        ResampleQuality::High
    } else {
        ResampleQuality::Linear
    };
    let mut mic = MicCapture::start_default_with_config(MicCaptureConfig {
        resample_quality,
    })?;
    let stderr_is_tty = std::io::stderr().is_terminal();
    let show_level = mic_args.show_level && stderr_is_tty;
    let stdout_is_tty = std::io::stdout().is_terminal();
    let mut transcript = TranscriptOutput::new(buffered_output || !stdout_is_tty);

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Optional level channel
    let (level_tx, level_rx) = if show_level {
        let (tx, rx) = mpsc::channel::<AudioLevel>(16);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Audio capture task
    let audio_task = tokio::spawn({
        let level_tx = level_tx.clone();
        async move {
            let mut meter = LevelMeter::default();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                    chunk = mic.recv() => {
                        let Some(chunk) = chunk else {
                            break;
                        };

                        // Compute and send level if enabled
                        if let Some(tx) = &level_tx {
                            let level = meter.process(&chunk.samples);
                            let _ = tx.try_send(level);
                        }

                        sender
                            .send(InMsg::Audio { pcm: chunk.samples })
                            .await?;
                    }
                }
            }

            kyutai_stt_client::Result::<()>::Ok(())
        }
    });

    // Level display task (if enabled)
    let level_task = level_rx.map(|rx| spawn_level_task(rx, stderr_is_tty));

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            ev = events.recv() => {
                let ev = ev?;
                match ev {
                    SttEvent::WordReceived { text, start_ms } => {
                        // Clear level meter line before printing word
                        if show_level {
                            clear_status_line(stderr_is_tty);
                        }
                        if mic_args.timestamps {
                            transcript.write_timestamped(start_ms, &text)?;
                        } else {
                            transcript.write_word(&text)?;
                        }
                    }
                    SttEvent::Error { message } => {
                        if show_level {
                            clear_status_line(stderr_is_tty);
                        }
                        transcript.flush()?;
                        eprintln!("stt error: {message}");
                    }
                    SttEvent::VadStep { step_idx, prs, buffered_pcm } => {
                        if mic_args.verbose {
                            info!(
                                step = step_idx,
                                buffered_samples = buffered_pcm,
                                "VAD step: prs={:?}",
                                prs
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);
    drop(level_tx); // Signal level task to stop

    audio_task
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))??;

    if let Some(task) = level_task {
        let _ = task.await;
    }

    transcript.flush()?;
    events.shutdown().await?;
    Ok(())
}

/// Render a level meter bar to stderr.
fn render_level_meter(level: &AudioLevel, stderr_is_tty: bool) {
    if !stderr_is_tty {
        return;
    }
    const BAR_WIDTH: usize = 40;
    const MIN_DB: f32 = -60.0;
    const MAX_DB: f32 = 0.0;

    // Map RMS dB to bar position
    let normalized = ((level.rms_db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0);
    let filled = (normalized * BAR_WIDTH as f32) as usize;

    // Map peak dB to bar position
    let peak_normalized = ((level.peak_db - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0);
    let peak_pos = (peak_normalized * BAR_WIDTH as f32) as usize;

    // Build bar string
    let mut bar = String::with_capacity(BAR_WIDTH);
    for i in 0..BAR_WIDTH {
        if i < filled {
            bar.push('█');
        } else if i == peak_pos && peak_pos > filled {
            bar.push('|');
        } else {
            bar.push('░');
        }
    }

    eprint!("\r\x1b[2KLevel: [{bar}] {:6.1} dB", level.rms_db);
    let _ = std::io::stderr().flush();
}

fn spawn_level_task(
    mut rx: mpsc::Receiver<AudioLevel>,
    stderr_is_tty: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(LEVEL_RENDER_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut latest: Option<AudioLevel> = None;
        let mut dirty = false;

        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    match maybe {
                        Some(level) => {
                            latest = Some(level);
                            dirty = true;
                        }
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    if dirty {
                        if let Some(level) = latest.as_ref() {
                            render_level_meter(level, stderr_is_tty);
                        }
                        dirty = false;
                    }
                }
            }
        }
        // Clear the level meter line on exit.
        clear_status_line(stderr_is_tty);
    })
}

#[derive(Clone, Copy)]
struct ProgressUpdate {
    audio_elapsed: Duration,
    wall_elapsed: Duration,
}

fn spawn_progress_task(
    mut rx: mpsc::Receiver<ProgressUpdate>,
    total_duration: Duration,
    stderr_is_tty: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(PROGRESS_RENDER_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut latest: Option<ProgressUpdate> = None;
        let mut dirty = false;

        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    match maybe {
                        Some(progress) => {
                            latest = Some(progress);
                            dirty = true;
                        }
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    if dirty {
                        if let Some(progress) = latest.as_ref() {
                            render_progress_line(progress, total_duration, stderr_is_tty);
                        }
                        dirty = false;
                    }
                }
            }
        }
        // Clear the progress line on exit.
        clear_status_line(stderr_is_tty);
    })
}

fn render_progress_line(
    progress: &ProgressUpdate,
    total_duration: Duration,
    stderr_is_tty: bool,
) {
    if !stderr_is_tty {
        return;
    }
    let elapsed = progress.audio_elapsed.min(total_duration);
    let elapsed_text = format_duration(elapsed);
    let total_text = format_duration(total_duration);
    let wall_secs = progress.wall_elapsed.as_secs_f64();
    let rtf = if wall_secs > 0.0 {
        elapsed.as_secs_f64() / wall_secs
    } else {
        0.0
    };

    eprint!(
        "\r\x1b[2KProgress: {elapsed_text}/{total_text} RTF {:5.2}",
        rtf
    );
    let _ = std::io::stderr().flush();
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

async fn run_file(
    url: String,
    auth_token: Option<String>,
    query_token: Option<String>,
    file_args: FileArgs,
    buffered_output: bool,
) -> Result<()> {
    ensure_hq_resample_enabled(file_args.hq_resample)?;

    let mut builder = SttClientBuilder::new().url(url);

    if let Some(token) = auth_token {
        builder = builder.auth_token(token);
    }
    if let Some(token) = query_token {
        builder = builder.query_token(token);
    }

    let (pcm, sr_in) =
        kaudio::pcm_decode(&file_args.path).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let rtf = file_args.rtf.filter(|v| v.is_finite() && *v > 0.0);
    let silence_prefix_samples =
        silence_samples_from_ms(file_args.silence_prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
    let audio_samples = if sr_in as usize == OUTPUT_SAMPLE_RATE_HZ {
        pcm.len()
    } else {
        (pcm.len() as u64 * OUTPUT_SAMPLE_RATE_HZ as u64 / sr_in.max(1) as u64)
            .saturating_add(2) as usize
    };
    let total_samples = audio_samples.saturating_add(silence_prefix_samples);
    let total_duration =
        Duration::from_secs_f64(total_samples as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);

    let session = builder.connect().await?;
    let mut events = session.into_event_stream();
    let sender = events.sender();
    let stderr_is_tty = std::io::stderr().is_terminal();
    let show_progress = file_args.progress && stderr_is_tty;
    let stdout_is_tty = std::io::stdout().is_terminal();
    let mut transcript = TranscriptOutput::new(buffered_output || !stdout_is_tty);

    let (progress_tx, progress_rx) = if show_progress {
        let (tx, rx) = mpsc::channel::<ProgressUpdate>(16);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    let progress_task =
        progress_rx.map(|rx| spawn_progress_task(rx, total_duration, stderr_is_tty));

    if file_args.silence_prefix_ms > 0 {
        eprintln!(
            "Sending {}ms of silence before streaming...",
            file_args.silence_prefix_ms
        );
    }

    let marker_id: i64 = 1;
    let silence_prefix_ms = file_args.silence_prefix_ms;
    let hq_resample = file_args.hq_resample;
    let progress_tx_for_send = progress_tx.clone();
    let send_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
        let chunk_duration =
            Duration::from_secs_f64(OUTPUT_CHUNK_SAMPLES as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);
        let start = Instant::now();
        let mut chunk_idx: u64 = 0;
        let mut samples_sent: usize = 0;
        let progress_tx = progress_tx_for_send;

        let mut resampler =
            FileResampler::new(sr_in as u32, OUTPUT_SAMPLE_RATE_HZ as u32, hq_resample)?;
        let mut resample_buf = Vec::<f32>::new();
        let mut pending = Vec::<f32>::new();
        let mut pending_read_idx = 0usize;

        if silence_prefix_ms > 0 {
            let silence_samples = silence_samples_from_ms(silence_prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
            let silence_chunks = (silence_samples + OUTPUT_CHUNK_SAMPLES - 1) / OUTPUT_CHUNK_SAMPLES;
            let silence_chunk = vec![0.0f32; OUTPUT_CHUNK_SAMPLES];

            for idx in 0..silence_chunks {
                sender.send(InMsg::Audio { pcm: silence_chunk.clone() }).await?;

                let remaining = silence_samples.saturating_sub(idx * OUTPUT_CHUNK_SAMPLES);
                let sent = remaining.min(OUTPUT_CHUNK_SAMPLES);
                samples_sent += sent;
                if let Some(tx) = &progress_tx {
                    let audio_elapsed = Duration::from_secs_f64(
                        samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64,
                    );
                    let _ = tx.try_send(ProgressUpdate {
                        audio_elapsed,
                        wall_elapsed: start.elapsed(),
                    });
                }

                chunk_idx += 1;
                if let Some(rtf) = rtf {
                    let target = start + chunk_duration.mul_f64(chunk_idx as f64 / rtf);
                    sleep_until(target).await;
                }
            }
        }

        for input_chunk in pcm.chunks(FILE_INPUT_CHUNK_SAMPLES) {
            let samples = match resampler.as_mut() {
                Some(resampler) => {
                    resampler.process_into(input_chunk, &mut resample_buf)?;
                    resample_buf.as_slice()
                }
                None => input_chunk,
            };

            if samples.is_empty() {
                continue;
            }

            pending.extend_from_slice(samples);

            while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
                let start_idx = pending_read_idx;
                let end_idx = pending_read_idx + OUTPUT_CHUNK_SAMPLES;
                let chunk = pending[start_idx..end_idx].to_vec();
                pending_read_idx = end_idx;

                sender.send(InMsg::Audio { pcm: chunk }).await?;

                chunk_idx += 1;
                samples_sent += OUTPUT_CHUNK_SAMPLES;
                if let Some(tx) = &progress_tx {
                    let audio_elapsed = Duration::from_secs_f64(
                        samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64,
                    );
                    let _ = tx.try_send(ProgressUpdate {
                        audio_elapsed,
                        wall_elapsed: start.elapsed(),
                    });
                }
                if let Some(rtf) = rtf {
                    let target = start + chunk_duration.mul_f64(chunk_idx as f64 / rtf);
                    sleep_until(target).await;
                }
            }

            if pending_read_idx > 0 && pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 {
                pending.drain(..pending_read_idx);
                pending_read_idx = 0;
            }
        }

        if let Some(resampler) = resampler.as_mut() {
            resampler.flush_into(&mut resample_buf)?;
            if !resample_buf.is_empty() {
                pending.extend_from_slice(&resample_buf);
            }
        }

        while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
            let start_idx = pending_read_idx;
            let end_idx = pending_read_idx + OUTPUT_CHUNK_SAMPLES;
            let chunk = pending[start_idx..end_idx].to_vec();
            pending_read_idx = end_idx;

            sender.send(InMsg::Audio { pcm: chunk }).await?;

            chunk_idx += 1;
            samples_sent += OUTPUT_CHUNK_SAMPLES;
            if let Some(tx) = &progress_tx {
                let audio_elapsed = Duration::from_secs_f64(
                    samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64,
                );
                let _ = tx.try_send(ProgressUpdate {
                    audio_elapsed,
                    wall_elapsed: start.elapsed(),
                });
            }
            if let Some(rtf) = rtf {
                let target = start + chunk_duration.mul_f64(chunk_idx as f64 / rtf);
                sleep_until(target).await;
            }
        }

        let remainder_len = pending.len().saturating_sub(pending_read_idx);
        if remainder_len > 0 {
            let mut tail = vec![0.0; OUTPUT_CHUNK_SAMPLES];
            tail[..remainder_len]
                .copy_from_slice(&pending[pending_read_idx..pending_read_idx + remainder_len]);
            sender.send(InMsg::Audio { pcm: tail }).await?;

            chunk_idx += 1;
            samples_sent += remainder_len;
            if let Some(tx) = &progress_tx {
                let audio_elapsed =
                    Duration::from_secs_f64(samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);
                let _ = tx.try_send(ProgressUpdate {
                    audio_elapsed,
                    wall_elapsed: start.elapsed(),
                });
            }
            if let Some(rtf) = rtf {
                let target = start + chunk_duration.mul_f64(chunk_idx as f64 / rtf);
                sleep_until(target).await;
            }
        }

        sender.send(InMsg::Marker { id: marker_id }).await?;
        Ok(())
    });

    let stream_start = Instant::now();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            ev = events.recv() => {
                match ev? {
                    SttEvent::WordReceived { text, .. } => {
                        transcript.write_word(&text)?;
                    }
                    SttEvent::StreamMarker { id } if id == marker_id => {
                        break;
                    }
                    SttEvent::Error { message } => {
                        transcript.flush()?;
                        eprintln!("stt error: {message}");
                    }
                    SttEvent::VadStep { step_idx, prs, buffered_pcm } => {
                        if file_args.verbose {
                            info!(
                                step = step_idx,
                                buffered_samples = buffered_pcm,
                                "VAD step: prs={:?}",
                                prs
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    send_task
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))??;

    drop(progress_tx);
    if let Some(task) = progress_task {
        let _ = task.await;
    }

    if file_args.progress {
        let wall_elapsed = stream_start.elapsed();
        let wall_text = format_duration(wall_elapsed);
        let audio_text = format_duration(total_duration);
        let rtf = if wall_elapsed.as_secs_f64() > 0.0 {
            total_duration.as_secs_f64() / wall_elapsed.as_secs_f64()
        } else {
            0.0
        };
        if show_progress {
            clear_status_line(stderr_is_tty);
        }
        eprintln!("Completed: {audio_text} audio in {wall_text} (RTF {rtf:.2})");
    }

    transcript.flush()?;
    events.shutdown().await?;
    Ok(())
}

fn clear_status_line(stderr_is_tty: bool) {
    if stderr_is_tty {
        eprint!("\r\x1b[2K");
        let _ = std::io::stderr().flush();
    }
}

enum FlushPolicy {
    PerWord,
    Interval(Duration),
}

struct TranscriptOutput {
    writer: std::io::BufWriter<std::io::Stdout>,
    policy: FlushPolicy,
    last_flush: std::time::Instant,
}

impl TranscriptOutput {
    fn new(buffered_output: bool) -> Self {
        let policy = if buffered_output {
            FlushPolicy::Interval(TRANSCRIPT_FLUSH_INTERVAL)
        } else {
            FlushPolicy::PerWord
        };
        Self {
            writer: std::io::BufWriter::new(std::io::stdout()),
            policy,
            last_flush: std::time::Instant::now(),
        }
    }

    fn write_word(&mut self, text: &str) -> std::io::Result<()> {
        write!(self.writer, "{text} ")?;
        self.flush_if_needed(false)
    }

    fn write_timestamped(&mut self, start_ms: u64, text: &str) -> std::io::Result<()> {
        write!(self.writer, "[{start_ms}] {text} ")?;
        self.flush_if_needed(false)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }

    fn flush_if_needed(&mut self, force: bool) -> std::io::Result<()> {
        let should_flush = force
            || match self.policy {
                FlushPolicy::PerWord => true,
                FlushPolicy::Interval(interval) => self.last_flush.elapsed() >= interval,
            };
        if should_flush {
            self.writer.flush()?;
            self.last_flush = std::time::Instant::now();
        }
        Ok(())
    }
}

/// Test microphone input without connecting to the STT server.
async fn run_mic_test(args: MicTestArgs) -> Result<()> {
    eprintln!("Starting microphone test...");
    eprintln!("Press Ctrl+C to stop.");
    if args.duration > 0 {
        eprintln!("Test will run for {} seconds.", args.duration);
    }

    let mut mic = MicCapture::start_default()?;
    let stderr_is_tty = std::io::stderr().is_terminal();
    let show_level = args.show_level && stderr_is_tty;
    eprintln!(
        "Microphone initialized: {}Hz, {} channel(s)",
        mic.sample_rate_hz(),
        mic.channels()
    );

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Level meter channel (if enabled)
    let (level_tx, level_rx) = if show_level {
        let (tx, rx) = mpsc::channel::<AudioLevel>(16);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Audio samples collector (if saving WAV)
    let (samples_tx, samples_rx) = if args.save_wav.is_some() {
        let (tx, rx) = mpsc::channel::<Vec<f32>>(64);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    // Audio capture task
    let audio_task = tokio::spawn({
        let level_tx = level_tx.clone();
        let samples_tx = samples_tx.clone();
        async move {
            let mut meter = LevelMeter::default();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                    chunk = mic.recv() => {
                        let Some(chunk) = chunk else {
                            break;
                        };

                        // Compute and send level if enabled
                        if let Some(tx) = &level_tx {
                            let level = meter.process(&chunk.samples);
                            let _ = tx.try_send(level);
                        }

                        // Send samples for WAV recording if enabled
                        if let Some(tx) = &samples_tx {
                            let _ = tx.try_send(chunk.samples);
                        }
                    }
                }
            }
        }
    });

    // Level display task (if enabled)
    let level_task = level_rx.map(|rx| spawn_level_task(rx, stderr_is_tty));

    // WAV recorder task (if enabled)
    let wav_path = args.save_wav.clone();
    let wav_task = samples_rx.map(|mut rx| {
        tokio::spawn(async move {
            let mut all_samples: Vec<f32> = Vec::new();
            while let Some(samples) = rx.recv().await {
                all_samples.extend(samples);
            }
            all_samples
        })
    });

    // Calculate end time if duration specified
    let end_time = if args.duration > 0 {
        Some(Instant::now() + Duration::from_secs(args.duration))
    } else {
        None
    };

    // Wait for either Ctrl+C or duration timeout
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nStopping...");
        }
        _ = async {
            if let Some(end) = end_time {
                sleep_until(end).await;
            } else {
                // Sleep forever if no duration
                std::future::pending::<()>().await;
            }
        } => {
            eprintln!("\nDuration complete.");
        }
    }

    // Shutdown
    let _ = shutdown_tx.send(true);
    drop(level_tx);
    drop(samples_tx);

    audio_task.await.map_err(|e| anyhow::anyhow!(e.to_string()))?;

    if let Some(task) = level_task {
        let _ = task.await;
    }

    // Save WAV file if requested
    if let (Some(path), Some(task)) = (wav_path, wav_task) {
        let samples = task.await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if samples.is_empty() {
            eprintln!("No audio samples recorded.");
        } else {
            eprintln!("Saving {} samples to {}...", samples.len(), path.display());
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            write_wav(&path, &samples, OUTPUT_SAMPLE_RATE_HZ as u32)?;
            eprintln!("Saved to {}", path.display());
        }
    }

    eprintln!("Microphone test complete.");
    Ok(())
}

/// Write f32 samples to a WAV file (mono, 24kHz, 16-bit PCM).
fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    use std::fs::File;
    use std::io::{BufWriter, Write as IoWrite};

    let num_samples = samples.len() as u32;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = num_samples * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
    let file_size = 36 + data_size;

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // RIFF header
    writer.write_all(b"RIFF")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    // fmt chunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // chunk size
    writer.write_all(&1u16.to_le_bytes())?; // audio format (PCM)
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;

    // data chunk
    writer.write_all(b"data")?;
    writer.write_all(&data_size.to_le_bytes())?;

    // Convert f32 samples to i16 and write
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        writer.write_all(&i16_sample.to_le_bytes())?;
    }

    writer.flush()?;
    Ok(())
}
