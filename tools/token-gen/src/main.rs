use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Token validity in hours
    #[arg(long, default_value_t = 1.0)]
    hours: f64,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
struct UserConfig {
    id: String,
    name: String,
    email: String,
    #[serde(rename = "emailVerified")]
    email_verified: bool,
    image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    session: SessionConfig,
    user: UserConfig,
    iat: i64,
    exp: i64,
}

fn load_secret() -> Result<String> {
    // Try environment first
    if let Ok(secret) = env::var("BETTER_AUTH_SECRET") {
        return Ok(secret);
    }

    // Try .env file using dotenvy
    if dotenvy::dotenv().is_ok() {
        // dotenvy loads into env vars
        if let Ok(secret) = env::var("BETTER_AUTH_SECRET") {
             return Ok(secret);
        }
    }

    anyhow::bail!("BETTER_AUTH_SECRET not found in environment or .env file")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let secret = load_secret().context("Failed to load BETTER_AUTH_SECRET")?;

    let now = Utc::now();
    let duration_secs = (cli.hours * 3600.0) as i64;
    let exp = now + Duration::seconds(duration_secs);

    let claims = Claims {
        session: SessionConfig {
            id: "test-session-id".to_string(),
            user_id: "test-user-id".to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            expires_at: exp.to_rfc3339(),
            token: "test-session-token".to_string(),
            ip_address: "127.0.0.1".to_string(),
            user_agent: "kyutai-tts-rs/0.1.0".to_string(),
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

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    println!("Generated test JWT token:");
    println!();
    println!("{}", token);
    println!();
    println!("Usage with tts-rs client:");
    println!("  echo \"Hello world\" | cargo run --release -- - output.wav --token \"{}\"", token);
    println!();

    Ok(())
}
