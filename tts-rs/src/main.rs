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
use serde::Serialize;
use std::io::{BufRead, Write};
use tokio_tungstenite::tungstenite::Message;

const SAMPLE_RATE: u32 = 24000;

/// TTS streaming client for moshi-server
#[derive(Debug, Parser)]
#[command(name = "kyutai-tts-rs")]
#[command(about = "Rust TTS client for moshi-server streaming API")]
struct Args {
    /// Input text file path, or "-" for stdin
    input: String,

    /// Output WAV file path
    output: String,

    /// WebSocket server URL
    #[arg(long, default_value = "ws://127.0.0.1:8080")]
    url: String,

    /// Voice to use (relative to voice repo root)
    #[arg(long, default_value = "expresso/ex03-ex01_happy_001_channel1_334s.wav")]
    voice: String,

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
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(file_name).to_string_lossy().to_string(),
        _ => file_name,
    }
}

async fn run_tts_once(args: &Args, text: &str, run_idx: usize) -> Result<(BenchResult, Option<Vec<f32>>)> {
    let ws_url = build_ws_url(args)?;
    if !args.json {
        println!("Connecting to: {}", redact_ws_url(&ws_url));
    }

    let start = std::time::Instant::now();

    let (ws_stream, _response) = tokio_tungstenite::connect_async(ws_url.to_string())
        .await
        .context("Failed to connect to WebSocket")?;

    let (mut write, mut read) = ws_stream.split();

    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<f64>();
    let (ttfb_tx, ttfb_rx) = tokio::sync::oneshot::channel::<f64>();

    let mut ready_tx = Some(ready_tx);
    let mut ttfb_tx = Some(ttfb_tx);

    let receive_task = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    match rmp_serde::from_slice::<InMsg>(&data) {
                        Ok(InMsg::Ready) => {
                            if let Some(tx) = ready_tx.take() {
                                let _ = tx.send(start.elapsed().as_secs_f64() * 1000.0);
                            }
                        }
                        Ok(InMsg::Audio { pcm }) => {
                            if let Some(tx) = ttfb_tx.take() {
                                let _ = tx.send(start.elapsed().as_secs_f64() * 1000.0);
                            }
                            if audio_tx.send(pcm).is_err() {
                                break;
                            }
                        }
                        Ok(InMsg::Text { .. }) => {}
                        Ok(InMsg::Error { message }) => {
                            return Err(anyhow::anyhow!("Server error: {message}"));
                        }
                        Ok(InMsg::OggOpus { .. }) => {}
                        Err(e) => {
                            return Err(anyhow::anyhow!("Failed to decode message: {e}"));
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(_) => {}
                Err(e) => return Err(anyhow::anyhow!("WebSocket error: {e}")),
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    write.send(Message::Text(text.into())).await?;
    write.send(Message::Binary(vec![0u8].into())).await?;
    drop(write);

    let mut all_samples = Vec::new();
    while let Some(samples) = audio_rx.recv().await {
        all_samples.extend(samples);
    }

    match receive_task.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(e) => return Err(anyhow::anyhow!("Receive task join error: {e}")),
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let audio_seconds = all_samples.len() as f64 / SAMPLE_RATE as f64;

    let tt_ready_ms = ready_rx.ok();
    let ttfb_ms = ttfb_rx.ok();
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
        ok: !all_samples.is_empty(),
        error: if all_samples.is_empty() {
            Some("no_audio".to_string())
        } else {
            None
        },
        tt_ready_ms,
        ttfb_ms,
        total_ms: Some(total_ms),
        audio_samples: all_samples.len(),
        audio_seconds,
        wall_seconds,
        rtf,
        x_real_time,
    };

    Ok((result, Some(all_samples)))
}

/// Incoming message types (received from server)
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum InMsg {
    Audio { pcm: Vec<f32> },
    Text { text: String, start_s: f64, stop_s: f64 },
    OggOpus { data: Vec<u8> },
    Error { message: String },
    Ready,
}

fn build_ws_url(args: &Args) -> Result<url::Url> {
    let mut url = url::Url::parse(&args.url)?;
    
    // Ensure path ends with /api/tts_streaming
    let path = url.path();
    if !path.ends_with("/api/tts_streaming") {
        url.set_path(&format!("{}/api/tts_streaming", path.trim_end_matches('/')));
    }

    // Add query parameters
    url.query_pairs_mut()
        .append_pair("voice", &args.voice)
        .append_pair("format", "PcmMessagePack")
        .append_pair("seed", &args.seed.to_string())
        .append_pair("temperature", &args.temperature.to_string())
        .append_pair("top_k", &args.top_k.to_string());

    if let Some(token) = &args.token {
        url.query_pairs_mut().append_pair("token", token);
    }

    Ok(url)
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
        std::io::BufReader::new(file).lines().collect::<std::io::Result<Vec<_>>>()?
    };
    Ok(lines)
}

fn write_wav(output: &str, samples: &[f32]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(output, spec)
        .with_context(|| format!("Failed to create WAV file: {}", output))?;

    for &sample in samples {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;
    Ok(())
}

async fn run_tts_client(args: Args) -> Result<()> {
    let lines = read_input_lines(&args.input)?;
    let text = lines.join(" ");
    if text.trim().is_empty() {
        anyhow::bail!("No text to synthesize");
    }

    for run_idx in 1..=args.runs {
        let out_path = output_path_for_run(&args.output, run_idx, args.runs);
        let (result, samples) = match run_tts_once(&args, &text, run_idx).await {
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
                return Err(anyhow::anyhow!(res.error.unwrap_or_else(|| "unknown".into())));
            }
        };

        if args.json {
            println!("{}", serde_json::to_string(&result)?);
        } else {
            println!("run {run_idx}/{}: ttfb_ms={:?} total_ms={:?} audio_s={:.2} rtf={:?}", args.runs, result.ttfb_ms, result.total_ms, result.audio_seconds, result.rtf);
        }

        if let Some(samples) = samples {
            if out_path != "-" {
                write_wav(&out_path, &samples)?;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    run_tts_client(args).await
}
