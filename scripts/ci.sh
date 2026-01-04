#!/usr/bin/env bash

# CI/CD pipeline script for Delayed Streams Modeling
# Designed for GitHub Actions but can be adapted for other CI systems

set -euo pipefail

# Configuration
export RUST_BACKTRACE=1
export RUST_LOG="info"
export CARGO_TERM_COLOR="always"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Pipeline stages
setup_environment() {
    log_info "Setting up environment..."

    # Install Rust components
    rustup component add rustfmt clippy rust-src

    # Install additional tools
    cargo install cargo-audit cargo-deny cargo-nextest

    # Set up caching directories
    mkdir -p ~/.cargo/registry
    mkdir -p ~/.cargo/git

    log_success "Environment setup completed"
}

security_scan() {
    log_info "Running security scans..."

    # Dependency audit
    log_info "Running cargo audit..."
    cargo audit --workspace --deny warnings

    # License check
    log_info "Running cargo deny..."
    cargo deny check workspace

    # Check for common security issues
    log_info "Scanning for security issues..."
    cargo clippy --workspace --all-targets --features cuda -- -D clippy::indexing_slicing -D clippy::unwrap_used -D clippy::panic

    log_success "Security scans completed"
}

code_quality() {
    log_info "Running code quality checks..."

    # Format check
    log_info "Checking code formatting..."
    cargo fmt --all -- --check

    # Linting
    log_info "Running clippy..."
    cargo clippy --workspace --all-targets --features cuda -- -D warnings

    # Documentation check
    log_info "Checking documentation..."
    cargo doc --workspace --features cuda --no-deps --document-private-items

    log_success "Code quality checks passed"
}

build_and_test() {
    log_info "Building and testing..."

    # Build with different profiles
    local profiles=("dev" "dev-opt" "release")

    for profile in "${profiles[@]}"; do
        log_info "Building with profile: $profile"
        cargo build --workspace --profile "$profile" --features cuda

        if [[ "$profile" == "release" ]]; then
            # Run tests in release mode for performance testing
            log_info "Running tests in release mode..."
            cargo test --workspace --profile release --features cuda -- --nocapture
        fi
    done

    # Run unit tests
    log_info "Running unit tests..."
    cargo nextest run --workspace --features cuda --profile dev-opt

    # Run integration tests
    log_info "Running integration tests..."
    cargo test --workspace --features cuda --test '*'

    log_success "Build and tests completed"
}

performance_benchmarks() {
    log_info "Running performance benchmarks..."

    # Run benchmarks if they exist
    if cargo bench --help &>/dev/null; then
        cargo bench --workspace --features cuda
    else
        log_warning "No benchmarks found, skipping"
    fi

    log_success "Performance benchmarks completed"
}

coverage_report() {
    log_info "Generating coverage report..."

    # Install cargo-llvm-cov if not present
    if ! command -v cargo-llvm-cov &> /dev/null; then
        cargo install cargo-llvm-cov
    fi

    # Generate coverage
    cargo llvm-cov --workspace --features cuda --lcov --output-path lcov.info

    # Generate HTML report
    cargo llvm-cov --workspace --features cuda --html

    log_success "Coverage report generated"
}

artifact_collection() {
    log_info "Collecting build artifacts..."

    # Create artifacts directory
    mkdir -p artifacts

    # Collect binaries
    find target/release -name "moshi*" -type f -executable -exec cp {} artifacts/ \; 2>/dev/null || true

    # Collect documentation
    if [[ -d "target/doc" ]]; then
        tar -czf artifacts/documentation.tar.gz -C target/doc .
    fi

    # Collect coverage reports
    if [[ -f "lcov.info" ]]; then
        cp lcov.info artifacts/
    fi

    if [[ -d "target/llvm-cov/html" ]]; then
        tar -czf artifacts/coverage.tar.gz -C target/llvm-cov/html .
    fi

    # Generate artifact manifest
    {
        echo "Artifact Manifest - $(date)"
        echo "=========================="
        echo "Build: $(git rev-parse HEAD)"
        echo "Branch: ${GITHUB_REF_NAME:-$(git rev-parse --abbrev-ref HEAD)}"
        echo ""
        echo "Files:"
        ls -la artifacts/
    } > artifacts/manifest.txt

    log_success "Artifacts collected"
}

deployment_validation() {
    log_info "Running deployment validation..."

    # Validate that binaries can run
    for binary in artifacts/moshi*; do
        if [[ -f "$binary" && -x "$binary" ]]; then
            log_info "Testing $binary..."
            "$binary" --version || log_warning "$binary --version failed"
        fi
    done

    # Validate configuration files
    log_info "Validating configuration files..."
    for config in configs/**/*.toml; do
        if [[ -f "$config" ]]; then
            log_info "Validating $config..."
            # Basic TOML validation
            python3 -c "import tomllib; tomllib.load(open('$config', 'rb'))" 2>/dev/null || \
            python3 -c "import toml; toml.load(open('$config'))" 2>/dev/null || \
            log_warning "Could not validate $config"
        fi
    done

    log_success "Deployment validation completed"
}

cleanup() {
    log_info "Cleaning up..."

    # Clean temporary files
    cargo clean

    # Remove intermediate build artifacts
    rm -rf target/debug
    rm -rf target/dev-opt

    log_success "Cleanup completed"
}

# Main pipeline execution
main() {
    local stage="${1:-all}"

    log_info "Starting CI/CD pipeline - Stage: $stage"

    case "$stage" in
        "setup")
            setup_environment
            ;;
        "security")
            setup_environment
            security_scan
            ;;
        "quality")
            setup_environment
            code_quality
            ;;
        "test")
            setup_environment
            build_and_test
            ;;
        "bench")
            setup_environment
            performance_benchmarks
            ;;
        "coverage")
            setup_environment
            coverage_report
            ;;
        "deploy")
            setup_environment
            build_and_test
            artifact_collection
            deployment_validation
            ;;
        "cleanup")
            cleanup
            ;;
        "all")
            setup_environment
            security_scan
            code_quality
            build_and_test
            performance_benchmarks
            coverage_report
            artifact_collection
            deployment_validation
            cleanup
            ;;
        *)
            log_error "Unknown stage: $stage"
            log_info "Available stages: setup, security, quality, test, bench, coverage, deploy, cleanup, all"
            exit 1
            ;;
    esac

    log_success "CI/CD pipeline completed successfully!"
}

# Run main function with stage argument
main "${1:-all}"
