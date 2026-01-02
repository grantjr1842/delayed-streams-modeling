use anyhow::{Result, Context};
use clap::{Args, Subcommand};
use kyutai_client::stt::audio::{
    AudioLevel, LevelMeter, MicCapture, MicCaptureConfig, ResampleQuality,
};
use kyutai_client::stt::protocol::InMsg;
use kyutai_client::stt::{SttClientBuilder, SttEvent};
use kyutai_client_core::auth;
use kyutai_client_core::audio::{DynResampler as FileResampler};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, interval, sleep_until};
use tracing::info;

const OUTPUT_SAMPLE_RATE_HZ: usize = 24_000;
const OUTPUT_CHUNK_SAMPLES: usize = 1920;
const FILE_INPUT_CHUNK_SAMPLES: usize = 4096;
const LEVEL_RENDER_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_RENDER_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Args, Debug)]
pub struct SttArgs {
    /// WebSocket URL for the STT server
    #[arg(long, default_value = "ws://localhost:8080/api/asr-streaming")]
    pub url: String,

    /// Bearer token for authentication
    #[arg(long)]
    pub auth_token: Option<String>,

    /// Query parameter token for authentication
    #[arg(long)]
    pub query_token: Option<String>,

    /// BETTER_AUTH_SECRET for generating JWT tokens
    #[arg(long, env = "BETTER_AUTH_SECRET")]
    pub secret: Option<String>,

    /// Environment name for loading .env.<env> when generating tokens
    #[arg(long, env = "ENV")]
    pub env: Option<String>,

    /// Buffer transcript output and flush periodically (reduces overhead)
    #[arg(long)]
    pub buffered_output: bool,

    #[command(subcommand)]
    pub command: SttCommand,
}

#[derive(Subcommand, Debug)]
pub enum SttCommand {
    /// Stream audio from microphone
    Mic(MicArgs),
    /// Stream audio from file
    File(FileArgs),
    /// Generate a JWT token
    Token(TokenArgs),
}

#[derive(Args, Debug)]
pub struct MicArgs {
    /// Show timestamps with each word
    #[arg(long)]
    pub timestamps: bool,

    /// Show real-time input level meter
    #[arg(long)]
    pub show_level: bool,

    /// Auto-generate a token using --secret/BETTER_AUTH_SECRET
    #[arg(long)]
    pub auto_token: bool,

    /// Enable verbose logging of model performance parameters (VAD steps)
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Prepend silence before streaming (milliseconds, rounded to 80ms chunks)
    #[arg(long, default_value = "0")]
    pub silence_prefix_ms: u64,

    /// Use high-quality resampling
    #[arg(long)]
    pub hq_resample: bool,
}

#[derive(Args, Debug)]
pub struct FileArgs {
    pub path: PathBuf,

    #[arg(long)]
    pub rtf: Option<f64>,

    /// Show file streaming progress/status line
    #[arg(long)]
    pub progress: bool,

    /// Auto-generate a token using --secret/BETTER_AUTH_SECRET
    #[arg(long)]
    pub auto_token: bool,

    /// Enable verbose logging of model performance parameters (VAD steps)
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Prepend silence before streaming (milliseconds, rounded to 80ms chunks)
    #[arg(long, default_value = "0")]
    pub silence_prefix_ms: u64,

    /// Use high-quality resampling
    #[arg(long)]
    pub hq_resample: bool,
}

#[derive(Args, Debug)]
pub struct TokenArgs {
    /// Token validity in hours
    #[arg(long, default_value = "1.0")]
    pub hours: f64,
}

pub async fn run_stt(args: SttArgs) -> Result<()> {
    match args.command {
        SttCommand::Mic(mic_args) => {
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
        SttCommand::File(file_args) => {
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
        SttCommand::Token(token_args) => run_token(&args.secret, args.env.as_deref(), token_args)?,
    }
    Ok(())
}

fn resolve_auth_token(
    auth_token: &Option<String>,
    secret: &Option<String>,
    env_name: Option<&str>,
    auto_token: bool,
) -> Result<Option<String>> {
    let resolver = auth::AuthResolver::new("kyutai-cli/0.1.0")
        .with_token(auth_token.as_deref())
        .with_secret(secret.as_deref())
        .with_env(env_name);

    let token = resolver.resolve(auto_token)?;
    if auto_token && token.is_some() {
        eprintln!("Generated auth token (valid for 1 hour)");
    }
    Ok(token)
}

fn run_token(secret: &Option<String>, env_name: Option<&str>, args: TokenArgs) -> Result<()> {
    let resolver = auth::AuthResolver::new("kyutai-cli/0.1.0")
        .with_secret(secret.as_deref())
        .with_env(env_name);

    let base_dir = std::env::current_dir()?;
    let secret_val = auth::resolve_secret(resolver.secret, &base_dir, resolver.env_name)?;
    let token = auth::generate_token(&secret_val, args.hours, resolver.user_agent)
        .map_err(|e| anyhow::anyhow!("Failed to generate token: {}", e))?;

    println!("{token}");
    Ok(())
}

async fn run_mic(
    url: String,
    auth_token: Option<String>,
    query_token: Option<String>,
    mic_args: MicArgs,
    buffered_output: bool,
) -> Result<()> {
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

    let (level_tx, level_rx) = if show_level {
        let (tx, rx) = mpsc::channel::<AudioLevel>(16);
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    let audio_task = tokio::spawn({
        let level_tx = level_tx.clone();
        async move {
            let mut meter = LevelMeter::default();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    chunk = mic.recv() => {
                        let Some(chunk) = chunk else { break; };
                        if let Some(tx) = &level_tx {
                            let level = meter.process(&chunk.samples);
                            let _ = tx.try_send(level);
                        }
                        sender.send(InMsg::Audio { pcm: chunk.samples }).await?;
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        }
    });

    let level_task = level_rx.map(|rx| spawn_level_task(rx, stderr_is_tty));

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            ev = events.recv() => {
                let ev = ev?;
                match ev {
                    SttEvent::WordReceived { text, start_ms } => {
                        if show_level { clear_status_line(stderr_is_tty); }
                        if mic_args.timestamps {
                            transcript.write_timestamped(start_ms, &text)?;
                        } else {
                            transcript.write_word(&text)?;
                        }
                    }
                    SttEvent::Error { message } => {
                        if show_level { clear_status_line(stderr_is_tty); }
                        transcript.flush()?;
                        eprintln!("stt error: {message}");
                    }
                    SttEvent::VadStep { step_idx, prs, buffered_pcm } => {
                        if mic_args.verbose {
                            info!(step = step_idx, buffered_samples = buffered_pcm, "VAD step: prs={:?}", prs);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);
    drop(level_tx);
    let _ = audio_task.await;
    if let Some(task) = level_task { let _ = task.await; }
    transcript.flush()?;
    events.shutdown().await?;
    Ok(())
}

async fn run_file(
    url: String,
    auth_token: Option<String>,
    query_token: Option<String>,
    file_args: FileArgs,
    buffered_output: bool,
) -> Result<()> {
    let mut builder = SttClientBuilder::new().url(url);
    if let Some(token) = auth_token { builder = builder.auth_token(token); }
    if let Some(token) = query_token { builder = builder.query_token(token); }

    let (pcm, sr_in) = kaudio::pcm_decode(&file_args.path).context("Failed to decode audio file")?;
    let rtf = file_args.rtf.filter(|v| v.is_finite() && *v > 0.0);
    let silence_prefix_samples = silence_samples_from_ms(file_args.silence_prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
    let audio_samples = if sr_in as usize == OUTPUT_SAMPLE_RATE_HZ { pcm.len() } else {
        (pcm.len() as u64 * OUTPUT_SAMPLE_RATE_HZ as u64 / sr_in.max(1) as u64).saturating_add(2) as usize
    };
    let total_samples = audio_samples.saturating_add(silence_prefix_samples);
    let total_duration = Duration::from_secs_f64(total_samples as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);

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
    } else { (None, None) };

    let progress_task = progress_rx.map(|rx| spawn_progress_task(rx, total_duration, stderr_is_tty));

    let marker_id: i64 = 1;
    let silence_prefix_ms = file_args.silence_prefix_ms;
    let progress_tx_for_send = progress_tx.clone();
    let send_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn(async move {
        let chunk_duration = Duration::from_secs_f64(OUTPUT_CHUNK_SAMPLES as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);
        let start = Instant::now();
        let mut chunk_idx: u64 = 0;
        let mut samples_sent: usize = 0;
        let quality = if file_args.hq_resample { ResampleQuality::High } else { ResampleQuality::Linear };
        let mut resampler = FileResampler::new(sr_in, OUTPUT_SAMPLE_RATE_HZ as u32, quality)?;
        let mut resample_buf = Vec::<f32>::with_capacity(FILE_INPUT_CHUNK_SAMPLES);
        let mut pending = Vec::<f32>::with_capacity(OUTPUT_CHUNK_SAMPLES * 4);
        let mut pending_read_idx = 0usize;

        if silence_prefix_ms > 0 {
            let silence_samples = silence_samples_from_ms(silence_prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
            let silence_chunks = silence_samples.div_ceil(OUTPUT_CHUNK_SAMPLES);
            let silence_chunk = vec![0.0f32; OUTPUT_CHUNK_SAMPLES];
            for idx in 0..silence_chunks {
                sender.send(InMsg::Audio { pcm: silence_chunk.clone() }).await?;
                samples_sent += silence_samples.saturating_sub(idx * OUTPUT_CHUNK_SAMPLES).min(OUTPUT_CHUNK_SAMPLES);
                if let Some(tx) = &progress_tx_for_send {
                    let _ = tx.try_send(ProgressUpdate { audio_elapsed: Duration::from_secs_f64(samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64), wall_elapsed: start.elapsed() });
                }
                chunk_idx += 1;
                if let Some(rtf) = rtf { sleep_until(start + chunk_duration.mul_f64(chunk_idx as f64 / rtf)).await; }
            }
        }

        for input_chunk in pcm.chunks(FILE_INPUT_CHUNK_SAMPLES) {
            let samples = match resampler.as_mut() {
                Some(r) => { r.process_into(input_chunk, &mut resample_buf)?; resample_buf.as_slice() }
                None => input_chunk,
            };
            if samples.is_empty() { continue; }
            pending.extend_from_slice(samples);
            while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
                let chunk = pending[pending_read_idx..pending_read_idx + OUTPUT_CHUNK_SAMPLES].to_vec();
                pending_read_idx += OUTPUT_CHUNK_SAMPLES;
                sender.send(InMsg::Audio { pcm: chunk }).await?;
                chunk_idx += 1;
                samples_sent += OUTPUT_CHUNK_SAMPLES;
                if let Some(tx) = &progress_tx_for_send {
                    let _ = tx.try_send(ProgressUpdate { audio_elapsed: Duration::from_secs_f64(samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64), wall_elapsed: start.elapsed() });
                }
                if let Some(rtf) = rtf { sleep_until(start + chunk_duration.mul_f64(chunk_idx as f64 / rtf)).await; }
            }
            if pending_read_idx >= OUTPUT_CHUNK_SAMPLES * 4 { pending.drain(..pending_read_idx); pending_read_idx = 0; }
        }

        if let Some(r) = resampler.as_mut() {
            r.flush(&mut resample_buf)?;
            pending.extend_from_slice(&resample_buf);
        }
        while pending.len().saturating_sub(pending_read_idx) >= OUTPUT_CHUNK_SAMPLES {
            let chunk = pending[pending_read_idx..pending_read_idx + OUTPUT_CHUNK_SAMPLES].to_vec();
            pending_read_idx += OUTPUT_CHUNK_SAMPLES;
            sender.send(InMsg::Audio { pcm: chunk }).await?;
            chunk_idx += 1;
            samples_sent += OUTPUT_CHUNK_SAMPLES;
            if let Some(tx) = &progress_tx_for_send {
                let _ = tx.try_send(ProgressUpdate { audio_elapsed: Duration::from_secs_f64(samples_sent as f64 / OUTPUT_SAMPLE_RATE_HZ as f64), wall_elapsed: start.elapsed() });
            }
            if let Some(rtf) = rtf { sleep_until(start + chunk_duration.mul_f64(chunk_idx as f64 / rtf)).await; }
        }
        let rem = pending.len().saturating_sub(pending_read_idx);
        if rem > 0 {
            let mut tail = vec![0.0; OUTPUT_CHUNK_SAMPLES];
            tail[..rem].copy_from_slice(&pending[pending_read_idx..pending_read_idx + rem]);
            sender.send(InMsg::Audio { pcm: tail }).await?;
        }
        sender.send(InMsg::Marker { id: marker_id }).await?;
        Ok(())
    });

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            ev = events.recv() => {
                match ev? {
                    SttEvent::WordReceived { text, .. } => { transcript.write_word(&text)?; }
                    SttEvent::StreamMarker { id } if id == marker_id => break,
                    SttEvent::Error { message } => { transcript.flush()?; eprintln!("stt error: {message}"); }
                    _ => {}
                }
            }
        }
    }
    transcript.flush()?;
    events.shutdown().await?;
    let _ = send_task.await;
    if let Some(task) = progress_task { let _ = task.await; }
    Ok(())
}

fn silence_samples_from_ms(prefix_ms: u64, sample_rate_hz: usize) -> usize {
    (prefix_ms as u128 * sample_rate_hz as u128).div_ceil(1000) as usize
}

async fn send_silence_prefix(sender: &kyutai_client::stt::SttSender, prefix_ms: u64) -> Result<()> {
    let total_samples = silence_samples_from_ms(prefix_ms, OUTPUT_SAMPLE_RATE_HZ);
    if total_samples == 0 { return Ok(()); }
    let chunk_duration = Duration::from_secs_f64(OUTPUT_CHUNK_SAMPLES as f64 / OUTPUT_SAMPLE_RATE_HZ as f64);
    let chunk_count = total_samples.div_ceil(OUTPUT_CHUNK_SAMPLES);
    let silence_chunk = vec![0.0f32; OUTPUT_CHUNK_SAMPLES];
    for idx in 0..chunk_count {
        sender.send(InMsg::Audio { pcm: silence_chunk.clone() }).await?;
        if idx + 1 < chunk_count { tokio::time::sleep(chunk_duration).await; }
    }
    Ok(())
}

fn clear_status_line(stderr_is_tty: bool) { if stderr_is_tty { eprint!("\r\x1b[2K"); let _ = std::io::stderr().flush(); } }

fn render_level_meter(level: &AudioLevel, stderr_is_tty: bool) {
    if !stderr_is_tty { return; }
    const BAR_WIDTH: usize = 40;
    let normalized = ((level.rms_db + 60.0) / 60.0).clamp(0.0, 1.0);
    let filled = (normalized * BAR_WIDTH as f32) as usize;
    let peak_pos = (((level.peak_db + 60.0) / 60.0).clamp(0.0, 1.0) * BAR_WIDTH as f32) as usize;
    let mut bar = String::with_capacity(BAR_WIDTH);
    for i in 0..BAR_WIDTH {
        if i < filled { bar.push('█'); }
        else if i == peak_pos && peak_pos > filled { bar.push('|'); }
        else { bar.push('░'); }
    }
    eprint!("\r\x1b[2KLevel: [{bar}] {:6.1} dB", level.rms_db);
    let _ = std::io::stderr().flush();
}

fn spawn_level_task(mut rx: mpsc::Receiver<AudioLevel>, stderr_is_tty: bool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(LEVEL_RENDER_INTERVAL);
        let mut latest: Option<AudioLevel> = None;
        loop {
            tokio::select! {
                maybe = rx.recv() => { if let Some(l) = maybe { latest = Some(l); } else { break; } }
                _ = ticker.tick() => { if let Some(l) = latest.as_ref() { render_level_meter(l, stderr_is_tty); } }
            }
        }
        clear_status_line(stderr_is_tty);
    })
}

#[derive(Clone, Copy)]
struct ProgressUpdate { audio_elapsed: Duration, wall_elapsed: Duration }

fn spawn_progress_task(mut rx: mpsc::Receiver<ProgressUpdate>, total: Duration, stderr_is_tty: bool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(PROGRESS_RENDER_INTERVAL);
        let mut latest: Option<ProgressUpdate> = None;
        loop {
            tokio::select! {
                maybe = rx.recv() => { if let Some(p) = maybe { latest = Some(p); } else { break; } }
                _ = ticker.tick() => { if let Some(p) = latest.as_ref() {
                    let elapsed = p.audio_elapsed.min(total);
                    let rtf = if p.wall_elapsed.as_secs_f64() > 0.0 { elapsed.as_secs_f64() / p.wall_elapsed.as_secs_f64() } else { 0.0 };
                    eprint!("\r\x1b[2KProgress: {}/{} RTF {:5.2}", format_duration(elapsed), format_duration(total), rtf);
                    let _ = std::io::stderr().flush();
                } }
            }
        }
        clear_status_line(stderr_is_tty);
    })
}

fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

struct TranscriptOutput { buffered: bool, buffer: String, last_flush: Instant }
impl TranscriptOutput {
    fn new(buffered: bool) -> Self { Self { buffered, buffer: String::new(), last_flush: Instant::now() } }
    fn write_word(&mut self, text: &str) -> Result<()> {
        if self.buffered { self.buffer.push_str(text); if self.last_flush.elapsed() > Duration::from_millis(200) { self.flush()?; } }
        else { print!("{text}"); let _ = std::io::stdout().flush(); }
        Ok(())
    }
    fn write_timestamped(&mut self, ms: u64, text: &str) -> Result<()> {
        self.flush()?;
        println!("[{}] {text}", format_duration(Duration::from_millis(ms)));
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        if !self.buffer.is_empty() { print!("{}", self.buffer); let _ = std::io::stdout().flush(); self.buffer.clear(); }
        self.last_flush = Instant::now();
        Ok(())
    }
}
