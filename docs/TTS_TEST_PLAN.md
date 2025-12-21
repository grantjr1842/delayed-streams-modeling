# TTS Server + Client Test Plan

## Scope
Validate the moshi-server TTS endpoints (`/api/tts` HTTP and `/api/tts_streaming` WebSocket)
with the Rust client (`kyutai-tts-rs`). Focus on basic correctness, audio output, and
client/server compatibility.

## Preconditions
- Follow build and runtime setup in `docs/MOSHI_SERVER_SETUP.md`.
- Ensure CUDA and model downloads are available (configs reference HF URLs).
- If auth is enabled, set `BETTER_AUTH_SECRET` in `.env` and use a valid JWT token.

## Build
From repo root:

```bash
cargo build -p kyutai-tts-rs --all-features --release
cd server/rust
cargo build --all-features --release
```

## Start Server
Run the TTS worker with a TTS config (pick one):

```bash
# From repo root
cargo run --manifest-path server/rust/moshi/Cargo.toml --bin moshi-server -- \
  worker --config configs/tts/config-tts.toml
```

Optional faster configs for testing:
- `configs/tts/config-tts-fast.toml`
- `configs/tts/config-tts-realtime.toml`

Wait for logs indicating the TTS module is loaded and ready.

## Test Cases

### 1) HTTP TTS (`/api/tts`)
Send a request and decode the returned WAV payload:

```bash
curl -sS -X POST http://127.0.0.1:8080/api/tts \
  -H 'Content-Type: application/json' \
  -d '{"text":["Hello from HTTP"],"seed":42,"temperature":0.8,"top_k":250}' \
  | jq -r '.wav' | base64 -d > /tmp/tts_http.wav
```

Expected:
- HTTP 200 response.
- `/tmp/tts_http.wav` exists and is non-empty.
- Server logs show a completed TTS request without errors.

### 2) WebSocket TTS (`/api/tts_streaming`) via Rust client
Generate audio via the streaming endpoint:

```bash
printf 'Hello from streaming.' | \
  cargo run -p kyutai-tts-rs -r -- \
  --url ws://127.0.0.1:8080 \
  --input - \
  --output /tmp/tts_stream.wav
```

Expected:
- Client connects successfully and finishes without errors.
- `/tmp/tts_stream.wav` exists and is non-empty.
- Server logs show a websocket session completing without errors.

### 3) Streaming benchmark (repeatability)
Run multiple iterations and capture metrics:

```bash
printf 'Benchmark me.' | \
  cargo run -p kyutai-tts-rs -r -- \
  --url ws://127.0.0.1:8080 \
  --input - \
  --output /tmp/tts_stream.wav \
  --runs 3 \
  --json
```

Expected:
- Each run reports `ok: true`.
- `rtf` is reported; lower than 1.0 indicates real-time capability.

### 4) Auth (if enabled)
If auth is required, verify that missing/invalid tokens are rejected and valid tokens pass:

- No token -> expect 401/403.
- Invalid token -> expect 401/403.
- Valid token -> success.

Use `--token <JWT>` with `kyutai-tts-rs` or `?token=<JWT>` for the WebSocket URL.

## Artifacts
Capture and retain:
- `/tmp/tts_http.wav`
- `/tmp/tts_stream.wav`
- Server logs around the request window

## Success Criteria
- Both HTTP and WebSocket flows generate playable audio without errors.
- Streaming runs complete with consistent output and acceptable RTF.
- Auth behavior matches expectations when enabled.
