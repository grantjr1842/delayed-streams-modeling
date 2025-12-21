#!/usr/bin/env bash

set -euo pipefail

HTTP_URL="http://127.0.0.1:8080"
WS_URL="ws://127.0.0.1:8080"
TEXT="Hello from the TTS smoke test."
OUTPUT_DIR="/tmp/tts-smoke"
VOICE="expresso/ex03-ex01_happy_001_channel1_334s.wav"
TOKEN=""
TTS_CLIENT_BIN=""

usage() {
  cat <<'USAGE'
Usage: tools/tts-smoke-test.sh [options]

Options:
  --http-url URL        Base HTTP URL (default: http://127.0.0.1:8080)
  --ws-url URL          Base WebSocket URL (default: ws://127.0.0.1:8080)
  --text TEXT           Prompt text (default: "Hello from the TTS smoke test.")
  --output-dir DIR      Output directory (default: /tmp/tts-smoke)
  --voice NAME          Voice name/path (default: expresso/ex03-ex01_happy_001_channel1_334s.wav)
  --token JWT           Bearer token for auth (optional)
  --tts-client-bin PATH Path to kyutai-tts-rs binary (optional)
  -h, --help            Show this help

Notes:
- The moshi-server must already be running with a TTS config.
- If --tts-client-bin is not provided, the script uses cargo to run kyutai-tts-rs.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --http-url)
      HTTP_URL="$2"
      shift 2
      ;;
    --ws-url)
      WS_URL="$2"
      shift 2
      ;;
    --text)
      TEXT="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --voice)
      VOICE="$2"
      shift 2
      ;;
    --token)
      TOKEN="$2"
      shift 2
      ;;
    --tts-client-bin)
      TTS_CLIENT_BIN="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v curl >/dev/null 2>&1; then
  echo "Missing dependency: curl" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "Missing dependency: python3" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
HTTP_OUT="$OUTPUT_DIR/tts_http.wav"
WS_OUT="$OUTPUT_DIR/tts_ws.wav"

HTTP_ENDPOINT="$HTTP_URL"
if [[ "$HTTP_ENDPOINT" != *"/api/tts" ]]; then
  HTTP_ENDPOINT="${HTTP_ENDPOINT%/}/api/tts"
fi

WS_ENDPOINT="$WS_URL"
if [[ "$WS_ENDPOINT" != *"/api/tts_streaming" ]]; then
  WS_ENDPOINT="${WS_ENDPOINT%/}/api/tts_streaming"
fi

payload=$(python3 - <<PY
import json
print(json.dumps({
  "text": ["$TEXT"],
  "seed": 42,
  "temperature": 0.8,
  "top_k": 250,
  "voice": "$VOICE",
}))
PY
)

curl_headers=(
  -H "Content-Type: application/json"
)
if [[ -n "$TOKEN" ]]; then
  curl_headers+=( -H "Authorization: Bearer $TOKEN" )
fi

printf "[http] POST %s\n" "$HTTP_ENDPOINT"
response=$(curl -sS -X POST "$HTTP_ENDPOINT" "${curl_headers[@]}" -d "$payload" -w "\n%{http_code}")
body=${response%$'\n'*}
status=${response##*$'\n'}

if [[ "$status" != "200" ]]; then
  echo "HTTP request failed (status $status)." >&2
  echo "$body" >&2
  exit 1
fi

python3 - <<PY
import base64
import json
import sys

body = sys.stdin.read()
parsed = json.loads(body)
if "wav" not in parsed:
    raise SystemExit("Missing 'wav' in response")
with open("$HTTP_OUT", "wb") as f:
    f.write(base64.b64decode(parsed["wav"]))
PY
<<< "$body"

printf "[http] Wrote %s\n" "$HTTP_OUT"

printf "[ws] Connect %s\n" "$WS_ENDPOINT"
if [[ -n "$TTS_CLIENT_BIN" ]]; then
  printf '%s' "$TEXT" | "$TTS_CLIENT_BIN" --url "$WS_ENDPOINT" --input - --output "$WS_OUT" ${TOKEN:+--token "$TOKEN"}
else
  printf '%s' "$TEXT" | cargo run -p kyutai-tts-rs -r -- \
    --url "$WS_ENDPOINT" \
    --input - \
    --output "$WS_OUT" \
    ${TOKEN:+--token "$TOKEN"}
fi

printf "[ws] Wrote %s\n" "$WS_OUT"
printf "Smoke test complete.\n"
