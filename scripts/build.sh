#!/usr/bin/env bash

# Build optimization script for Delayed Streams Modeling
# This script provides various build configurations and optimizations

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
BUILD_PROFILE="release"
FEATURES="cuda"
TARGET_DIR="target"
PARALLEL_JOBS=$(nproc)
SKIP_TESTS=false
ENABLE_BENCH=false

# Help function
show_help() {
    cat << EOF
Build Optimization Script for Delayed Streams Modeling

USAGE:
    $0 [OPTIONS]

OPTIONS:
    -p, --profile <PROFILE>     Build profile (dev, dev-opt, release, bench) [default: release]
    -f, --features <FEATURES>   Cargo features to enable [default: cuda]
    -j, --jobs <N>              Number of parallel jobs [default: nproc]
    -t, --target-dir <DIR>     Target directory [default: target]
    --skip-tests               Skip running tests
    --bench                    Enable benchmarking
    --clean                    Clean before building
    --check                    Run cargo check only
    --dry-run                  Show commands without executing
    -h, --help                 Show this help

EXAMPLES:
    $0                                    # Standard release build
    $0 -p dev-opt --skip-tests           # Development build with optimizations
    $0 -p bench --bench                   # Benchmark build
    $0 --clean --profile release         # Clean release build

PROFILES:
    dev         - Fast compilation, basic optimizations
    dev-opt     - Development with more optimizations
    release     - Production build with maximum optimizations
    bench       - Benchmarking build with debug symbols
EOF
}

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -p|--profile)
                BUILD_PROFILE="$2"
                shift 2
                ;;
            -f|--features)
                FEATURES="$2"
                shift 2
                ;;
            -j|--jobs)
                PARALLEL_JOBS="$2"
                shift 2
                ;;
            -t|--target-dir)
                TARGET_DIR="$2"
                shift 2
                ;;
            --skip-tests)
                SKIP_TESTS=true
                shift
                ;;
            --bench)
                ENABLE_BENCH=true
                shift
                ;;
            --clean)
                CLEAN_BUILD=true
                shift
                ;;
            --check)
                CHECK_ONLY=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

# Validate arguments
validate_args() {
    local valid_profiles=("dev" "dev-opt" "release" "bench")
    if [[ ! " ${valid_profiles[@]} " =~ " ${BUILD_PROFILE} " ]]; then
        log_error "Invalid profile: $BUILD_PROFILE"
        log_error "Valid profiles: ${valid_profiles[*]}"
        exit 1
    fi

    if [[ ! "$PARALLEL_JOBS" =~ ^[0-9]+$ ]] || [[ "$PARALLEL_JOBS" -lt 1 ]]; then
        log_error "Invalid number of jobs: $PARALLEL_JOBS"
        exit 1
    fi
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo not found. Please install Rust."
        exit 1
    fi

    # Check Rust version
    local rust_version=$(rustc --version | cut -d' ' -f2)
    log_info "Rust version: $rust_version"

    # Check if CUDA is available (if using cuda features)
    if [[ "$FEATURES" == *"cuda"* ]]; then
        if command -v nvcc &> /dev/null; then
            local cuda_version=$(nvcc --version | grep "release" | awk '{print $6}' | cut -d',' -f1)
            log_info "CUDA version: $cuda_version"
        else
            log_warning "CUDA not found, but cuda features are enabled"
        fi
    fi

    # Check available memory
    local available_memory=$(free -h | awk '/^Mem:/ {print $7}')
    log_info "Available memory: $available_memory"

    # Check disk space
    local available_disk=$(df -h . | awk 'NR==2 {print $4}')
    log_info "Available disk space: $available_disk"
}

# Clean build artifacts
clean_build() {
    if [[ "${CLEAN_BUILD:-false}" == true ]]; then
        log_info "Cleaning build artifacts..."
        run_command "cargo clean"
        run_command "rm -rf $TARGET_DIR"
    fi
}

# Run cargo command with dry run support
run_command() {
    local cmd="$1"
    if [[ "${DRY_RUN:-false}" == true ]]; then
        log_info "[DRY-RUN] $cmd"
    else
        log_info "Running: $cmd"
        eval "$cmd"
    fi
}

# Build the project
build_project() {
    local build_cmd="cargo build --workspace --profile $BUILD_PROFILE --features $FEATURES --jobs $PARALLEL_JOBS"

    if [[ -n "$TARGET_DIR" && "$TARGET_DIR" != "target" ]]; then
        build_cmd="$build_cmd --target-dir $TARGET_DIR"
    fi

    log_info "Starting build with profile: $BUILD_PROFILE"
    run_command "$build_cmd"

    if [[ "${CHECK_ONLY:-false}" != true ]]; then
        log_success "Build completed successfully"
    fi
}

# Run tests
run_tests() {
    if [[ "$SKIP_TESTS" == false ]]; then
        log_info "Running tests..."
        local test_cmd="cargo test --workspace --profile $BUILD_PROFILE --features $FEATURES --jobs $PARALLEL_JOBS"

        if [[ -n "$TARGET_DIR" && "$TARGET_DIR" != "target" ]]; then
            test_cmd="$test_cmd --target-dir $TARGET_DIR"
        fi

        run_command "$test_cmd"
        log_success "All tests passed"
    else
        log_info "Skipping tests as requested"
    fi
}

# Run benchmarks
run_benchmarks() {
    if [[ "$ENABLE_BENCH" == true ]]; then
        log_info "Running benchmarks..."
        local bench_cmd="cargo bench --workspace --features $FEATURES --jobs $PARALLEL_JOBS"

        if [[ -n "$TARGET_DIR" && "$TARGET_DIR" != "target" ]]; then
            bench_cmd="$bench_cmd --target-dir $TARGET_DIR"
        fi

        run_command "$bench_cmd"
        log_success "Benchmarks completed"
    fi
}

# Run code quality checks
run_quality_checks() {
    log_info "Running code quality checks..."

    # Format check
    run_command "cargo fmt --all -- --check"

    # Clippy
    run_command "cargo clippy --workspace --all-targets --features $FEATURES -- -D warnings"

    # Audit dependencies
    run_command "cargo audit --workspace"

    log_success "Code quality checks passed"
}

# Generate build report
generate_report() {
    log_info "Generating build report..."

    local report_file="build-report-$(date +%Y%m%d-%H%M%S).txt"
    {
        echo "Build Report - Delayed Streams Modeling"
        echo "======================================="
        echo "Timestamp: $(date)"
        echo "Profile: $BUILD_PROFILE"
        echo "Features: $FEATURES"
        echo "Jobs: $PARALLEL_JOBS"
        echo "Target Directory: $TARGET_DIR"
        echo ""
        echo "System Information:"
        echo "OS: $(uname -s)"
        echo "Kernel: $(uname -r)"
        echo "Architecture: $(uname -m)"
        echo "CPU Cores: $(nproc)"
        echo "Memory: $(free -h | awk '/^Mem:/ {print $2}')"
        echo ""
        echo "Rust Information:"
        echo "Version: $(rustc --version)"
        echo "Cargo Version: $(cargo --version)"
        echo ""
        if command -v nvcc &> /dev/null; then
            echo "CUDA Information:"
            echo "Version: $(nvcc --version | grep "release" | awk '{print $6}' | cut -d',' -f1)"
            echo ""
        fi
        echo "Build Artifacts:"
        if [[ -d "$TARGET_DIR/$BUILD_PROFILE" ]]; then
            find "$TARGET_DIR/$BUILD_PROFILE" -name "moshi*" -type f -exec ls -lh {} \; 2>/dev/null || true
        fi
    } > "$report_file"

    log_success "Build report saved to: $report_file"
}

# Main execution
main() {
    log_info "Starting build optimization script..."

    parse_args "$@"
    validate_args
    check_prerequisites
    clean_build

    if [[ "${CHECK_ONLY:-false}" == true ]]; then
        run_command "cargo check --workspace --features $FEATURES --jobs $PARALLEL_JOBS"
        run_quality_checks
    else
        build_project
        run_tests
        run_benchmarks
        run_quality_checks
    fi

    generate_report

    log_success "Build optimization completed successfully!"
}

# Run main function with all arguments
main "$@"
