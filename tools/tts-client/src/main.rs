use anyhow::{Context, Result};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use chrono::{Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input file, use - for stdin.
    inp: String,

    /// Output file to generate, use - for playing the audio.
    out: String,

    /// The voice to use, relative to the voice repo root.
    #[arg(long, default_value = "expresso/ex03-ex01_happy_001_channel1_334s.wav")]
    voice: String,

    /// The URL of the server.
    #[arg(long, default_value = "ws://127.0.0.1:8080")]
    url: String,

    /// Better Auth JWT token for authentication.
    #[arg(long)]
    token: Option<String>,
}

const DEFAULT_TOKEN_HOURS: f64 = 1.0;

#[derive(Serialize, Deserialize, Debug)]
struct AudioMessage {
    r#type: String,
    pcm: Vec<f32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Construct URL
    let mut url = Url::parse(&cli.url)?;
    url.set_path("/api/tts_streaming");
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("voice", &cli.voice);
        pairs.append_pair("format", "PcmMessagePack");
        if let Some(token) = resolve_token(cli.token, "kyutai-tts-rs/0.1.0") {
            pairs.append_pair("token", &token);
        }
    }

    println!("Connecting to {}", url);
    let (ws_stream, _) = connect_async(url.to_string()).await?;
    println!("Connected");

    let (mut write, mut read) = ws_stream.split();

    // Audio handling setup
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<f32>>();
    let play_audio = cli.out == "-";
    
    let _audio_handle = std::thread::spawn(move || {
        if play_audio {
            let host = cpal::default_host();
            let device = host.default_output_device().expect("no output device available");
            let config = device.default_output_config().unwrap();

            let config: cpal::StreamConfig = config.into();

            let audio_buffer = Arc::new(Mutex::new(Vec::new()));
            let playback_buffer = audio_buffer.clone();
            
            let channels = config.channels as usize;
            
            let stream = device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                     let mut buffer = playback_buffer.lock().unwrap();
                     for frame in data.chunks_mut(channels) {
                         if !buffer.is_empty() {
                             let sample = buffer.remove(0);
                             for sample_out in frame.iter_mut() {
                                 *sample_out = sample;
                             }
                         } else {
                             for sample_out in frame.iter_mut() {
                                 *sample_out = 0.0;
                             }
                         }
                     }
                },
                move |err| eprintln!("an error occurred on stream: {}", err),
                None, // None=blocking, some default time
            ).unwrap();

            stream.play().unwrap();

            while let Some(pcm) = audio_rx.blocking_recv() {
                let mut buffer = audio_buffer.lock().unwrap();
                buffer.extend_from_slice(&pcm);
            }
        } else {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 24000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut writer = hound::WavWriter::create(&cli.out, spec).expect("failed to create wav file");
            
            while let Some(pcm) = audio_rx.blocking_recv() {
                for sample in pcm {
                    writer.write_sample(sample).unwrap();
                }
            }
            writer.finalize().unwrap();
            println!("Saved audio to {}", cli.out);
        }
    });

    // Receive loop
    let recv_handle = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Binary(bin)) => {
                    // Try msgpack unpack
                    if let Ok(val) = rmp_serde::from_slice::<AudioMessage>(&bin) {
                        if val.r#type == "Audio" {
                             let _ = audio_tx.send(val.pcm);
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // Send loop
    if cli.inp == "-" {
        println!("Enter text to synthesize (Ctrl+D to end input):");
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            for word in line.split_whitespace() {
                 write.send(Message::Text(word.to_string().into())).await?;
            }
            line.clear();
        }
    } else {
        let file = File::open(&cli.inp)?;
        let reader = std::io::BufReader::new(file);
        for line in std::io::BufRead::lines(reader) {
            let line = line?;
            for word in line.split_whitespace() {
                 write.send(Message::Text(word.to_string().into())).await?;
            }
        }
    }
    
    // Send EOF
    write.send(Message::Binary(vec![0].into())).await?;
    
    recv_handle.await?;
    // audio_handle might assume stream ends when channel closes (which happens when audio_tx drops).
    // audio_tx drops when recv_handle finishes.
    
    // However, recv_handle finishes when server closes connection?
    // tts-server closes connection after stream? 
    // The script sends b"\0" and then waits for messages until close.
    
    Ok(())
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
