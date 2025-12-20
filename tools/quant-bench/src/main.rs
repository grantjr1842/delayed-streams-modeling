use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Stt(SttArgs),
    Tts(TtsArgs),
}

#[derive(clap::Args)]
struct SttArgs {
    /// Audio files to transcribe.
    #[arg(required = true)]
    audio_files: Vec<String>,
}

#[derive(clap::Args)]
struct TtsArgs {
    /// Text to synthesize.
    #[arg(long)]
    text: String,
}

fn main() {
    let _cli = Cli::parse();
    println!("quant-bench is not yet fully implemented in Rust.");
    println!("Please refer to the implementation plan for Phase 3/4 integration details.");
}
