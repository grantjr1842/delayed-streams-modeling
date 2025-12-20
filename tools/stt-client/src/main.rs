use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, I24};
use chrono::{Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

const FRAME_SIZE: usize = 1920;
const TARGET_SAMPLE_RATE: u32 = 24_000;
const TAIL_SILENCE_SECS: f32 = 2.0;
const RMS_INTERVAL_MS: u64 = 500;
const DEFAULT_TOKEN_HOURS: f64 = 1.0;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Mic {
        #[arg(long, default_value = "ws://127.0.0.1:8080")]
        url: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        device: Option<usize>,
    },
    File {
        path: String,
        #[arg(long, default_value = "ws://127.0.0.1:8080")]
        url: String,
        #[arg(long)]
        token: Option<String>,
    }
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum InMsg {
    Audio { pcm: Vec<f32> },
    Marker { id: i64 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum OutMsg {
    Word { text: String, start_time: f64 },
    EndWord { stop_time: f64 },
    Marker { id: i64 },
    Step { step_idx: usize, prs: Vec<f32>, buffered_pcm: usize },
    Error { message: String },
    Ready,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Mic { url, token, device: device_index } => {
            run_mic(url, token, device_index).await
        }
        Commands::File { path, url, token } => {
            run_file(path, url, token).await
        }
    }
}

async fn post_process(msg: Message) -> Option<OutMsg> {
    if let Message::Text(text) = msg {
        println!("Received: {}", text);
        return None;
    } else if let Message::Binary(bin) = msg {
        if let Ok(out_msg) = rmp_serde::from_slice::<OutMsg>(&bin) {
            match &out_msg {
                OutMsg::Word { text, .. } => {
                    print!("{text} ");
                    let _ = io::stdout().flush();
                }
                OutMsg::EndWord { .. } => {}
                OutMsg::Marker { id } => println!("\nMarker {id}"),
                OutMsg::Ready => println!("Ready"),
                OutMsg::Error { message } => eprintln!("Error: {message}"),
                OutMsg::Step { .. } => {}
            }
            return Some(out_msg);
        }
        if let Ok(val) = rmp_serde::from_slice::<serde_json::Value>(&bin) {
            println!("Received: {:?}", val);
        }
    }
    None
}

async fn run_mic(url_str: String, token: Option<String>, device_index: Option<usize>) -> Result<()> {
    let mut url = Url::parse(&url_str)?;
    url.set_path("/api/asr-streaming");
    let token = resolve_token(token, "kyutai-stt-rs/0.1.0");
    if let Some(t) = token {
        url.query_pairs_mut().append_pair("token", &t);
    }

    println!("Connecting to {}", url);
    let (ws_stream, _) = connect_async(url.to_string()).await?;
    println!("Connected");

    let (mut write, mut read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<f32>>();

    let host = cpal::default_host();
    let (device, device_reason) = select_input_device(&host, device_index)?;
    let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());
    println!("Using input device: {} ({})", device_name, device_reason);

    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let channels = config.channels as usize;
    let input_sample_rate = config.sample_rate.0;
    println!(
        "Input format: {} Hz, {} channels, {:?}",
        input_sample_rate, channels, sample_format
    );

    let rms_state = Arc::new(Mutex::new(RmsState::default()));
    let rms_handle = spawn_rms_meter(rms_state.clone());
    let stream = match sample_format {
        SampleFormat::I8 => build_input_stream::<i8>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::I16 => build_input_stream::<i16>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::I24 => build_input_stream::<I24>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::I32 => build_input_stream::<i32>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::I64 => build_input_stream::<i64>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::U8 => build_input_stream::<u8>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::U16 => build_input_stream::<u16>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::U32 => build_input_stream::<u32>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::U64 => build_input_stream::<u64>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::F32 => build_input_stream::<f32>(&device, &config, channels, tx, rms_state)?,
        SampleFormat::F64 => build_input_stream::<f64>(&device, &config, channels, tx, rms_state)?,
        other => anyhow::bail!("Unsupported sample format: {other:?}"),
    };
    stream.play()?;

    let send_handle = tokio::spawn(async move {
        let mut pending = Vec::<f32>::new();
        while let Some(mut pcm) = rx.recv().await {
            if input_sample_rate != TARGET_SAMPLE_RATE {
                pcm = kaudio::resample(
                    &pcm,
                    input_sample_rate as usize,
                    TARGET_SAMPLE_RATE as usize,
                )?;
            }
            if pcm.is_empty() {
                continue;
            }
            pending.extend_from_slice(&pcm);
            while pending.len() >= FRAME_SIZE {
                let rest = pending.split_off(FRAME_SIZE);
                let chunk = std::mem::replace(&mut pending, rest);
                let msg = InMsg::Audio { pcm: chunk };
                let buf = rmp_serde::to_vec_named(&msg)?;
                write.send(Message::Binary(buf.into())).await?;
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    while let Some(msg) = read.next().await {
        // Handle server responses
        if let Ok(m) = msg {
             let _ = post_process(m).await;
        }
    }

    drop(stream);
    rms_handle.abort();
    send_handle.await??;
    Ok(())
}

async fn run_file(path: String, url_str: String, token: Option<String>) -> Result<()> {
    let mut url = Url::parse(&url_str)?;
    url.set_path("/api/asr-streaming");
    let token = resolve_token(token, "kyutai-stt-rs/0.1.0");
    if let Some(t) = token {
        url.query_pairs_mut().append_pair("token", &t);
    }

    println!("Loading audio file from: {}", path);
    let (mut pcm, sample_rate) = kaudio::pcm_decode(&path)?;
    if sample_rate != TARGET_SAMPLE_RATE {
        pcm = kaudio::resample(
            &pcm,
            sample_rate as usize,
            TARGET_SAMPLE_RATE as usize,
        )?;
    }

    println!("Connecting to {}", url);
    let (ws_stream, _) = connect_async(url.to_string()).await?;
    println!("Connected");

    let (mut write, mut read) = ws_stream.split();
    let marker_id = 1i64;
    let (marker_tx, marker_rx) = oneshot::channel::<i64>();
    let recv_handle = tokio::spawn(async move {
        let mut marker_tx = Some(marker_tx);
        while let Some(msg) = read.next().await {
            if let Ok(m) = msg {
                if let Some(out_msg) = post_process(m).await {
                    if let OutMsg::Marker { id } = out_msg {
                        if id == marker_id {
                            if let Some(tx) = marker_tx.take() {
                                let _ = tx.send(id);
                            }
                        }
                    }
                }
            }
        }
    });

    for chunk in pcm.chunks(FRAME_SIZE) {
        if chunk.is_empty() {
            continue;
        }
        let msg = InMsg::Audio { pcm: chunk.to_vec() };
        let buf = rmp_serde::to_vec_named(&msg)?;
        write.send(Message::Binary(buf.into())).await?;
    }

    let msg = InMsg::Marker { id: marker_id };
    let buf = rmp_serde::to_vec_named(&msg)?;
    write.send(Message::Binary(buf.into())).await?;

    let tail_samples = (TARGET_SAMPLE_RATE as f32 * TAIL_SILENCE_SECS).round() as usize;
    if tail_samples > 0 {
        let silence = vec![0.0f32; tail_samples];
        for chunk in silence.chunks(FRAME_SIZE) {
            let msg = InMsg::Audio { pcm: chunk.to_vec() };
            let buf = rmp_serde::to_vec_named(&msg)?;
            write.send(Message::Binary(buf.into())).await?;
        }
    }

    let audio_secs = pcm.len() as f64 / TARGET_SAMPLE_RATE as f64;
    let wait_for = StdDuration::from_secs_f64(audio_secs.max(1.0) * 2.0 + 5.0);
    match timeout(wait_for, marker_rx).await {
        Ok(Ok(_)) => {}
        Ok(Err(_)) => eprintln!("Warning: marker channel closed before completion."),
        Err(_) => eprintln!("Warning: timed out waiting for marker response."),
    }

    write.send(Message::Close(None)).await?;
    recv_handle.await?;
    Ok(())
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::UnboundedSender<Vec<f32>>,
    rms_state: Arc<Mutex<RmsState>>,
) -> Result<cpal::Stream>
where
    T: Sample + SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let err_fn = move |err| eprintln!("cpal error: {err}");
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            if data.is_empty() {
                return;
            }
            let mut pcm = Vec::with_capacity(data.len() / channels.max(1));
            if channels <= 1 {
                pcm.extend(data.iter().map(|sample| sample.to_sample::<f32>()));
            } else {
                for frame in data.chunks(channels) {
                    pcm.push(frame[0].to_sample::<f32>());
                }
            }
            update_rms(&rms_state, &pcm);
            let _ = tx.send(pcm);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn select_input_device(
    host: &cpal::Host,
    device_index: Option<usize>,
) -> Result<(cpal::Device, String)> {
    let devices: Vec<_> = host.input_devices()?.collect();
    if devices.is_empty() {
        anyhow::bail!("no input devices found");
    }

    if let Some(index) = device_index {
        if index >= devices.len() {
            anyhow::bail!(
                "Device index {} out of range ({} devices found)",
                index,
                devices.len()
            );
        }
        return Ok((devices[index].clone(), format!("index {index}")));
    }

    if let Some(device) = host.default_input_device() {
        return Ok((device, "default input device".to_string()));
    }

    if let Some((index, device)) = devices.iter().enumerate().find(|(_, device)| {
        device
            .name()
            .map(|name| {
                let name = name.to_lowercase();
                name.contains("mic") || name.contains("microphone")
            })
            .unwrap_or(false)
    }) {
        return Ok((device.clone(), format!("matched mic name at index {index}")));
    }

    Ok((devices[0].clone(), "first available input device".to_string()))
}

#[derive(Default)]
struct RmsState {
    sum_squares: f64,
    samples: u64,
}

fn update_rms(state: &Arc<Mutex<RmsState>>, samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mut sum = 0.0f64;
    for sample in samples {
        let v = *sample as f64;
        sum += v * v;
    }
    if let Ok(mut guard) = state.lock() {
        guard.sum_squares += sum;
        guard.samples = guard.samples.saturating_add(samples.len() as u64);
    }
}

fn spawn_rms_meter(state: Arc<Mutex<RmsState>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(tokio::time::Duration::from_millis(RMS_INTERVAL_MS));
        loop {
            ticker.tick().await;
            let (sum_squares, samples) = {
                if let Ok(mut guard) = state.lock() {
                    let sum_squares = guard.sum_squares;
                    let samples = guard.samples;
                    guard.sum_squares = 0.0;
                    guard.samples = 0;
                    (sum_squares, samples)
                } else {
                    continue;
                }
            };
            if samples == 0 {
                eprintln!("Mic RMS: (no samples)");
                continue;
            }
            let mean = sum_squares / samples as f64;
            let rms = mean.sqrt();
            let db = 20.0 * (rms + 1e-12).log10();
            eprintln!("Mic RMS: {db:.1} dBFS");
        }
    })
}

#[derive(Serialize)]
struct SessionConfig {
    id: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(rename = "expiresAt")]
    expires_at: String,
    token: String,
    #[serde(rename = "ipAddress")]
    ip_address: String,
    #[serde(rename = "userAgent")]
    user_agent: String,
}

#[derive(Serialize)]
struct UserConfig {
    id: String,
    name: String,
    email: String,
    #[serde(rename = "emailVerified")]
    email_verified: bool,
    image: Option<String>,
}

#[derive(Serialize)]
struct Claims {
    session: SessionConfig,
    user: UserConfig,
    iat: i64,
    exp: i64,
}

fn resolve_token(token: Option<String>, user_agent: &str) -> Option<String> {
    if token.is_some() {
        return token;
    }

    let secret = match load_secret() {
        Ok(secret) => secret,
        Err(err) => {
            eprintln!("Warning: {err}. Proceeding without token.");
            return None;
        }
    };

    match generate_token(&secret, user_agent) {
        Ok(token) => {
            eprintln!("Generated JWT token from BETTER_AUTH_SECRET.");
            Some(token)
        }
        Err(err) => {
            eprintln!("Warning: failed to generate token: {err}. Proceeding without token.");
            None
        }
    }
}

fn load_secret() -> Result<String> {
    if let Ok(secret) = env::var("BETTER_AUTH_SECRET") {
        return Ok(secret);
    }

    if dotenvy::dotenv().is_ok() {
        if let Ok(secret) = env::var("BETTER_AUTH_SECRET") {
            return Ok(secret);
        }
    }

    anyhow::bail!("BETTER_AUTH_SECRET not found in environment or .env file")
}

fn generate_token(secret: &str, user_agent: &str) -> Result<String> {
    let now = Utc::now();
    let exp = now + Duration::seconds((DEFAULT_TOKEN_HOURS * 3600.0) as i64);
    let claims = Claims {
        session: SessionConfig {
            id: "test-session-id".to_string(),
            user_id: "test-user-id".to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            expires_at: exp.to_rfc3339(),
            token: "test-session-token".to_string(),
            ip_address: "127.0.0.1".to_string(),
            user_agent: user_agent.to_string(),
        },
        user: UserConfig {
            id: "test-user-id".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            email_verified: false,
            image: None,
        },
        iat: now.timestamp(),
        exp: exp.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .context("failed to encode JWT")
}
