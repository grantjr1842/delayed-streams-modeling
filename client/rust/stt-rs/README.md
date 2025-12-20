# Kyutai STT Rust Client

A Rust client library and CLI for streaming audio to the **Kyutai STT server** (`moshi-server`) and receiving real-time word-level transcription events.

## Features

- **Real-time streaming** — Stream microphone audio or files to the STT server over WebSocket
- **Word-level timestamps** — Receive `Word` + `EndWord` events with precise timing information
- **Utterance assembly** — Automatic utterance finalization with configurable timeout (default: 1500ms)
- **Better Auth JWT** — Built-in authentication support via Bearer header or query parameter
- **Keepalive** — Application-level ping to prevent server timeouts during silence
- **Flexible audio input** — Microphone capture (`cpal`) or file decode (`kaudio`)

## Crates

| Crate | Description |
|-------|-------------|
| [`kyutai-stt-client`](crates/kyutai-stt-client) | Core library with WebSocket transport, MessagePack protocol, and transcript assembly |
| [`kyutai-stt-cli`](crates/kyutai-stt-cli) | Command-line interface for mic/file transcription |

## Quick Start

### Prerequisites

- Rust 2024 edition (1.85+)
- A running Kyutai STT server (`moshi-server`)
- Valid JWT token for authentication

### Installation

```bash
# Clone the repository
git clone https://github.com/grantjr1842/stt-rust-client.git
cd stt-rust-client

# Build the project
cargo build --all-features --release --verbose --verbose
```

### Usage

```bash
# Stream from microphone (requires a valid token)
cargo run -p kyutai-stt-cli -- --auth-token <JWT> mic

# Stream from audio file
cargo run -p kyutai-stt-cli -- --url ws://localhost:8080/api/asr-streaming --auth-token <JWT> file input.wav

# Stream from audio file with progress + RTF status line
cargo run -p kyutai-stt-cli -- --auth-token <JWT> file input.wav --progress

# Show real-time input level meter (RMS/peak dB)
cargo run -p kyutai-stt-cli -- --auth-token <JWT> mic --show-level

# Enable verbose logging of model performance (VAD step info)
RUST_LOG=info cargo run -p kyutai-stt-cli -- --auth-token <JWT> mic --verbose

# Buffer transcript output (higher throughput, slightly higher latency)
cargo run -p kyutai-stt-cli -- --auth-token <JWT> mic --buffered-output

# Auto-generate JWT from .env.development (or set ENV to choose .env.<ENV>)
cargo run -p kyutai-stt-cli -- --env development mic --auto-token

# Generate a JWT token directly
cargo run -p kyutai-stt-cli -- --env development token --hours 2

# Test microphone input without server (useful for debugging audio setup)
cargo run -p kyutai-stt-cli -- mic-test

# Test microphone for 5 seconds and save to WAV file
cargo run -p kyutai-stt-cli -- mic-test --duration 5 --save-wav /tmp/test.wav
```

## Audio Requirements

The STT server expects:

- **Sample rate**: 24,000 Hz
- **Channels**: Mono
- **Frame size**: 1920 samples (80ms)
- **Format**: Float32 PCM

The client automatically resamples input audio to meet these requirements.

## Architecture

```mermaid
flowchart LR
  Audio[Mic / File] --> Resample[24kHz Mono]
  Resample --> Chunk[1920 samples]
  Chunk --> WS[WebSocket]
  WS --> Server[moshi-server]
  Server --> Events[Word/EndWord]
  Events --> Transcript[Utterance Assembly]
```

## Library Features

The `kyutai-stt-client` library supports feature flags:

| Feature | Default | Description |
|---------|---------|-------------|
| `mic` | ✓ | Microphone capture via `cpal` |
| `file` | ✓ | Audio file decoding via `kaudio` |
| `hq-resample` | | High-quality resampling via `rubato` |

## Development

### Local Checks

Run the same checks locally that CI would run:

```bash
./scripts/ci.sh
```

Or run them individually:

```bash
cargo fmt --all --check --verbose
cargo clippy --all-targets --all-features --verbose --verbose
cargo test --all-features --verbose --verbose
```

### Iteration Loop

For fast feedback during development:

```bash
cargo check --all-targets --all-features --verbose --verbose
```

## Documentation

- See [`SYSTEM_DESIGN.md`](SYSTEM_DESIGN.md) for the complete architecture and implementation guide

## License

MIT
