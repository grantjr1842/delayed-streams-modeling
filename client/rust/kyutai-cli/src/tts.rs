use anyhow::{Result, Context};
use clap::{Args, ValueEnum};
use kyutai_client::tts::{TtsClientBuilder, InMsg};
use kyutai_client_core::auth;
use kyutai_client_core::audio::{
    AudioPlayer, DynResampler, ResampleQuality,
};
use ringbuf::traits::*;
use std::io::BufRead;
use std::sync::atomic::Ordering;
use std::time::{Duration as StdDuration, Instant};
use serde::Serialize;

const SAMPLE_RATE: u32 = 24000;

#[derive(Args, Debug)]
pub struct TtsArgs {
    /// WebSocket URL for the TTS server
    #[arg(long, default_value = "ws://localhost:8080/api/tts-streaming")]
    pub url: String,

    /// Bearer token for authentication
    #[arg(long)]
    pub token: Option<String>,

    /// Text to synthesize (if not provided, interactive mode)
    #[arg(long, short = 'i')]
    pub input: Option<String>,

    /// Output WAV file path
    #[arg(long, short = 'o')]
    pub output: Option<String>,

    /// Interactive mode
    #[arg(long)]
    pub interactive: bool,

    /// Number of benchmarks to run
    #[arg(long, default_value = "1")]
    pub runs: usize,

    /// Audio playback backend
    #[arg(long, default_value = "cpal")]
    pub play_backend: PlayBackend,

    /// Prebuffer duration in ms for cpal
    #[arg(long, default_value = "200")]
    pub prebuffer_ms: u32,

    /// Max buffer duration in ms for cpal
    #[arg(long, default_value = "2000")]
    pub max_buffer_ms: u32,

    /// CPAL specific: preferred sample rate
    #[arg(long)]
    pub cpal_sample_rate_hz: Option<u32>,

    /// CPAL specific: preferred buffer size
    #[arg(long)]
    pub cpal_buffer_frames: Option<u32>,

    /// PulseAudio specific: latency in ms
    #[arg(long, default_value = "100")]
    pub pulse_latency_ms: u32,

    /// PulseAudio specific: process time in ms
    #[arg(long, default_value = "20")]
    pub pulse_process_time_ms: u32,

    /// Output benchmarking results as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum PlayBackend {
    Cpal,
    Pulse,
}

#[derive(Debug, Serialize)]
struct BenchResult {
    run_idx: usize,
    ok: bool,
    error: Option<String>,
    tt_ready_ms: Option<f64>,
    ttfb_ms: Option<f64>,
    total_ms: Option<f64>,
    audio_samples: usize,
    audio_seconds: f64,
    wall_seconds: Option<f64>,
    rtf: Option<f64>,
    x_real_time: Option<f64>,
}

pub async fn run_tts(mut args: TtsArgs) -> Result<()> {
    ensure_token(&mut args)?;

    if args.interactive || (args.input.is_none() && args.output.is_none()) {
        run_tts_interactive_mode(args).await
    } else if let (Some(input), Some(output)) = (&args.input, &args.output) {
        run_tts_file_mode(&args, input, output).await
    } else {
        anyhow::bail!("Non-interactive mode requires both --input and --output");
    }
}

fn ensure_token(args: &mut TtsArgs) -> Result<()> {
    let resolver = auth::AuthResolver::new("kyutai-cli/0.1.0")
        .with_token(args.token.as_deref());
    if let Some(token) = resolver.resolve(true)? {
        args.token = Some(token);
    }
    Ok(())
}

async fn run_tts_once(
    args: &TtsArgs,
    text: &str,
    run_idx: usize,
    output: Option<&str>,
    play_audio: bool,
) -> Result<BenchResult> {
    let start = Instant::now();
    let mut builder = TtsClientBuilder::new(&args.url);
    if let Some(token) = &args.token {
        builder = builder.auth_token(token);
    }

    let mut session = builder.connect().await?;
    session.send_text(text).await?;

    let mut cpal_player = if play_audio && matches!(args.play_backend, PlayBackend::Cpal) {
        AudioPlayer::setup(args.prebuffer_ms, args.max_buffer_ms, args.cpal_sample_rate_hz, args.cpal_buffer_frames, !args.json).ok()
    } else { None };

    let mut resampler = match cpal_player.as_ref() {
        Some(p) => DynResampler::new(SAMPLE_RATE, p.output_sample_rate as u32, ResampleQuality::High)?,
        None => None,
    };

    let mut audio_samples = 0;
    let mut tt_ready_ms = None;
    let mut ttfb_ms = None;
    let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;

    while let Some(msg) = session.recv().await? {
        match msg {
            InMsg::Ready => { if tt_ready_ms.is_none() { tt_ready_ms = Some(start.elapsed().as_secs_f64() * 1000.0); } }
            InMsg::Audio { pcm } => {
                if ttfb_ms.is_none() { ttfb_ms = Some(start.elapsed().as_secs_f64() * 1000.0); }
                audio_samples += pcm.len();

                if let (Some(p), Some(r)) = (cpal_player.as_mut(), resampler.as_mut()) {
                    let mut out = Vec::new();
                    r.process_into(&pcm, &mut out)?;
                    let mut pos = 0;
                    while pos < out.len() {
                        let pushed = p.producer.push_slice(&out[pos..]);
                        if pushed == 0 { tokio::time::sleep(StdDuration::from_millis(5)).await; continue; }
                        p.queued_samples.fetch_add(pushed, Ordering::AcqRel);
                        pos += pushed;
                    }
                }

                if let Some(out_path) = output {
                    if writer.is_none() {
                        let f = std::fs::File::create(out_path)?;
                        writer = Some(hound::WavWriter::new(std::io::BufWriter::new(f), hound::WavSpec { channels: 1, sample_rate: SAMPLE_RATE, bits_per_sample: 32, sample_format: hound::SampleFormat::Float })?);
                    }
                    if let Some(w) = writer.as_mut() { for s in pcm { w.write_sample(s)?; } }
                }
            }
            InMsg::Error { message } => return Err(anyhow::anyhow!("Server error: {message}")),
            _ => {}
        }
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let audio_seconds = audio_samples as f64 / SAMPLE_RATE as f64;

    Ok(BenchResult {
        run_idx, ok: audio_samples > 0, error: None, tt_ready_ms, ttfb_ms, total_ms: Some(total_ms),
        audio_samples, audio_seconds, wall_seconds: Some(total_ms / 1000.0),
        rtf: None, x_real_time: None,
    })
}

async fn run_tts_file_mode(args: &TtsArgs, input: &str, output: &str) -> Result<()> {
    let text = std::fs::read_to_string(input).context("Failed to read input file")?;
    let res = run_tts_once(args, &text, 0, Some(output), false).await?;
    if args.json { println!("{}", serde_json::to_string(&res)?); }
    else { println!("TTS completed: {} samples", res.audio_samples); }
    Ok(())
}

async fn run_tts_interactive_mode(args: TtsArgs) -> Result<()> {
    println!("TTS Interactive Mode. Type text and press Enter. (Ctrl+D to exit)");
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        if line.trim().is_empty() { continue; }
        if let Err(e) = run_tts_once(&args, &line, 0, None, true).await {
            eprintln!("Error: {e}");
        }
    }
    Ok(())
}
