use anyhow::Result;
use clap::Parser;
use nvml_wrapper::Nvml;
use serde::Serialize;
use std::process::ExitCode;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON instead of a friendly summary
    #[arg(long)]
    json: bool,

    /// Exit with status 2 if any detected GPU requires the fp16 conversion (pre-Ampere)
    #[arg(long)]
    fail_on_pre_ampere: bool,
}

#[derive(Serialize)]
struct DeviceCapability {
    index: u32,
    name: String,
    sm_tag: String,
    compute_capability: f64,
    is_pre_ampere: bool,
    source: String,
}

const AMPERE_MAJOR: i32 = 8;
const CONVERTER_CMD: &str =
    "cargo run --bin sm75-prep -- --output assets/fp16/stt-1b-en_fr-candle.fp16.safetensors";
const SM75_CONFIG: &str = "configs/stt/config-stt-en_fr-lowram-sm75.toml";

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut devices = Vec::new();

    // Try NVML first
    match Nvml::init() {
        Ok(nvml) => {
            match nvml.device_count() {
                Ok(count) => {
                    for i in 0..count {
                        if let Ok(device) = nvml.device_by_index(i) {
                            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                            if let Ok(capability) = device.cuda_compute_capability() {
                                let major = capability.major;
                                let minor = capability.minor;
                                let compute_capability = major as f64 + (minor as f64 / 10.0);
                                let is_pre_ampere = major < AMPERE_MAJOR;
                                
                                devices.push(DeviceCapability {
                                    index: i,
                                    name,
                                    sm_tag: format!("sm{}{}", major, minor),
                                    compute_capability,
                                    is_pre_ampere,
                                    source: "nvml".to_string(),
                                });
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => {
            // NVML failed, fallback handled by empty list check later
        }
    }

    // TODO: Simulate logic could be added here if needed, but for now we focus on real hardware

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
    } else {
        if devices.is_empty() {
             println!("No CUDA-capable GPU detected via NVML. Run this helper on the machine that executes moshi-server.");
        } else {
            println!("Detected {} CUDA device(s):", devices.len());
            for dev in &devices {
                let status = if dev.is_pre_ampere { "PRE-AMPERE" } else { "Ampere+" };
                println!("- #{} {} [{}] (compute capability {:.1}, source={}, {})", 
                    dev.index, dev.name, dev.sm_tag, dev.compute_capability, dev.source, status);
            }

            let risky: Vec<_> = devices.iter().filter(|d| d.is_pre_ampere).collect();
            if !risky.is_empty() {
                println!();
                println!("The devices above with compute capability < 8.0 cannot load the bf16 checkpoint. Convert the model first and switch to the SM75 config:");
                println!("  1. {}", CONVERTER_CMD);
                println!("  2. Use {} so moshi-server forces float16 weights.", SM75_CONFIG);
            }
        }
    }

    if cli.fail_on_pre_ampere && devices.iter().any(|d| d.is_pre_ampere) {
        return Ok(ExitCode::from(2));
    }

    Ok(ExitCode::SUCCESS)
}
