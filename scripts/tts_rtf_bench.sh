#!/usr/bin/env bash
# TTS RTF Benchmark Script
# Profiles TTS performance with different n_q values
#
# Usage:
#   ./scripts/tts_rtf_bench.sh [server_url] [runs_per_config]
#
# Prerequisites:
#   - TTS server must be restarted between n_q changes (manually)
#   - kyutai-tts-rs client must be built
#
# Output:
#   - CSV file with RTF metrics per configuration

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TTS_CLIENT="$REPO_ROOT/tts-rs/target/release/kyutai-tts-rs"

SERVER_URL="${1:-ws://127.0.0.1:8080}"
RUNS="${2:-5}"
OUTPUT_DIR="$REPO_ROOT/tmp/tts_benchmarks"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
OUTPUT_CSV="$OUTPUT_DIR/tts_rtf_bench_$TIMESTAMP.csv"

# Test phrase (consistent across all runs)
TEST_TEXT="The quick brown fox jumps over the lazy dog. This is a test of the text to speech system for performance benchmarking."

# n_q values to test (requires server restart between each)
NQ_VALUES=(4 8 12 16)

echo "=== TTS RTF Benchmark ==="
echo "Server URL: $SERVER_URL"
echo "Runs per config: $RUNS"
echo "Output: $OUTPUT_CSV"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Check if client exists
if [[ ! -x "$TTS_CLIENT" ]]; then
    echo "Error: TTS client not found at $TTS_CLIENT"
    echo "Build it with: cd tts-rs && cargo build --release"
    exit 1
fi

# Write CSV header
echo "n_q,run_idx,ok,ttfb_ms,total_ms,audio_seconds,rtf,x_real_time,cpal_underruns,max_audio_rx_gap_ms" > "$OUTPUT_CSV"

# Function to run benchmark for current server config
run_benchmark() {
    local nq="$1"
    local label="$2"
    
    echo ""
    echo "--- Benchmarking n_q=$nq ($label) ---"
    echo "Running $RUNS iterations..."
    
    # Create temp file for input text
    local input_file=$(mktemp)
    echo "$TEST_TEXT" > "$input_file"
    
    # Run benchmark with JSON output
    for i in $(seq 1 "$RUNS"); do
        local output_wav="$OUTPUT_DIR/bench_nq${nq}_run${i}.wav"
        
        # Run TTS and capture JSON output
        local result
        if result=$("$TTS_CLIENT" \
            --url "$SERVER_URL" \
            --input "$input_file" \
            --output "$output_wav" \
            --json 2>&1); then
            
            # Parse JSON and append to CSV
            echo "$result" | python3 -c "
import json, sys
try:
    data = json.loads(sys.stdin.read().strip())
    print(f\"$nq,{data.get('run_idx', $i)},{data.get('ok', False)},{data.get('ttfb_ms', '')},{data.get('total_ms', '')},{data.get('audio_seconds', '')},{data.get('rtf', '')},{data.get('x_real_time', '')},{data.get('cpal_underruns', '')},{data.get('max_audio_rx_gap_ms', '')}\")
except json.JSONDecodeError:
    print(f\"$nq,$i,error,,,,,,,\", file=sys.stderr)
" >> "$OUTPUT_CSV"
            
            echo "  Run $i: OK (RTF=$(echo "$result" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(f\"{d.get('rtf', 'N/A'):.2f}\" if d.get('rtf') else 'N/A')" 2>/dev/null || echo "N/A"))"
        else
            echo "$nq,$i,error,,,,,,," >> "$OUTPUT_CSV"
            echo "  Run $i: FAILED"
        fi
    done
    
    rm -f "$input_file"
}

# Instructions for manual testing
echo ""
echo "=== MANUAL TESTING MODE ==="
echo ""
echo "This script requires the TTS server to be restarted with different configs."
echo "For each n_q value, you need to:"
echo "  1. Edit configs/config-tts.toml to set n_q=<value>"
echo "     OR use a pre-made config file"
echo "  2. Restart the TTS server"
echo "  3. Press Enter when ready"
echo ""
echo "Alternatively, test with current server config:"
echo ""
read -p "Press Enter to benchmark CURRENT server config (or Ctrl+C to exit): "

# Run benchmark for current server config
run_benchmark "current" "current server config"

echo ""
echo "=== Benchmark Complete ==="
echo "Results saved to: $OUTPUT_CSV"
echo ""
echo "Summary:"
cat "$OUTPUT_CSV" | python3 -c "
import csv
import sys

reader = csv.DictReader(sys.stdin)
rows = list(reader)
if not rows:
    print('  No data')
    sys.exit(0)

rtfs = [float(r['rtf']) for r in rows if r['rtf'] and r['ok'] == 'True']
if rtfs:
    print(f'  Runs: {len(rtfs)}')
    print(f'  RTF avg: {sum(rtfs)/len(rtfs):.3f}')
    print(f'  RTF min: {min(rtfs):.3f}')
    print(f'  RTF max: {max(rtfs):.3f}')
else:
    print('  No successful runs with RTF data')
"
