# Development Guide

This document provides comprehensive guidance for developers working on the Delayed Streams Modeling project.

## Architecture Overview

The project consists of several key components:

- **Server Components** (`server/`): Rust-based backend services
  - `moshi/`: Core server implementation with ASR and TTS capabilities
  - `auth-server/`: TypeScript authentication service
- **Client Components** (`client/`): Rust client libraries and CLI tools
- **Tools** (`tools/`): Development and deployment utilities
- **Configs** (`configs/`): Configuration files for different deployment scenarios

## Development Workflow

### 1. Environment Setup

```bash
# Install Rust with CUDA support
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy

# Clone and setup
git clone https://github.com/kyutai-labs/delayed-streams-modeling.git
cd delayed-streams-modeling
cargo build --workspace
```

### 2. Code Quality Standards

#### Rust Code
- Use `cargo fmt` for consistent formatting
- Address all clippy warnings: `cargo clippy --workspace --all-targets -- -D warnings`
- Write comprehensive tests with `cargo test`
- Use proper error handling - avoid `unwrap()` and `expect()` in production code
- Document public APIs with rustdoc comments

#### Error Handling Pattern
```rust
// Good: Proper error handling
match some_operation() {
    Ok(result) => process(result),
    Err(e) => {
        tracing::error!("Operation failed: {}", e);
        return Err(e.into());
    }
}

// Avoid: unwrap() in production code
let result = some_operation().unwrap(); // Don't do this
```

### 3. Performance Guidelines

#### Memory Management
- Avoid unnecessary clones - use references where possible
- Use `Arc<Mutex<T>>` for shared state across threads
- Consider using `Cow<str>` for string data that might be borrowed
- Profile memory usage with tools like `valgrind` or `heaptrack`

#### Async Programming
- Use `tokio` for async operations
- Avoid blocking operations in async contexts
- Use `tokio::spawn` for concurrent tasks
- Handle async errors properly with `?` operator

### 4. Testing Strategy

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_functionality() {
        // Test implementation
        assert_eq!(result, expected);
    }
}
```

#### Integration Tests
- Test component interactions in `tests/` directory
- Use mock services for external dependencies
- Test error scenarios and edge cases

### 5. Build Optimization

The project includes several build profiles:

- `dev`: Fast compilation with basic optimizations
- `dev-opt`: Development with more optimizations
- `release`: Production build with maximum optimizations
- `bench`: Benchmarking build with debug symbols

Use appropriate profile for your use case:
```bash
cargo build --profile dev-opt  # Development with optimizations
cargo build --release          # Production build
```

## Common Issues and Solutions

### CUDA Compilation
If you encounter CUDA-related compilation errors:
```bash
# Ensure CUDA is installed and in PATH
export CUDA_PATH=/usr/local/cuda
cargo build --features cuda
```

### Memory Issues
For large models, monitor memory usage:
```bash
# Use memory profiling tools
cargo run --bin moshi-server --features cuda | \
    grep -i "memory\|gpu"
```

### Performance Profiling
Use built-in profiling:
```bash
# Enable tracing
RUST_LOG=debug cargo run --bin moshi-server

# Use flamegraph for profiling
cargo install flamegraph
cargo flamegraph --bin moshi-server
```

## Contributing Guidelines

### Before Submitting PRs
1. Run full test suite: `cargo test --workspace`
2. Check formatting: `cargo fmt --check`
3. Run clippy: `cargo clippy --workspace --all-targets -- -D warnings`
4. Update documentation if API changes were made
5. Add tests for new functionality

### Code Review Process
- All PRs require code review
- Focus on correctness, performance, and maintainability
- Ensure error handling is robust
- Verify that documentation is accurate

## Security Considerations

- Validate all external inputs
- Use secure authentication mechanisms
- Follow principle of least privilege
- Keep dependencies updated
- Use secure coding practices

## Deployment

### Production Deployment
1. Use release build: `cargo build --release`
2. Configure appropriate logging levels
3. Set up monitoring and alerting
4. Use environment variables for configuration
5. Implement proper backup and recovery procedures

### Configuration Management
- Use TOML configuration files in `configs/`
- Environment-specific settings
- Secure handling of secrets and API keys
- Validation of configuration parameters

## Troubleshooting

### Common Build Issues
- **CUDA errors**: Verify CUDA installation and PATH
- **Memory errors**: Check system resources and model size
- **Dependency conflicts**: Update Cargo.lock if needed

### Runtime Issues
- **Performance**: Check GPU utilization and memory usage
- **Connection issues**: Verify network configuration
- **Model loading**: Check model file paths and permissions

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs/tokio/tutorial)
- [Candle ML Framework](https://github.com/huggingface/candle)
- [Project Issues](https://github.com/kyutai-labs/delayed-streams-modeling/issues)
