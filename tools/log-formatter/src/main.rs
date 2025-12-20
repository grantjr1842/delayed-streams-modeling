use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use clap::Parser;
use regex::Regex;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Raw Moshi log (use '-' to read from stdin).
    input: String,

    /// Output path for the formatted log; defaults to the parent of the 
    /// 'raw' directory so the sanitized log lives alongside the raw trace.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Trim whitespace from lines that do not start with a timestamp.
    #[arg(long)]
    strip_raw: bool,
}

fn infer_output_path(input_path: &Path) -> PathBuf {
    if let Some(parent) = input_path.parent() {
        if parent.file_name().and_then(|n| n.to_str()) == Some("raw") {
            if let Some(grandparent) = parent.parent() {
                 if let Some(file_name) = input_path.file_name() {
                     return grandparent.join(file_name);
                 }
            }
        }
    }
    
    let file_name = input_path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("output.log");
        
    input_path.with_file_name(format!("friendly-{}", file_name))
}

fn sanitize_text(line: &str, ansi_regex: &Regex) -> String {
    let text = ansi_regex.replace_all(line, "");
    let text = text.replace("\r", "");
    text.chars()
        .map(|ch| if ch.is_control() && !ch.is_whitespace() { ' ' } else { ch })
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn parse_timestamp(token: &str) -> Option<DateTime<Utc>> {
    let token = token.trim();
    // 2024-05-20T12:34:56.789Z or similar
    // We try robust parsing.
    DateTime::parse_from_rfc3339(token).ok().map(|dt| dt.with_timezone(&Utc))
}

fn format_timestamp(dt: DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.with_timezone(&Local);
    // %Y-%m-%d %I:%M:%S.%3f %p %Z (%z)
    // Rust chrono strftime
    let base = local.format("%Y-%m-%d %I:%M:%S");
    let ms = local.timestamp_subsec_millis();
    let suffix = local.format("%p %Z (%z)");
    format!("{}.{:03} {}", base, ms, suffix)
}

fn reformat_line(line: &str, strip_raw: bool, ansi_regex: &Regex) -> String {
    let sanitized = sanitize_text(line, ansi_regex);
    if sanitized.is_empty() {
        return String::new();
    }
    
    let tokens: Vec<&str> = sanitized.splitn(4, char::is_whitespace).collect();
    if tokens.len() < 3 {
        return if strip_raw { sanitized.trim().to_string() } else { sanitized };
    }
    
    let timestamp_str = tokens[0];
    let level = tokens[1];
    let target = tokens[2];
    let rest = if tokens.len() == 4 { tokens[3] } else { "" };
    
    if let Some(dt) = parse_timestamp(timestamp_str) {
        let formatted_ts = format_timestamp(dt);
        let target_clean = target.trim_end_matches(':');
        let body = rest.trim();
        
        let mut parts = vec![
            format!("[{}]", formatted_ts),
            format!("[{}]", level.to_uppercase()),
            format!("[{}]", target_clean),
        ];
        
        if !body.is_empty() {
            parts.push(body.to_string());
        }
        
        parts.join(" ")
    } else {
        if strip_raw { sanitized.trim().to_string() } else { sanitized }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let ansi_regex = Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]")?;
    
    let input_source: Box<dyn BufRead> = if cli.input == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        let path = Path::new(&cli.input);
        if !path.exists() {
             anyhow::bail!("Input file not found: {}", cli.input);
        }
        Box::new(BufReader::new(File::open(path)?))
    };

    let output_path = if let Some(out) = cli.output {
        Some(out)
    } else if cli.input != "-" {
        Some(infer_output_path(Path::new(&cli.input)))
    } else {
        None
    };

    let mut output_writer: Box<dyn Write> = if let Some(path) = &output_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Box::new(File::create(path)?)
    } else {
        Box::new(io::stdout())
    };

    for line in input_source.lines() {
        let line = line?;
        let formatted = reformat_line(&line, cli.strip_raw, &ansi_regex);
        writeln!(output_writer, "{}", formatted)?;
    }
    
    Ok(())
}
