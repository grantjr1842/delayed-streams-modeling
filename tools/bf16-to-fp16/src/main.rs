use anyhow::Result;
use candle_core::{DType, Device, Tensor};
use clap::{Parser, ValueEnum};
use hf_hub::{api::sync::Api, Repo, RepoType};

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Hugging Face repo id that hosts the source checkpoint.
    #[arg(long, default_value = "kyutai/stt-1b-en_fr-candle")]
    hf_repo: String,

    /// Model file inside the repo (bf16 safetensors).
    #[arg(long, default_value = "model.safetensors")]
    model_file: String,

    /// Optional local path to the bf16 checkpoint (skips HF download).
    #[arg(long)]
    input_path: Option<PathBuf>,

    /// Destination path for the converted safetensors file.
    #[arg(long, default_value = "assets/fp16/stt-1b-en_fr-candle.fp16.safetensors")]
    output: PathBuf,

    /// Target dtype for bf16 tensors.
    #[arg(long, value_enum, default_value_t = TargetDType::Float16)]
    dtype: TargetDType,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum TargetDType {
    Float16,
    Float32,
}

fn resolve_source(cli: &Cli) -> Result<PathBuf> {
    if let Some(input_path) = &cli.input_path {
        if !input_path.exists() {
            anyhow::bail!("Input checkpoint {:?} does not exist", input_path);
        }
        Ok(input_path.clone())
    } else {
        let api = Api::new()?;
        let repo = api.repo(Repo::new(cli.hf_repo.clone(), RepoType::Model));
        let path = repo.get(&cli.model_file)?;
        Ok(path)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let src = resolve_source(&cli)?;
    let dst = cli.output.clone();
    
    let target_dtype = match cli.dtype {
        TargetDType::Float16 => DType::F16,
        TargetDType::Float32 => DType::F32,
    };

    println!("Loading tensors from {:?}", src);
    
    let mut converted_tensors: HashMap<String, Tensor> = HashMap::new();
    let mut count_converted = 0;
    let mut count_unchanged = 0;
    // Better approach: use candle_core::safetensors::load
    let tensors = candle_core::safetensors::load(&src, &Device::Cpu)?;
    
    for (name, tensor) in tensors {
        if tensor.dtype() == DType::BF16 {
            let converted = tensor.to_dtype(target_dtype)?;
            converted_tensors.insert(name, converted);
            count_converted += 1;
        } else {
            converted_tensors.insert(name, tensor);
            count_unchanged += 1;
        }
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    candle_core::safetensors::save(&converted_tensors, &dst)?;

    println!(
        "Saved {:?} with dtype={:?} (converted {} tensors, {} untouched)",
        dst, target_dtype, count_converted, count_unchanged
    );

    Ok(())
}
