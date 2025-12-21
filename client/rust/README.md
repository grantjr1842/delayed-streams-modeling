# Client Rust Components

This directory contains all client-side Rust code for the Delayed Streams Modeling project.

## Structure

- **kyutai-client-core/** - Shared auth/WebSocket helpers
- **kyutai-stt-client/** - Speech-to-Text client library
- **kyutai-stt-cli/** - Speech-to-Text CLI
- **tts-rs/** - Text-to-Speech standalone client

## Building

Build the client components from the repo root:

```bash
cargo build -p kyutai-stt-cli -p kyutai-tts-rs --all-features --release
```

## Usage

### STT Client

Run the STT client on an audio file:

```bash
cargo run -p kyutai-stt-cli -r -- file ../../../audio/bria.mp3
```

### TTS Client

Run the TTS client to generate audio:

```bash
cargo run -p kyutai-tts-rs -r -- "Hello world" /tmp/output.wav
```

## Testing

Run all tests:

```bash
cargo test -p kyutai-stt-cli -p kyutai-tts-rs --all-features
```

Run clippy lints:

```bash
cargo clippy -p kyutai-stt-cli -p kyutai-tts-rs --all-targets --all-features
```

## Documentation

For more details, see:
- [Main project README](../../README.md)
