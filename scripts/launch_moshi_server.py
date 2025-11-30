# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "tomli-w",
# ]
# ///

import argparse
import subprocess
import sys
import os
import tomllib
import tempfile
import shutil
import tomli_w

def get_available_vram_mb():
    """Returns available VRAM in MB using nvidia-smi."""
    try:
        # Query memory.free for all GPUs. We'll use the first one or the one specified by CUDA_VISIBLE_DEVICES if simple.
        # For simplicity, we just take the first one reported.
        result = subprocess.run(
            ['nvidia-smi', '--query-gpu=memory.free', '--format=csv,noheader,nounits'],
            capture_output=True, text=True, check=True
        )
        # Output is like "12345\n" or "12345\n23456\n"
        lines = result.stdout.strip().split('\n')
        if not lines:
            return None
        # Just take the first one for now
        return int(lines[0])
    except (subprocess.CalledProcessError, FileNotFoundError, ValueError):
        print("Warning: Could not detect VRAM using nvidia-smi. Skipping auto-adjustment.", file=sys.stderr)
        return None

def adjust_config(config_path, vram_mb):
    try:
        with open(config_path, "rb") as f:
            config = tomllib.load(f)
    except Exception as e:
        print(f"Error parsing config file {config_path}: {e}", file=sys.stderr)
        return None
    
    # Heuristic: 400MB per batch item
    # Allow override via env var
    try:
        mem_per_item = int(os.environ.get("MOSHI_MEMORY_PER_BATCH_MB", 400))
    except ValueError:
        mem_per_item = 400
        
    max_batch_size = max(1, vram_mb // mem_per_item)
    
    print(f"Detected {vram_mb} MB VRAM. Max safe batch size estimated at {max_batch_size} (assuming {mem_per_item} MB/item).", file=sys.stderr)
    
    modified = False
    if "modules" in config:
        for name, module in config["modules"].items():
            # Check for BatchedAsr type or just presence of batch_size
            if "batch_size" in module:
                current_bs = module["batch_size"]
                if isinstance(current_bs, int) and current_bs > max_batch_size:
                    print(f"Adjusting batch_size for module '{name}' from {current_bs} to {max_batch_size}.", file=sys.stderr)
                    module["batch_size"] = max_batch_size
                    modified = True
    
    return config if modified else None

def main():
    # Basic argument parsing to find --config
    if "--config" not in sys.argv:
        # Just run moshi-server directly if no config specified
        try:
            subprocess.run(["moshi-server"] + sys.argv[1:], check=True)
        except subprocess.CalledProcessError as e:
            sys.exit(e.returncode)
        except FileNotFoundError:
            print("Error: moshi-server not found in PATH.", file=sys.stderr)
            sys.exit(1)
        return

    try:
        config_idx = sys.argv.index("--config")
    except ValueError:
        config_idx = -1

    if config_idx != -1 and config_idx + 1 >= len(sys.argv):
        print("Error: --config flag provided without value", file=sys.stderr)
        sys.exit(1)
        
    config_path = sys.argv[config_idx + 1]
    
    vram_mb = get_available_vram_mb()
    
    cleanup_temp = False
    temp_config_path = None
    cmd_args = sys.argv[1:]

    if vram_mb:
        new_config_data = adjust_config(config_path, vram_mb)
        if new_config_data:
            # Create temp file
            fd, temp_config_path = tempfile.mkstemp(suffix=".toml", text=True)
            os.close(fd) 
            
            with open(temp_config_path, "wb") as f:
                tomli_w.dump(new_config_data, f)
            
            cleanup_temp = True
            
            # Replace config path in args
            cmd_args[config_idx + 1] = temp_config_path

    try:
        # print(f"Launching: moshi-server {' '.join(cmd_args)}", file=sys.stderr)
        subprocess.run(["moshi-server"] + cmd_args, check=True)
    except subprocess.CalledProcessError as e:
        sys.exit(e.returncode)
    except FileNotFoundError:
        print("Error: moshi-server not found in PATH.", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
    finally:
        if cleanup_temp and temp_config_path and os.path.exists(temp_config_path):
            os.remove(temp_config_path)

if __name__ == "__main__":
    main()
