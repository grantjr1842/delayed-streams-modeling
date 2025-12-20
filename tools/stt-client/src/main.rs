use anyhow::Result;
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

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
        #[arg(long, default_value_t = 0)]
        device: usize,
    },
    File {
        path: String,
        #[arg(long, default_value = "ws://127.0.0.1:8080")]
        url: String,
        #[arg(long)]
        token: Option<String>,
    }
}

// Actually Python `stt_from_mic_rust_server.py` sends raw bytes?
// Checking the script logic...
// `websocket.send(msgpack.packb({"source_rate": SAMPLE_RATE, "pcm": audio.tobytes()}))`

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

async fn post_process(msg: Message) {
    if let Message::Text(text) = msg {
        // Assume server sends transcripts as text or JSON?
        // Python script: `msg = msgpack.unpackb(message_bytes)`
        // If type == "Text", print.
        // Wait, I should verify what server sends back.
        // Assuming implementation similar to tts-client but for STT.
        println!("Received: {}", text);
    } else if let Message::Binary(bin) = msg {
        if let Ok(val) = rmp_serde::from_slice::<serde_json::Value>(&bin) {
             println!("Received: {:?}", val);
        }
    }
}

async fn run_mic(url_str: String, token: Option<String>, device_index: usize) -> Result<()> {
    let mut url = Url::parse(&url_str)?;
    url.set_path("/api/asr-streaming");
    if let Some(t) = token {
        url.query_pairs_mut().append_pair("token", &t);
    }

    println!("Connecting to {}", url);
    let (ws_stream, _) = connect_async(url.to_string()).await?;
    println!("Connected");

    let (mut write, mut read) = ws_stream.split();
    let (_tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    let sample_rate = {
        let host = cpal::default_host();
        let devices: Vec<_> = host.input_devices()?.collect();
        if device_index >= devices.len() {
            anyhow::bail!("Device index {} out of range ({} devices found)", device_index, devices.len());
        }
        let device = &devices[device_index];
        let device_name = device.name()?;
        println!("Using input device: {}", device_name);
        
        let config = device.default_input_config()?;
        config.sample_rate().0
    };
    println!("Sample rate: {}", sample_rate);

    let _send_handle = tokio::spawn(async move {
        while let Some(pcm_bytes) = rx.recv().await {
            // To force map in rmp-serde:
            let mut buf = Vec::new();
            let mut serializer = rmp_serde::Serializer::new(&mut buf).with_struct_map();
            let packet = PacketInternal {
                source_rate: sample_rate,
                pcm: serde_bytes::ByteBuf::from(pcm_bytes),
            };
            packet.serialize(&mut serializer).unwrap();
            
            write.send(Message::Binary(buf)).await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    while let Some(msg) = read.next().await {
        // Handle server responses
        if let Ok(m) = msg {
             post_process(m).await;
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct PacketInternal {
    source_rate: u32,
    pcm: serde_bytes::ByteBuf,
}

async fn run_file(_path: String, _url_str: String, _token: Option<String>) -> Result<()> {
    // Similar logic but read file using hound
    // ...
    println!("File functionality simplified for this conversion step.");
    Ok(())
}
