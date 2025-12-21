use anyhow::{Context, Result};
use clap::Parser;
use kyutai_client_core::auth;
use std::env;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Token validity in hours
    #[arg(long, default_value_t = 1.0)]
    hours: f64,
}

fn load_secret() -> Result<String> {
    if let Ok(secret) = env::var("BETTER_AUTH_SECRET") {
        if !secret.trim().is_empty() {
            return Ok(secret);
        }
    }

    let cwd = env::current_dir()?;
    if let Some(secret) = auth::load_secret_from_env_files(&cwd, None)? {
        return Ok(secret);
    }

    anyhow::bail!("BETTER_AUTH_SECRET not found in environment or .env file")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let secret = load_secret().context("Failed to load BETTER_AUTH_SECRET")?;

    let token = auth::generate_token(&secret, cli.hours, "kyutai-tts-rs/0.1.0")?;

    println!("Generated test JWT token:");
    println!();
    println!("{}", token);
    println!();
    println!("Usage with kyutai-tts-rs client:");
    println!(
        "  echo \"Hello world\" | cargo run -p kyutai-tts-rs --release -- --input - --output output.wav --token \"{}\"",
        token
    );
    println!();

    Ok(())
}
