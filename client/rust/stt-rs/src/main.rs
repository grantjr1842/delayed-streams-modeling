// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use anyhow::{Context, Result};
use candle::{Device, Tensor};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, I24};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

const FRAME_SIZE: usize = 1920;
const TARGET_SAMPLE_RATE: u32 = 24_000;
const RMS_INTERVAL_MS: u64 = 500;
const DEFAULT_TOKEN_HOURS: f64 = 1.0;

#[derive(Debug, Parser)]
#[command(author, version, about = "Kyutai Speech-to-Text client", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Transcribe an audio file locally using the model
    File {
        /// The audio input file, in wav/mp3/ogg/... format.
        in_file: String,

        /// The repo where to get the model from.
        #[arg(long, default_value = "kyutai/stt-1b-en_fr-candle")]
        hf_repo: String,

        /// Path to the model file in the repo.
        #[arg(long, default_value = "model.safetensors")]
        model_path: String,

        /// Run the model on cpu.
        #[arg(long)]
        cpu: bool,

        /// Display word level timestamps.
        #[arg(long)]
        timestamps: bool,

        /// Display the level of voice activity detection (VAD).
        #[arg(long)]
        vad: bool,
    },
    /// Stream microphone audio to a server for real-time transcription
    Mic {
        /// WebSocket URL of the moshi-server
        #[arg(long, default_value = "ws://127.0.0.1:8080")]
        url: String,

        /// Authentication token (optional, auto-generated from BETTER_AUTH_SECRET if not provided)
        #[arg(long)]
        token: Option<String>,

        /// Input device index (use --list-devices to see options)
        #[arg(long)]
        device: Option<usize>,

        /// List available audio input devices
        #[arg(long)]
        list_devices: bool,
    },
}

// ============================================================================
// WebSocket Protocol Messages
// ============================================================================

#[derive(Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
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

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::File {
            in_file,
            hf_repo,
            model_path,
            cpu,
            timestamps,
            vad,
        } => run_file_mode(&in_file, &hf_repo, &model_path, cpu, timestamps, vad),
        Commands::Mic {
            url,
            token,
            device,
            list_devices,
        } => {
            if list_devices {
                list_audio_devices();
                Ok(())
            } else {
                run_mic_mode(&url, token, device).await
            }
        }
    }
}

// ============================================================================
// File Mode (Local Inference)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct SttConfig {
    audio_silence_prefix_seconds: f64,
    audio_delay_seconds: f64,
}

#[derive(Debug, serde::Deserialize)]
struct Config {
    mimi_name: String,
    tokenizer_name: String,
    #[allow(dead_code)]
    card: usize,
    #[allow(dead_code)]
    text_card: usize,
    dim: usize,
    n_q: usize,
    context: usize,
    max_period: f64,
    num_heads: usize,
    num_layers: usize,
    causal: bool,
    stt_config: SttConfig,
}

impl Config {
    fn model_config(&self, vad: bool) -> moshi::lm::Config {
        let lm_cfg = moshi::transformer::Config {
            d_model: self.dim,
            num_heads: self.num_heads,
            num_layers: self.num_layers,
            dim_feedforward: self.dim * 4,
            causal: self.causal,
            norm_first: true,
            bias_ff: false,
            bias_attn: false,
            layer_scale: None,
            context: self.context,
            max_period: self.max_period as usize,
            use_conv_block: false,
            use_conv_bias: true,
            cross_attention: None,
            gating: Some(candle_nn::Activation::Silu),
            norm: moshi::NormType::RmsNorm,
            positional_embedding: moshi::transformer::PositionalEmbedding::Rope,
            conv_layout: false,
            conv_kernel_size: 3,
            kv_repeat: 1,
            max_seq_len: 4096 * 4,
            shared_cross_attn: false,
        };
        let extra_heads = if vad {
            Some(moshi::lm::ExtraHeadsConfig {
                num_heads: 4,
                dim: 6,
            })
        } else {
            None
        };
        moshi::lm::Config {
            transformer: lm_cfg,
            depformer: None,
            audio_vocab_size: self.n_q + 1,
            text_in_vocab_size: 32001,
            text_out_vocab_size: 32000,
            audio_codebooks: self.n_q,
            conditioners: Default::default(),
            extra_heads,
        }
    }
}

struct Model {
    state: moshi::asr::State,
    text_tokenizer: sentencepiece::SentencePieceProcessor,
    timestamps: bool,
    vad: bool,
    config: Config,
    dev: Device,
}

impl Model {
    fn load_from_hf(
        hf_repo: &str,
        model_path: &str,
        vad: bool,
        timestamps: bool,
        dev: &Device,
    ) -> Result<Self> {
        let api = hf_hub::api::sync::Api::new()?;
        let repo = api.model(hf_repo.to_string());
        let config_file = repo.get("config.json")?;
        let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_file)?)?;
        let tokenizer_file = repo.get(&config.tokenizer_name)?;
        let model_file = repo.get(model_path)?;
        let mimi_file = repo.get(&config.mimi_name)?;
        let is_quantized = model_file.to_str().unwrap().ends_with(".gguf");

        let text_tokenizer = sentencepiece::SentencePieceProcessor::open(&tokenizer_file)?;

        let lm = if is_quantized {
            let vb_lm = candle_transformers::quantized_var_builder::VarBuilder::from_gguf(
                &model_file,
                dev,
            )?;
            moshi::lm::LmModel::new(
                &config.model_config(vad),
                moshi::nn::MaybeQuantizedVarBuilder::Quantized(vb_lm),
            )?
        } else {
            let dtype = dev.bf16_default_to_f32();
            let vb_lm = unsafe {
                candle_nn::VarBuilder::from_mmaped_safetensors(&[&model_file], dtype, dev)?
            };
            moshi::lm::LmModel::new(
                &config.model_config(vad),
                moshi::nn::MaybeQuantizedVarBuilder::Real(vb_lm),
            )?
        };

        let audio_tokenizer = moshi::mimi::load(mimi_file.to_str().unwrap(), Some(32), dev)?;
        let asr_delay_in_tokens = (config.stt_config.audio_delay_seconds * 12.5) as usize;
        let state = moshi::asr::State::new(1, asr_delay_in_tokens, 0., audio_tokenizer, lm)?;
        Ok(Model {
            state,
            config,
            text_tokenizer,
            timestamps,
            vad,
            dev: dev.clone(),
        })
    }

    fn run(&mut self, mut pcm: Vec<f32>) -> Result<()> {
        use std::io::Write;

        if self.config.stt_config.audio_silence_prefix_seconds > 0.0 {
            let silence_len =
                (self.config.stt_config.audio_silence_prefix_seconds * 24000.0) as usize;
            pcm.splice(0..0, vec![0.0; silence_len]);
        }
        let suffix = (self.config.stt_config.audio_delay_seconds * 24000.0) as usize;
        pcm.resize(pcm.len() + suffix + 24000, 0.0);

        let mut last_word = None;
        let mut printed_eot = false;
        for pcm in pcm.chunks(1920) {
            let pcm = Tensor::new(pcm, &self.dev)?.reshape((1, 1, ()))?;
            let asr_msgs = self.state.step_pcm(pcm, None, &().into(), |_, _, _| ())?;
            for asr_msg in asr_msgs.iter() {
                match asr_msg {
                    moshi::asr::AsrMsg::Step { prs, .. } => {
                        if self.vad && prs[2][0] > 0.5 && !printed_eot {
                            printed_eot = true;
                            if !self.timestamps {
                                print!(" <endofturn pr={}>", prs[2][0]);
                            } else {
                                println!("<endofturn pr={}>", prs[2][0]);
                            }
                        }
                    }
                    moshi::asr::AsrMsg::EndWord { stop_time, .. } => {
                        printed_eot = false;
                        #[allow(clippy::collapsible_if)]
                        if self.timestamps {
                            if let Some((word, start_time)) = last_word.take() {
                                println!("[{start_time:5.2}-{stop_time:5.2}] {word}");
                            }
                        }
                    }
                    moshi::asr::AsrMsg::Word {
                        tokens, start_time, ..
                    } => {
                        printed_eot = false;
                        let word = self
                            .text_tokenizer
                            .decode_piece_ids(tokens)
                            .unwrap_or_else(|_| String::new());
                        if !self.timestamps {
                            print!(" {word}");
                            std::io::stdout().flush()?
                        } else {
                            if let Some((word, prev_start_time)) = last_word.take() {
                                println!("[{prev_start_time:5.2}-{start_time:5.2}] {word}");
                            }
                            last_word = Some((word, *start_time));
                        }
                    }
                }
            }
        }
        if let Some((word, start_time)) = last_word.take() {
            println!("[{start_time:5.2}-     ] {word}");
        }
        println!();
        Ok(())
    }
}

fn device(cpu: bool) -> Result<Device> {
    if cpu {
        Ok(Device::Cpu)
    } else if candle::utils::cuda_is_available() {
        Ok(Device::new_cuda(0)?)
    } else if candle::utils::metal_is_available() {
        Ok(Device::new_metal(0)?)
    } else {
        Ok(Device::Cpu)
    }
}

fn run_file_mode(
    in_file: &str,
    hf_repo: &str,
    model_path: &str,
    cpu: bool,
    timestamps: bool,
    vad: bool,
) -> Result<()> {
    let device = device(cpu)?;
    println!("Using device: {:?}", device);

    println!("Loading audio file from: {}", in_file);
    let (pcm, sample_rate) = kaudio::pcm_decode(in_file)?;
    let pcm = if sample_rate != 24_000 {
        kaudio::resample(&pcm, sample_rate as usize, 24_000)?
    } else {
        pcm
    };
    println!("Loading model from repository: {}", hf_repo);
    let mut model = Model::load_from_hf(hf_repo, model_path, vad, timestamps, &device)?;
    println!("Running inference");
    model.run(pcm)?;
    Ok(())
}

// ============================================================================
// Microphone Mode (Streaming to Server)
// ============================================================================

fn list_audio_devices() {
    let host = cpal::default_host();
    println!("Available audio input devices:");
    match host.input_devices() {
        Ok(devices) => {
            for (idx, device) in devices.enumerate() {
                let name = device.name().unwrap_or_else(|_| "unknown".to_string());
                let is_default = host
                    .default_input_device()
                    .map(|d| d.name().ok() == device.name().ok())
                    .unwrap_or(false);
                let marker = if is_default { " (default)" } else { "" };
                println!("  {}: {}{}", idx, name, marker);
            }
        }
        Err(e) => eprintln!("Error listing devices: {}", e),
    }
}

async fn run_mic_mode(url_str: &str, token: Option<String>, device_index: Option<usize>) -> Result<()> {
    let mut url = Url::parse(url_str)?;
    url.set_path("/api/asr-streaming");
    let token = resolve_token(token, "kyutai-stt-rs/0.1.0");
    if let Some(t) = &token {
        url.query_pairs_mut().append_pair("token", t);
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

    println!("Streaming... Press Ctrl+C to stop.\n");

    while let Some(msg) = read.next().await {
        if let Ok(m) = msg {
            let _ = post_process(m).await;
        }
    }

    drop(stream);
    rms_handle.abort();
    send_handle.await??;
    Ok(())
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

// ============================================================================
// RMS Meter
// ============================================================================

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

// ============================================================================
// JWT Token Generation
// ============================================================================

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
