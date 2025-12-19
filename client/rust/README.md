# Client Rust Components

This directory contains all client-side Rust code for the Delayed Streams Modeling project.

## Structure

- **stt-rs/** - Speech-to-Text standalone client
- **tts-rs/** - Text-to-Speech standalone client

## Building

Build all client components:

```bash
cd client/rust
cargo build --all-features --release
```

## Usage

### STT Client

Run the STT client on an audio file:

```bash
cd client/rust/stt-rs
cargo run --features cuda -r -- ../../../audio/bria.mp3
```

Add `--timestamps` flag to see word-level timestamps:

```bash
cargo run --features cuda -r -- --timestamps ../../../audio/bria.mp3
```

Add `--vad` flag to see semantic VAD output:

```bash
cargo run --features cuda -r -- --vad ../../../audio/bria.mp3
```

### TTS Client

Run the TTS client to generate audio:

```bash
cd client/rust/tts-rs
cargo run -r -- "Hello world" /tmp/output.wav
```

## Testing

Run all tests:

```bash
cd client/rust
cargo test --all-features
```

Run clippy lints:

```bash
cd client/rust
cargo clippy --all-targets --all-features
```

## Documentation

For more details, see:
- [Main project README](../../README.md)
