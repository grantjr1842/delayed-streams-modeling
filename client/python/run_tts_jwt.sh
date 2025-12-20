#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/run_kyutai_tts_rs_with_jwt.sh <input> <output> [options]

Options:
  --tts-url <url>            Default: ws://127.0.0.1:8080
  --voice <voice>            Default: expresso/ex03-ex01_happy_001_channel1_334s.wav
  --runs <n>                 Default: 1
  --json                     Print one JSON object per run
  --seed <n>                 Default: 42
  --temperature <f>          Default: 0.8
  --top-k <n>                Default: 250

Token source (choose one):
  --token <jwt>              Use an existing JWT
  --mint                     Mint a local dev token using BETTER_AUTH_SECRET (via generate_test_token.py)
  --ttl-hours <hours>        Token TTL when minting (default: 1)
  --auth-url <url>           Default: http://localhost:3001
  --login-path <path>        Default: /api/auth/sign-in/email
  --email <email>            Login email (used with --password)
  --password <password>      Login password

Execution:
  --tts-bin <path|cargo>     Default: kyutai-tts-rs (uses PATH). Use 'cargo' to run via cargo.
  --no-run                   Only acquire token and print it (redacted)

Notes:
  - moshi-server must be started with the same BETTER_AUTH_SECRET for JWT validation.
  - kyutai-tts-rs passes the token via ?token=... on the WebSocket URL.
EOF
}

redact() {
  local token="$1"
  local len=${#token}
  if [[ $len -le 16 ]]; then
    printf '%s' '<redacted>'
    return
  fi
  printf '%s…%s' "${token:0:8}" "${token:len-8:8}"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

INPUT=${1:-}
OUTPUT=${2:-}
if [[ -z "${INPUT}" || -z "${OUTPUT}" ]]; then
  usage >&2
  exit 2
fi
shift 2

TTS_URL="ws://127.0.0.1:8080"
VOICE="expresso/ex03-ex01_happy_001_channel1_334s.wav"
RUNS=1
JSON=0
SEED=42
TEMPERATURE="0.8"
TOP_K=250

TOKEN=""
MINT=0
TTL_HOURS="1"

AUTH_URL="http://localhost:3001"
LOGIN_PATH="/api/auth/sign-in/email"
EMAIL=""
PASSWORD=""

TTS_BIN="kyutai-tts-rs"
NO_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --tts-url)
      TTS_URL="$2"; shift 2 ;;
    --voice)
      VOICE="$2"; shift 2 ;;
    --runs)
      RUNS="$2"; shift 2 ;;
    --json)
      JSON=1; shift 1 ;;
    --seed)
      SEED="$2"; shift 2 ;;
    --temperature)
      TEMPERATURE="$2"; shift 2 ;;
    --top-k)
      TOP_K="$2"; shift 2 ;;

    --token)
      TOKEN="$2"; shift 2 ;;
    --mint)
      MINT=1; shift 1 ;;
    --ttl-hours)
      TTL_HOURS="$2"; shift 2 ;;

    --auth-url)
      AUTH_URL="$2"; shift 2 ;;
    --login-path)
      LOGIN_PATH="$2"; shift 2 ;;
    --email)
      EMAIL="$2"; shift 2 ;;
    --password)
      PASSWORD="$2"; shift 2 ;;

    --tts-bin)
      TTS_BIN="$2"; shift 2 ;;
    --no-run)
      NO_RUN=1; shift 1 ;;

    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "${REPO_ROOT}/.agent/tmp"
COOKIE_JAR="${REPO_ROOT}/.agent/tmp/better-auth.cookies.txt"

get_token_via_login() {
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl not found" >&2
    exit 1
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 not found (required to safely build JSON request body)" >&2
    exit 1
  fi

  if [[ -z "${EMAIL}" || -z "${PASSWORD}" ]]; then
    echo "--email and --password are required for login" >&2
    exit 2
  fi

  local sign_in_url
  sign_in_url="${AUTH_URL%/}${LOGIN_PATH}"

  local body
  body="$(python3 - <<PY
import json
import os
email = os.environ.get("_BA_EMAIL", "")
password = os.environ.get("_BA_PASSWORD", "")
print(json.dumps({"email": email, "password": password, "rememberMe": True}))
PY
)"

  _BA_EMAIL="${EMAIL}" _BA_PASSWORD="${PASSWORD}" \
    curl -fsS -X POST "${sign_in_url}" \
      -H 'Content-Type: application/json' \
      -H 'Accept: application/json' \
      -c "${COOKIE_JAR}" \
      -b "${COOKIE_JAR}" \
      --data-raw "${body}" \
      >/dev/null

  local tok
  tok="$(awk '($6=="better-auth.session_token"){print $7; exit 0}' "${COOKIE_JAR}" || true)"

  if [[ -z "${tok}" ]]; then
    echo "Sign-in did not produce cookie better-auth.session_token (see ${COOKIE_JAR})" >&2
    exit 1
  fi

  printf '%s' "${tok}"
}

get_token_via_mint() {
  local out
  if command -v uv >/dev/null 2>&1; then
    out="$(uv run "${REPO_ROOT}/scripts/generate_test_token.py" --hours "${TTL_HOURS}")"
  elif command -v python3 >/dev/null 2>&1; then
    out="$(python3 "${REPO_ROOT}/scripts/generate_test_token.py" --hours "${TTL_HOURS}")"
  else
    echo "Neither uv nor python3 found; cannot mint token" >&2
    exit 1
  fi

  local tok
  tok="$(printf '%s\n' "${out}" | awk '/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/{print; exit 0}')"
  if [[ -z "${tok}" ]]; then
    echo "Failed to parse JWT from generate_test_token.py output" >&2
    exit 1
  fi
  printf '%s' "${tok}"
}

if [[ -z "${TOKEN}" ]]; then
  if [[ ${MINT} -eq 1 ]]; then
    TOKEN="$(get_token_via_mint)"
  elif [[ -n "${EMAIL}" && -n "${PASSWORD}" ]]; then
    TOKEN="$(get_token_via_login)"
  else
    TOKEN="$(get_token_via_mint)"
  fi
fi

echo "JWT: $(redact "${TOKEN}")"

if [[ ${NO_RUN} -eq 1 ]]; then
  exit 0
fi

TTS_ARGS=(
  "${INPUT}"
  "${OUTPUT}"
  --url "${TTS_URL}"
  --voice "${VOICE}"
  --token "${TOKEN}"
  --runs "${RUNS}"
  --seed "${SEED}"
  --temperature "${TEMPERATURE}"
  --top-k "${TOP_K}"
)
if [[ ${JSON} -eq 1 ]]; then
  TTS_ARGS+=(--json)
fi

if [[ "${TTS_BIN}" == "cargo" ]]; then
  exec cargo run --manifest-path "${REPO_ROOT}/tts-rs/Cargo.toml" --release -- "${TTS_ARGS[@]}"
fi

exec "${TTS_BIN}" "${TTS_ARGS[@]}"
