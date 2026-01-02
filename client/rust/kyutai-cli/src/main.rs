use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod stt;
mod tts;

#[derive(Parser, Debug)]
#[command(author, version, about = "Kyutai Unified CLI for STT and TTS")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Speech-to-Text commands
    Stt(stt::SttArgs),
    /// Text-to-Speech commands
    Tts(tts::TtsArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Stt(args) => stt::run_stt(args).await?,
        Commands::Tts(args) => tts::run_tts(args).await?,
    }

    Ok(())
}
