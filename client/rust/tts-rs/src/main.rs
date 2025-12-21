// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

//! Rust TTS client for moshi-server WebSocket streaming API.
//!
//! This client connects to the moshi-server TTS streaming endpoint,
//! sends text words, and receives PCM audio which is saved to a WAV file.

use anyhow::{Context, Result};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use kyutai_client_core::{auth, ws};
use ringbuf::{HeapRb, traits::*};
use serde::Serialize;
use std::io::BufRead;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration as StdDuration;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

const SAMPLE_RATE: u32 = 24000;

fn default_output_dir() -> String {
    format!("{}/../tmp/tts", env!("CARGO_MANIFEST_DIR"))
}

struct AudioPlayer {
    _stream: cpal::Stream,
    producer: ringbuf::HeapProd<f32>,
    queued_samples: Arc<AtomicUsize>,
    started: Arc<AtomicBool>,
    output_sample_rate: usize,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum PlayBackend {
    Cpal,
    Pulse,
}

struct PulsePlayer {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    scratch: Vec<u8>,
}

impl PulsePlayer {
    async fn start(
        sample_rate_hz: u32,
        latency_ms: Option<u32>,
        process_time_ms: Option<u32>,
    ) -> Result<Self> {
        let mut args = vec![
            "--raw".to_string(),
            format!("--rate={sample_rate_hz}"),
            "--channels=1".to_string(),
            "--format=float32le".to_string(),
        ];
        if let Some(ms) = latency_ms {
            if ms > 0 {
                args.push(format!("--latency-msec={ms}"));
            }
        }
        if let Some(ms) = process_time_ms {
            if ms > 0 {
                args.push(format!("--process-time-msec={ms}"));
            }
        }

        let mut child = Command::new("pacat")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn pacat")?;

        let stdin = child.stdin.take().context("Failed to open pacat stdin")?;

        Ok(Self {
            child,
            stdin,
            scratch: Vec::new(),
        })
    }

    async fn write_f32(&mut self, pcm: &[f32]) -> Result<()> {
        if cfg!(target_endian = "little") {
            let bytes =
                unsafe { std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 4) };
            self.stdin.write_all(bytes).await?;
        } else {
            self.scratch.clear();
            self.scratch.reserve(pcm.len() * 4);
            for s in pcm {
                self.scratch.extend_from_slice(&s.to_le_bytes());
            }
            self.stdin.write_all(&self.scratch).await?;
        }
        Ok(())
    }

    async fn finish(mut self, timeout: StdDuration) {
        let _ = self.stdin.shutdown().await;
        drop(self.stdin);
        let _ = tokio::time::timeout(timeout, self.child.wait()).await;
    }
}

async fn run_pulse_writer(
    mut rx: mpsc::Receiver<Vec<f32>>,
    mut prebuf: Vec<f32>,
    sample_rate_hz: u32,
    latency_ms: u32,
    process_time_ms: u32,
    finish_timeout: StdDuration,
    json: bool,
) {
    let mut pp =
        match PulsePlayer::start(sample_rate_hz, Some(latency_ms), Some(process_time_ms)).await {
            Ok(v) => v,
            Err(e) => {
                if !json {
                    eprintln!("Warning: failed to start pacat: {e}");
                }
                return;
            }
        };

    let chunk_ms = if process_time_ms > 0 {
        process_time_ms
    } else {
        40
    };
    let chunk_samples = ((sample_rate_hz as u64 * chunk_ms as u64) / 1000) as usize;

    let mut pending: Vec<f32> = Vec::new();
    let mut pending_pos: usize = 0;

    pending.append(&mut prebuf);

    loop {
        while pending.len().saturating_sub(pending_pos) >= chunk_samples && chunk_samples > 0 {
            let end = pending_pos + chunk_samples;
            if let Err(e) = pp.write_f32(&pending[pending_pos..end]).await {
                if !json {
                    eprintln!("Warning: failed to write to pacat: {e}");
                }
                return;
            }
            pending_pos = end;
        }

        if pending_pos > 0 && pending_pos >= pending.len() / 2 {
            pending.drain(..pending_pos);
            pending_pos = 0;
        }

        let Some(chunk) = rx.recv().await else {
            break;
        };
        pending.extend_from_slice(&chunk);
    }

    if pending_pos < pending.len() {
        let _ = pp.write_f32(&pending[pending_pos..]).await;
    }
    pp.finish(finish_timeout).await;
}

struct ResamplerState {
    resampler: rubato::FastFixedIn<f32>,
    output_buffer: Vec<f32>,
    input_buffer: Vec<f32>,
    input_len: usize,
}

impl ResamplerState {
    fn new(input_sample_rate: usize, output_sample_rate: usize) -> Result<Self> {
        use rubato::Resampler;

        let resample_ratio = output_sample_rate as f64 / input_sample_rate as f64;
        let resampler = rubato::FastFixedIn::new(
            resample_ratio,
            f64::max(resample_ratio, 1.0),
            rubato::PolynomialDegree::Septic,
            1024,
            1,
        )?;
        let input_buffer = resampler.input_buffer_allocate(true).remove(0);
        let output_buffer = resampler.output_buffer_allocate(true).remove(0);
        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            input_len: 0,
        })
    }

    fn push_input_buffer(&mut self, samples: &[f32]) {
        self.input_buffer[self.input_len..self.input_len + samples.len()].copy_from_slice(samples);
        self.input_len += samples.len();
    }

    fn push_samples(&mut self, samples: &[f32], out: &mut Vec<f32>) -> Result<()> {
        use rubato::Resampler;

        let mut pos_in = 0;
        loop {
            let rem = self.input_buffer.len() - self.input_len;
            let pos_end = usize::min(pos_in + rem, samples.len());
            self.push_input_buffer(&samples[pos_in..pos_end]);
            pos_in = pos_end;
            if self.input_len < self.input_buffer.len() {
                break;
            }
            let (_, out_len) = self.resampler.process_into_buffer(
                &[&self.input_buffer],
                &mut [&mut self.output_buffer],
                None,
            )?;
            out.extend_from_slice(&self.output_buffer[..out_len]);
            self.input_len = 0;
        }
        Ok(())
    }

    fn flush(&mut self, out: &mut Vec<f32>) -> Result<()> {
        let rem = self.input_buffer.len().saturating_sub(self.input_len);
        if rem == 0 {
            return Ok(());
        }
        let pad = vec![0.0f32; rem];
        self.push_samples(&pad, out)
    }
}

fn setup_output_stream(
    prebuffer_ms: u32,
    max_buffer_ms: u32,
    cpal_sample_rate_hz: Option<u32>,
    cpal_buffer_frames: Option<u32>,
    json: bool,
) -> Result<AudioPlayer> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no output device available")?;

    let mut supported_configs_range = device.supported_output_configs()?;
    let config_range = match supported_configs_range.find(|c| c.channels() == 1) {
        None => device
            .supported_output_configs()?
            .next()
            .context("no audio output available")?,
        Some(config_range) => config_range,
    };

    let default_sr = device
        .default_output_config()
        .ok()
        .map(|cfg| cfg.sample_rate().0);
    let desired_sr = cpal_sample_rate_hz.or(default_sr).unwrap_or(SAMPLE_RATE);

    let sample_rate = cpal::SampleRate(desired_sr).clamp(
        config_range.min_sample_rate(),
        config_range.max_sample_rate(),
    );
    let mut config: cpal::StreamConfig = config_range.with_sample_rate(sample_rate).into();
    if let Some(frames) = cpal_buffer_frames {
        if frames > 0 {
            config.buffer_size = cpal::BufferSize::Fixed(frames);
        }
    }
    let channels = config.channels as usize;

    let output_sample_rate = config.sample_rate.0 as usize;

    let min_buffer_samples = ((output_sample_rate as u64 * prebuffer_ms as u64) / 1000) as usize;
    let max_buffer_samples = ((output_sample_rate as u64 * max_buffer_ms as u64) / 1000) as usize;
    let min_buffer_samples = usize::max(min_buffer_samples, output_sample_rate / 20);
    let max_buffer_samples = usize::max(max_buffer_samples, min_buffer_samples.saturating_mul(2));

    let rb = HeapRb::<f32>::new(max_buffer_samples);
    let (producer, mut consumer) = rb.split();
    let queued_samples = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicBool::new(false));
    let qs = queued_samples.clone();
    let started_cb = started.clone();
    let mut last_elem_state = 0.0f32;

    if !json {
        let device_name = device.name().unwrap_or_else(|_| "unk".to_string());
        println!(
            "cpal device: {device_name} sample_rate={} channels={} buffer={:?}",
            config.sample_rate.0, config.channels, config.buffer_size
        );
    }

    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            data.fill(0.);

            if !started_cb.load(Ordering::Acquire) {
                if qs.load(Ordering::Acquire) < min_buffer_samples {
                    return;
                }
                started_cb.store(true, Ordering::Release);
            }

            let mut last_elem = last_elem_state;
            let mut popped = 0usize;
            for (idx, elem) in data.iter_mut().enumerate() {
                if idx % channels == 0 {
                    let v_opt = consumer.try_pop();
                    match v_opt {
                        None => {
                            break;
                        }
                        Some(v) => {
                            last_elem = v;
                            popped = popped.saturating_add(1);
                            *elem = v;
                        }
                    }
                } else {
                    *elem = last_elem
                }
            }

            if popped > 0 {
                let _ = qs.fetch_update(Ordering::AcqRel, Ordering::Acquire, |v0| {
                    Some(v0.saturating_sub(popped))
                });
            }
            last_elem_state = last_elem;
        },
        move |err| eprintln!("cpal error: {err}"),
        None,
    )?;
    stream.play()?;

    Ok(AudioPlayer {
        _stream: stream,
        producer,
        queued_samples,
        started,
        output_sample_rate,
    })
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn ensure_token(args: &mut Args) -> Result<()> {
    if args.token.is_some() {
        return Ok(());
    }

    if let Ok(token) = std::env::var("MOSHI_JWT_TOKEN") {
        if !token.trim().is_empty() {
            args.token = Some(token);
            return Ok(());
        }
    }

    let secret = match std::env::var("BETTER_AUTH_SECRET") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Ok(()),
    };

    args.token = Some(auth::generate_dev_jwt(&secret, 12)?);
    Ok(())
}

/// TTS streaming client for moshi-server
#[derive(Debug, Parser)]
#[command(name = "kyutai-tts-rs")]
#[command(about = "Rust TTS client for moshi-server streaming API")]
struct Args {
    /// WebSocket server URL
    #[arg(long, default_value = "ws://127.0.0.1:8080")]
    url: String,

    /// Voice to use (relative to voice repo root)
    #[arg(long, default_value = "expresso/ex03-ex01_happy_001_channel1_334s.wav")]
    voice: String,

    /// Interactive mode output directory for generated WAV files
    #[arg(long, default_value_t = default_output_dir())]
    output_dir: String,

    /// Non-interactive mode: input text file path, or "-" for stdin
    #[arg(long)]
    input: Option<String>,

    /// Non-interactive mode: output WAV file path
    #[arg(long)]
    output: Option<String>,

    /// JWT token for Better Auth authentication
    #[arg(long)]
    token: Option<String>,

    /// Number of benchmark runs to execute
    #[arg(long, default_value_t = 1)]
    runs: usize,

    /// Print one JSON object per run
    #[arg(long)]
    json: bool,

    /// Seed for reproducible generation
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Temperature for sampling (0.0 = deterministic)
    #[arg(long, default_value = "0.8")]
    temperature: f64,

    /// Top-k for sampling
    #[arg(long, default_value = "250")]
    top_k: usize,

    #[arg(long, default_missing_value = "true", num_args = 0..=1)]
    play: Option<bool>,

    #[arg(long, value_enum, default_value_t = PlayBackend::Cpal)]
    play_backend: PlayBackend,

    #[arg(long, default_value_t = 800)]
    prebuffer_ms: u32,

    #[arg(long, default_value_t = 4000)]
    max_buffer_ms: u32,

    #[arg(long, default_value_t = 200)]
    pulse_latency_ms: u32,

    #[arg(long, default_value_t = 0)]
    pulse_process_time_ms: u32,

    #[arg(long, default_value_t = 24000)]
    pulse_sample_rate_hz: u32,

    #[arg(long, default_value_t = false)]
    pulse_buffered: bool,

    #[arg(long)]
    cpal_sample_rate_hz: Option<u32>,

    #[arg(long)]
    cpal_buffer_frames: Option<u32>,
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

fn output_path_for_run(base: &str, run_idx: usize, runs: usize) -> String {
    if runs <= 1 {
        return base.to_string();
    }
    if base == "-" {
        return base.to_string();
    }
    let path = std::path::Path::new(base);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(base);
    let ext = path.extension().and_then(|s| s.to_str());
    let parent = path.parent();
    let file_name = match ext {
        Some(ext) => format!("{stem}_run{run_idx}.{ext}"),
        None => format!("{stem}_run{run_idx}"),
    };
    match parent {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.join(file_name).to_string_lossy().to_string()
        }
        _ => file_name,
    }
}

async fn run_tts_once(
    args: &Args,
    text: &str,
    run_idx: usize,
    output: Option<&str>,
    play_audio: bool,
) -> Result<BenchResult> {
    let ws_url = build_ws_url(args)?;
    if !args.json {
        println!("Connecting to: {}", redact_ws_url(&ws_url));
    }

    let start = Instant::now();

    let (ws_stream, _response) = tokio_tungstenite::connect_async(ws_url.to_string())
        .await
        .context("Failed to connect to WebSocket")?;

    let (mut write, mut read) = ws_stream.split();

    let mut cpal_player = if play_audio && matches!(args.play_backend, PlayBackend::Cpal) {
        match setup_output_stream(
            args.prebuffer_ms,
            args.max_buffer_ms,
            args.cpal_sample_rate_hz,
            args.cpal_buffer_frames,
            args.json,
        ) {
            Ok(v) => Some(v),
            Err(e) => {
                if !args.json {
                    eprintln!("Warning: failed to setup audio output: {e}");
                }
                None
            }
        }
    } else {
        None
    };

    let pulse_enabled = play_audio && matches!(args.play_backend, PlayBackend::Pulse);
    let pulse_buffered = args.pulse_buffered;
    let pulse_sample_rate_hz = args.pulse_sample_rate_hz;
    let wav_enabled = output.map(|p| p != "-").unwrap_or(false);
    let cpal_enabled = cpal_player.is_some();

    let mut pulse_tx: Option<mpsc::Sender<Vec<f32>>> = None;
    let mut pulse_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut pulse_buf: Vec<f32> = Vec::new();
    let pulse_prebuffer_samples =
        ((pulse_sample_rate_hz as u64 * args.prebuffer_ms as u64) / 1000) as usize;

    let mut pulse_resampler = if pulse_enabled && pulse_sample_rate_hz != SAMPLE_RATE {
        Some(ResamplerState::new(
            SAMPLE_RATE as usize,
            pulse_sample_rate_hz as usize,
        )?)
    } else {
        None
    };

    let mut resampler = match cpal_player.as_ref() {
        Some(p) => Some(ResamplerState::new(
            SAMPLE_RATE as usize,
            p.output_sample_rate,
        )?),
        None => None,
    };

    let mut tt_ready_ms: Option<f64> = None;
    let mut ttfb_ms: Option<f64> = None;
    let mut audio_samples: usize = 0;

    let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
    let wav_spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    write.send(Message::Text(text.into())).await?;
    write.send(Message::Binary(vec![0u8].into())).await?;
    drop(write);

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Binary(data)) => match rmp_serde::from_slice::<InMsg>(&data) {
                Ok(InMsg::Ready) => {
                    if tt_ready_ms.is_none() {
                        tt_ready_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                    }
                }
                Ok(InMsg::Audio { pcm }) => {
                    if ttfb_ms.is_none() {
                        ttfb_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                    }

                    let pulse_pcm: Option<Vec<f32>> = if pulse_enabled {
                        if let Some(r) = pulse_resampler.as_mut() {
                            let mut out = Vec::with_capacity(
                                (pcm.len() as f64 * pulse_sample_rate_hz as f64
                                    / SAMPLE_RATE as f64) as usize
                                    + 1024,
                            );
                            r.push_samples(&pcm, &mut out)?;
                            Some(out)
                        } else {
                            Some(pcm.clone())
                        }
                    } else {
                        None
                    };

                    if pulse_enabled {
                        if pulse_buffered {
                            if let Some(ppcm) = pulse_pcm.as_ref() {
                                pulse_buf.extend_from_slice(ppcm);
                            }
                        } else {
                            if let Some(tx) = pulse_tx.as_ref() {
                                if cpal_enabled || wav_enabled {
                                    if let Some(ppcm) = pulse_pcm {
                                        let _ = tx.send(ppcm).await;
                                    }
                                } else {
                                    let pcm_len = pcm.len();
                                    if let Some(ppcm) = pulse_pcm {
                                        let _ = tx.send(ppcm).await;
                                    }
                                    audio_samples = audio_samples.saturating_add(pcm_len);
                                    continue;
                                }
                            } else {
                                if let Some(ppcm) = pulse_pcm.as_ref() {
                                    pulse_buf.extend_from_slice(ppcm);
                                }
                                if pulse_buf.len() >= pulse_prebuffer_samples {
                                    let finish_timeout = StdDuration::from_secs(15 * 60);

                                    let (tx, rx) = mpsc::channel::<Vec<f32>>(512);
                                    let prebuf = std::mem::take(&mut pulse_buf);
                                    let sample_rate_hz = pulse_sample_rate_hz;
                                    let latency_ms = args.pulse_latency_ms;
                                    let process_time_ms = args.pulse_process_time_ms;
                                    let json = args.json;
                                    pulse_handle = Some(tokio::spawn(async move {
                                        run_pulse_writer(
                                            rx,
                                            prebuf,
                                            sample_rate_hz,
                                            latency_ms,
                                            process_time_ms,
                                            finish_timeout,
                                            json,
                                        )
                                        .await;
                                    }));
                                    pulse_tx = Some(tx);
                                }
                            }
                        }
                    }

                    if let (Some(p), Some(r)) = (cpal_player.as_mut(), resampler.as_mut()) {
                        let mut out = Vec::with_capacity(
                            (pcm.len() as f64 * p.output_sample_rate as f64 / SAMPLE_RATE as f64)
                                as usize
                                + 1024,
                        );
                        r.push_samples(&pcm, &mut out)?;
                        if !out.is_empty() {
                            let mut pos = 0usize;
                            while pos < out.len() {
                                if p.producer.vacant_len() == 0 {
                                    tokio::time::sleep(StdDuration::from_millis(5)).await;
                                    continue;
                                }
                                let pushed = p.producer.push_slice(&out[pos..]);
                                if pushed == 0 {
                                    tokio::time::sleep(StdDuration::from_millis(5)).await;
                                    continue;
                                }
                                p.queued_samples.fetch_add(pushed, Ordering::AcqRel);
                                pos += pushed;
                            }
                        }
                    }

                    if let Some(out_path) = output {
                        if out_path != "-" {
                            if writer.is_none() {
                                if let Some(parent) = Path::new(out_path).parent() {
                                    if !parent.as_os_str().is_empty() {
                                        std::fs::create_dir_all(parent).with_context(|| {
                                            format!(
                                                "Failed to create output directory: {}",
                                                parent.display()
                                            )
                                        })?;
                                    }
                                }
                                let f = std::fs::File::create(out_path).with_context(|| {
                                    format!("Failed to create WAV file: {out_path}")
                                })?;
                                let buf = std::io::BufWriter::new(f);
                                writer = Some(hound::WavWriter::new(buf, wav_spec)?);
                            }
                            if let Some(w) = writer.as_mut() {
                                for sample in pcm.iter().copied() {
                                    w.write_sample(sample)?;
                                }
                            }
                        }
                    }

                    audio_samples = audio_samples.saturating_add(pcm.len());
                }
                Ok(InMsg::Text {
                    text,
                    start_s,
                    stop_s,
                }) => {
                    let _ = (text, start_s, stop_s);
                }
                Ok(InMsg::Error { message }) => {
                    return Err(anyhow::anyhow!("Server error: {message}"));
                }
                Ok(InMsg::OggOpus { data }) => {
                    let _ = data;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to decode message: {e}"));
                }
            },
            Ok(Message::Close(frame)) => {
                if let Some(frame) = frame {
                    let code_u16: u16 = match frame.code {
                        CloseCode::Normal => 1000,
                        CloseCode::Away => 1001,
                        CloseCode::Protocol => 1002,
                        CloseCode::Unsupported => 1003,
                        CloseCode::Status => 1005,
                        CloseCode::Abnormal => 1006,
                        CloseCode::Invalid => 1007,
                        CloseCode::Policy => 1008,
                        CloseCode::Size => 1009,
                        CloseCode::Extension => 1010,
                        CloseCode::Error => 1011,
                        CloseCode::Restart => 1012,
                        CloseCode::Again => 1013,
                        CloseCode::Tls => 1015,
                        CloseCode::Reserved(v)
                        | CloseCode::Iana(v)
                        | CloseCode::Library(v)
                        | CloseCode::Bad(v) => v,
                    };

                    if code_u16 == 4001 {
                        return Err(anyhow::anyhow!(
                            "WebSocket authentication failed (close code 4001). Provide --token <jwt> (or connect with a session cookie / Authorization Bearer token)."
                        ));
                    }
                }
                break;
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Ok(_) => {}
            Err(e) => return Err(anyhow::anyhow!("WebSocket error: {e}")),
        }
    }

    if pulse_enabled {
        if let Some(r) = pulse_resampler.as_mut() {
            let mut out = Vec::with_capacity(1024);
            r.flush(&mut out)?;
            if !out.is_empty() {
                if pulse_buffered {
                    pulse_buf.extend_from_slice(&out);
                } else if let Some(tx) = pulse_tx.as_ref() {
                    let _ = tx.send(out).await;
                } else {
                    pulse_buf.extend_from_slice(&out);
                }
            }
        }
    }

    if pulse_enabled {
        if pulse_tx.is_none() && !pulse_buf.is_empty() {
            let finish_timeout = StdDuration::from_secs(15 * 60);

            let (tx, rx) = mpsc::channel::<Vec<f32>>(1);
            drop(tx);
            let prebuf = std::mem::take(&mut pulse_buf);
            let sample_rate_hz = pulse_sample_rate_hz;
            let latency_ms = args.pulse_latency_ms;
            let process_time_ms = args.pulse_process_time_ms;
            let json = args.json;
            pulse_handle = Some(tokio::spawn(async move {
                run_pulse_writer(
                    rx,
                    prebuf,
                    sample_rate_hz,
                    latency_ms,
                    process_time_ms,
                    finish_timeout,
                    json,
                )
                .await;
            }));
        }
    }

    if let (Some(p), Some(r)) = (cpal_player.as_mut(), resampler.as_mut()) {
        let mut out = Vec::with_capacity(1024);
        r.flush(&mut out)?;
        if !out.is_empty() {
            let mut pos = 0usize;
            while pos < out.len() {
                if p.producer.vacant_len() == 0 {
                    tokio::time::sleep(StdDuration::from_millis(5)).await;
                    continue;
                }
                let pushed = p.producer.push_slice(&out[pos..]);
                if pushed == 0 {
                    tokio::time::sleep(StdDuration::from_millis(5)).await;
                    continue;
                }
                p.queued_samples.fetch_add(pushed, Ordering::AcqRel);
                pos += pushed;
            }
        }
    }

    if let Some(w) = writer {
        w.finalize()?;
    }

    if let Some(p) = cpal_player.as_ref() {
        if !p.started.load(Ordering::Acquire) && p.queued_samples.load(Ordering::Acquire) > 0 {
            p.started.store(true, Ordering::Release);
        }

        let audio_seconds = audio_samples as f64 / SAMPLE_RATE as f64;
        let deadline = Instant::now() + StdDuration::from_secs_f64(audio_seconds + 1.0);
        loop {
            let remaining = p.queued_samples.load(Ordering::Acquire);
            if remaining == 0 {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(25)).await;
        }
    }

    if let Some(tx) = pulse_tx {
        drop(tx);
    }
    if let Some(h) = pulse_handle {
        let audio_seconds = audio_samples as f64 / SAMPLE_RATE as f64;
        let pulse_latency_s = args.pulse_latency_ms as f64 / 1000.0;
        let join_s = (audio_seconds + pulse_latency_s + 2.0).clamp(5.0, 15.0 * 60.0);
        let join_timeout = StdDuration::from_secs_f64(join_s);
        let _ = tokio::time::timeout(join_timeout, h).await;
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let audio_seconds = audio_samples as f64 / SAMPLE_RATE as f64;

    let wall_seconds = ttfb_ms.map(|ttfb| (total_ms - ttfb) / 1000.0);
    let (rtf, x_real_time) = match (wall_seconds, audio_seconds > 0.0) {
        (Some(wall_s), true) if wall_s > 0.0 => {
            let rtf = wall_s / audio_seconds;
            let xrt = audio_seconds / wall_s;
            (Some(rtf), Some(xrt))
        }
        _ => (None, None),
    };

    let result = BenchResult {
        run_idx,
        ok: audio_samples > 0,
        error: if audio_samples == 0 {
            Some("no_audio".to_string())
        } else {
            None
        },
        tt_ready_ms,
        ttfb_ms,
        total_ms: Some(total_ms),
        audio_samples,
        audio_seconds,
        wall_seconds,
        rtf,
        x_real_time,
    };

    Ok(result)
}

/// Incoming message types (received from server)
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum InMsg {
    Audio {
        pcm: Vec<f32>,
    },
    Text {
        text: String,
        start_s: f64,
        stop_s: f64,
    },
    OggOpus {
        data: Vec<u8>,
    },
    Error {
        message: String,
    },
    Ready,
}

fn build_ws_url(args: &Args) -> Result<url::Url> {
    let base_url = url::Url::parse(&args.url)?;
    let path = if base_url.path().ends_with("/api/tts_streaming") {
        base_url.path().to_string()
    } else {
        format!("{}/api/tts_streaming", base_url.path().trim_end_matches('/'))
    };

    let seed = args.seed.to_string();
    let temperature = args.temperature.to_string();
    let top_k = args.top_k.to_string();
    let query = [
        ("voice", args.voice.as_str()),
        ("format", "PcmMessagePack"),
        ("seed", seed.as_str()),
        ("temperature", temperature.as_str()),
        ("top_k", top_k.as_str()),
    ];

    ws::build_ws_url(&args.url, &path, &query, args.token.as_deref())
}

fn redact_ws_url(url: &url::Url) -> String {
    let mut url = url.clone();
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    url.set_query(None);
    {
        let mut qp = url.query_pairs_mut();
        for (k, v) in pairs {
            if k == "token" {
                qp.append_pair(&k, "<redacted>");
            } else {
                qp.append_pair(&k, &v);
            }
        }
    }
    url.to_string()
}

fn read_input_lines(input: &str) -> Result<Vec<String>> {
    let lines: Vec<String> = if input == "-" {
        let stdin = std::io::stdin();
        stdin.lock().lines().collect::<std::io::Result<Vec<_>>>()?
    } else {
        let file = std::fs::File::open(input)
            .with_context(|| format!("Failed to open input file: {}", input))?;
        std::io::BufReader::new(file)
            .lines()
            .collect::<std::io::Result<Vec<_>>>()?
    };
    Ok(lines)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn default_output_base(args: &Args, prompt_idx: usize) -> String {
    let ts = now_millis();
    format!("{}/tts_{}_{}.wav", args.output_dir, ts, prompt_idx)
}

async fn run_tts_file_mode(args: &Args, input: &str, output: &str) -> Result<()> {
    let lines = read_input_lines(input)?;
    let text = lines.join(" ");
    if text.trim().is_empty() {
        anyhow::bail!("No text to synthesize");
    }

    for run_idx in 1..=args.runs {
        let out_path = output_path_for_run(output, run_idx, args.runs);
        let play_audio = args.play.unwrap_or(false);
        let result = match run_tts_once(args, &text, run_idx, Some(&out_path), play_audio).await {
            Ok(v) => v,
            Err(err) => {
                let res = BenchResult {
                    run_idx,
                    ok: false,
                    error: Some(err.to_string()),
                    tt_ready_ms: None,
                    ttfb_ms: None,
                    total_ms: None,
                    audio_samples: 0,
                    audio_seconds: 0.0,
                    wall_seconds: None,
                    rtf: None,
                    x_real_time: None,
                };
                if args.json {
                    println!("{}", serde_json::to_string(&res)?);
                    continue;
                }
                return Err(anyhow::anyhow!(
                    res.error.unwrap_or_else(|| "unknown".into())
                ));
            }
        };

        if args.json {
            println!("{}", serde_json::to_string(&result)?);
        } else {
            println!(
                "run {run_idx}/{}: ttfb_ms={:?} total_ms={:?} audio_s={:.2} rtf={:?}",
                args.runs, result.ttfb_ms, result.total_ms, result.audio_seconds, result.rtf
            );
        }

        if !result.ok && !args.json {
            anyhow::bail!("No audio received from server");
        }
    }

    Ok(())
}

async fn run_tts_interactive_mode(args: Args) -> Result<()> {
    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("Failed to create output directory: {}", args.output_dir))?;

    if !args.json {
        println!("Interactive TTS client. Type text and press Enter.");
        println!("Type 'quit' (or 'exit') to stop.");
        if args.token.is_none() {
            println!(
                "Warning: no --token provided. If moshi-server requires auth, it will close the WebSocket with 4001."
            );
        }
    }

    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();
    let mut prompt_idx: usize = 0;

    loop {
        if !args.json {
            print!("tts> ");
            std::io::stdout().flush()?;
        }

        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        let text = line.trim();
        if text.eq_ignore_ascii_case("quit")
            || text.eq_ignore_ascii_case("exit")
            || text.eq_ignore_ascii_case(":q")
            || text.eq_ignore_ascii_case("q")
        {
            break;
        }

        if text.is_empty() {
            continue;
        }

        prompt_idx = prompt_idx.saturating_add(1);
        let output_base = default_output_base(&args, prompt_idx);
        let play_audio = args.play.unwrap_or(true);

        for run_idx in 1..=args.runs {
            let out_path = output_path_for_run(&output_base, run_idx, args.runs);
            let result = match run_tts_once(&args, text, run_idx, Some(&out_path), play_audio).await
            {
                Ok(v) => v,
                Err(err) => {
                    let res = BenchResult {
                        run_idx,
                        ok: false,
                        error: Some(err.to_string()),
                        tt_ready_ms: None,
                        ttfb_ms: None,
                        total_ms: None,
                        audio_samples: 0,
                        audio_seconds: 0.0,
                        wall_seconds: None,
                        rtf: None,
                        x_real_time: None,
                    };
                    if args.json {
                        println!("{}", serde_json::to_string(&res)?);
                        continue;
                    }
                    if !args.json {
                        eprintln!("Error: {err}");
                        break;
                    }
                    break;
                }
            };

            if args.json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!(
                    "run {run_idx}/{}: ttfb_ms={:?} total_ms={:?} audio_s={:.2} rtf={:?}",
                    args.runs, result.ttfb_ms, result.total_ms, result.audio_seconds, result.rtf
                );
            }

            if result.ok {
                if !args.json {
                    println!("Wrote: {out_path}");
                }
            } else if !args.json {
                eprintln!("No audio received from server");
            }
        }
    }

    Ok(())
}

async fn run_tts_client(args: Args) -> Result<()> {
    match (&args.input, &args.output) {
        (Some(input), Some(output)) => run_tts_file_mode(&args, input, output).await,
        (None, None) => run_tts_interactive_mode(args).await,
        (Some(_), None) => anyhow::bail!("Non-interactive mode requires --output"),
        (None, Some(_)) => anyhow::bail!("Non-interactive mode requires --input"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = Args::parse();
    auth::load_better_auth_secret_from_env_files_if_needed(&repo_root())?;
    ensure_token(&mut args)?;
    run_tts_client(args).await
}
