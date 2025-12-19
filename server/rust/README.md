# Server Rust Components

This directory contains all server-side Rust code for the Delayed Streams Modeling project.

## Structure

- **moshi/** - Moshi server workspace
  - **moshi-server/** - Main server binary
  - **moshi-core/** - Core library with model implementations
  - **moshi-backend/** - Backend infrastructure
  - **moshi-cli/** - Command-line interface
  - **mimi-pyo3/** - Python bindings for Mimi

## Building

Build all server components:

```bash
cd server/rust
cargo build --all-features --release
```

## Running the Server

Start the moshi-server with a configuration file:

```bash
cd server/rust/moshi
cargo run --bin moshi-server -- worker --config ../../../configs/config-tts.toml
```

Or if you've installed the binary:

```bash
moshi-server worker --config configs/config-tts.toml
```

## Testing

Run all tests:

```bash
cd server/rust
cargo test --all-features
```

Run clippy lints:

```bash
cd server/rust
cargo clippy --all-targets --all-features
```

## Documentation

For more details, see:
- [Moshi README](moshi/README.md)
- [Main project README](../../README.md)
