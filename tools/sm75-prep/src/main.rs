use anyhow::{Context, Result};
use clap::Parser;
use nvml_wrapper::Nvml;

use std::path::PathBuf;
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Hugging Face repo containing the bf16 checkpoint.
    #[arg(long, default_value = "kyutai/stt-1b-en_fr-candle")]
    hf_repo: String,

    /// Filename inside the HF repo.
    #[arg(long, default_value = "model.safetensors")]
    model_file: String,

    /// Optional local bf16 checkpoint to convert (skips the Hugging Face download).
    #[arg(long)]
    input_path: Option<PathBuf>,

    /// Destination path for the fp16 checkpoint.
    #[arg(long, default_value = "assets/fp16/stt-1b-en_fr-candle.fp16.safetensors")]
    output: PathBuf,

    /// Target dtype for converted tensors.
    #[arg(long, default_value = "float16")]
    dtype: String,

    /// Simulate a GPU (e.g., sm75).
    #[arg(long)]
    simulate: Option<String>,

    /// Always run the conversion, even when Ampere GPUs are detected.
    #[arg(long)]
    force: bool,

    /// Skip the conversion when GPU detection fails.
    #[arg(long)]
    skip_when_undetected: bool,

    /// Only print the converter command instead of executing it.
    #[arg(long)]
    dry_run: bool,
}

struct DeviceCapability {
    index: u32,
    name: String,
    major: i32,
    minor: i32,
}

impl DeviceCapability {
    fn is_pre_ampere(&self) -> bool {
        self.major < 8
    }
}

fn detect_devices(simulate: Option<&String>) -> Vec<DeviceCapability> {
    if let Some(sim) = simulate {
        // Simple simulation parsing: sm75 -> major=7, minor=5
        let sim = sim.trim().trim_start_matches("sm");
        if sim.len() >= 2 {
             if let (Ok(major), Ok(minor)) = (sim[0..1].parse(), sim[1..2].parse()) {
                 return vec![DeviceCapability {
                     index: 0,
                     name: format!("Simulated sm{}{}", major, minor),
                     major,
                     minor,
                 }];
             }
        }
    }

    let mut devices = Vec::new();
    if let Ok(nvml) = Nvml::init() {
        if let Ok(count) = nvml.device_count() {
            for i in 0..count {
                if let Ok(device) = nvml.device_by_index(i) {
                     if let Ok(cap) = device.cuda_compute_capability() {
                         let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                         devices.push(DeviceCapability {
                             index: i,
                             name,
                             major: cap.major,
                             minor: cap.minor,
                         });
                     }
                }
            }
        }
    }
    devices
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    
    // We assume bf16-to-fp16 is built and available via cargo run
    // In a real installed scenario, we'd expect the binary in PATH.
    // For this repo, we use cargo run --bin bf16-to-fp16
    
    let devices = detect_devices(cli.simulate.as_ref());
    
    if devices.is_empty() {
        println!("No CUDA devices detected (or simulation failed).");
        if cli.skip_when_undetected && !cli.force {
             println!("Skipping conversion.");
             return Ok(ExitCode::SUCCESS);
        }
    } else {
        println!("Detected CUDA devices:");
        for dev in &devices {
            let status = if dev.is_pre_ampere() { "PRE-AMPERE" } else { "Ampere+" };
            println!("- #{} {} (sm{}{}, {})", dev.index, dev.name, dev.major, dev.minor, status);
        }
    }

    let needs_conversion = cli.force || devices.iter().any(|d| d.is_pre_ampere()) || (devices.is_empty() && !cli.skip_when_undetected);

    if !needs_conversion {
        println!("No pre-Ampere GPUs detected and --force not set; skipping the conversion.");
        return Ok(ExitCode::SUCCESS);
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--quiet").arg("--bin").arg("bf16-to-fp16").arg("--");
    
    cmd.arg("--hf-repo").arg(&cli.hf_repo);
    cmd.arg("--model-file").arg(&cli.model_file);
    cmd.arg("--output").arg(&cli.output);
    if let Some(input) = &cli.input_path {
        cmd.arg("--input-path").arg(input);
    }
    // bf16-to-fp16 takes enum, so we pass string that matches ValueEnum
    cmd.arg("--dtype").arg(&cli.dtype);
    
    if cli.dry_run {
        println!("[dry-run] Would execute: {:?}", cmd);
        return Ok(ExitCode::SUCCESS);
    }

    println!("Running conversion...");
    let status = cmd.status().context("Failed to execute bf16-to-fp16")?;
    
    if status.success() {
        println!("fp16 checkpoint ready at {:?}.", cli.output);
        Ok(ExitCode::SUCCESS)
    } else {
        anyhow::bail!("Conversion failed with status: {}", status);
    }
}
