use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, config::Builder as ConfigBuilder, types::ObjectCannedAcl};
use clap::Parser;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Destination S3 bucket name.
    #[arg(long)]
    bucket: String,

    /// Optional S3 prefix (e.g., 'moshi/logs').
    #[arg(long, default_value = "")]
    prefix: String,

    /// Directory containing friendly logs.
    #[arg(long, default_value = "logs/moshi-logs")]
    source: PathBuf,

    /// Also upload raw traces under logs/moshi-logs/raw.
    #[arg(long)]
    include_raw: bool,

    /// Print the actions without uploading anything.
    #[arg(long)]
    dry_run: bool,

    /// Upload every file even when the remote copy already matches the local hash.
    #[arg(long)]
    force: bool,

    /// Optional AWS profile name.
    #[arg(long)]
    profile: Option<String>,

    /// Optional AWS region override.
    #[arg(long)]
    region: Option<String>,

    /// Custom S3 endpoint URL.
    #[arg(long)]
    endpoint_url: Option<String>,

    /// Optional canned ACL to apply (e.g., public-read).
    #[arg(long)]
    acl: Option<String>,
}

struct PublishStats {
    uploaded: usize,
    skipped: usize,
    dry_run_uploads: usize,
    bytes_uploaded: u64,
}

impl PublishStats {
    fn new() -> Self {
        Self { uploaded: 0, skipped: 0, dry_run_uploads: 0, bytes_uploaded: 0 }
    }
}

// Quick fix for struct field initialization above
impl Default for PublishStats {
    fn default() -> Self {
        Self { uploaded: 0, skipped: 0, dry_run_uploads: 0, bytes_uploaded: 0 }
    }
}

fn compute_md5(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let digest = md5::compute(&buffer);
    Ok(format!("{:x}", digest))
}

fn human_bytes(value: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    for unit in units {
        if size < 1024.0 || unit == "TB" {
            return format!("{:.2} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{} B", value)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    if !cli.source.exists() {
        anyhow::bail!("Source directory {:?} does not exist.", cli.source);
    }

    let region_provider = RegionProviderChain::first_try(cli.region.map(aws_types::region::Region::new))
        .or_default_provider()
        .or_else(aws_types::region::Region::new("us-east-1"));

    let config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider);

    let config_loader = if let Some(profile) = cli.profile {
        config_loader.profile_name(profile)
    } else {
        config_loader
    };

    let sdk_config = config_loader.load().await;
    let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&sdk_config);
    if let Some(endpoint) = cli.endpoint_url {
        s3_config_builder = s3_config_builder.endpoint_url(endpoint);
    }
    let client = Client::from_conf(s3_config_builder.build());

    let mut stats = PublishStats::default();
    let prefix = cli.prefix.trim_matches('/');

    for entry in WalkDir::new(&cli.source) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !cli.include_raw {
            if let Ok(rel) = path.strip_prefix(&cli.source) {
                 if rel.components().any(|c| c.as_os_str() == "raw") {
                     continue;
                 }
            }
        }

        let rel_path = path.strip_prefix(&cli.source)?;
        let rel_path_str = rel_path.to_string_lossy().replace("\\", "/");
        let s3_key = if prefix.is_empty() {
             rel_path_str
        } else {
             format!("{}/{}", prefix, rel_path_str)
        };

        let md5_hex = compute_md5(path)?;
        
        let needs_upload = if cli.force {
            true
        } else {
            match client.head_object().bucket(&cli.bucket).key(&s3_key).send().await {
                Ok(resp) => {
                    let mut remote_match = false;
                    if let Some(metadata) = resp.metadata() {
                        if let Some(remote_md5) = metadata.get("local-md5") {
                            if remote_md5 == &md5_hex {
                                remote_match = true;
                            }
                        }
                    }
                    if !remote_match {
                         if let Some(etag) = resp.e_tag() {
                             let etag_clean = etag.trim_matches('"');
                             if etag_clean == md5_hex {
                                 remote_match = true;
                             }
                         }
                    }
                    
                    if remote_match {
                        stats.skipped += 1;
                        println!("[skip] {} unchanged (s3://{}/{})", rel_path.display(), cli.bucket, s3_key);
                        false
                    } else {
                        true
                    }
                }
                Err(_) => true, // NotFound or other error -> try upload
            }
        };

        if needs_upload {
            if cli.dry_run {
                stats.dry_run_uploads += 1;
                println!("[dry-run] Would upload {} -> s3://{}/{} (md5={})", rel_path.display(), cli.bucket, s3_key, md5_hex);
            } else {
                let body = ByteStream::from_path(path).await?;
                let mut req = client.put_object()
                    .bucket(&cli.bucket)
                    .key(&s3_key)
                    .body(body)
                    .metadata("local-md5", &md5_hex)
                    .content_type("text/plain");

                if let Some(acl_str) = &cli.acl {
                    let acl = match acl_str.as_str() {
                        "private" => ObjectCannedAcl::Private,
                        "public-read" => ObjectCannedAcl::PublicRead,
                        // Add others if needed, clap could handle enum
                        _ => ObjectCannedAcl::Private, 
                    };
                    req = req.acl(acl);
                }

                req.send().await.context(format!("Failed to upload {}", path.display()))?;
                
                let size = fs::metadata(path)?.len();
                stats.uploaded += 1;
                stats.bytes_uploaded += size;
                println!("[upload] {} -> s3://{}/{} ({})", rel_path.display(), cli.bucket, s3_key, human_bytes(size));
            }
        }
    }

    println!();
    println!("Summary:");
    println!("  Uploaded: {}", stats.uploaded);
    println!("  Skipped (unchanged): {}", stats.skipped);
    println!("  Dry-run uploads: {}", stats.dry_run_uploads);
    println!("  Bytes uploaded: {}", human_bytes(stats.bytes_uploaded));

    Ok(())
}
