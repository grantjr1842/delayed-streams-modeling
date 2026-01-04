#!/usr/bin/env bash

# Developer setup script for Delayed Streams Modeling
# This script sets up a complete development environment

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
PROJECT_NAME="Delayed Streams Modeling"
MIN_RUST_VERSION="1.70.0"
REQUIRED_TOOLS=("git" "cargo" "rustc")
OPTIONAL_TOOLS=("nvcc" "python3" "node" "npm")

# ASCII Art Banner
show_banner() {
    echo -e "${CYAN}"
    cat << "EOF"
 _____ _   _ _   _    _    _   _  ____ _____ ____
| ____| \ | | | | |  / \  | \ | |/ ___| ____|  _ \
|  _| |  \| | |_| | / _ \ |  \| | |   |  _| | | | |
| |___| |\  |  _  |/ ___ \| |\  | |___| |___| |_| |
|_____|_| \_|_| |_/_/   \_\_| \_|\____|_____|____/
              _____ _     _
             | ____| |   | |
             |  _| | |   | |
             | |___| |___| |___
             |_____|_____|_____|

    Developer Environment Setup Script
EOF
    echo -e "${NC}"
}

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${PURPLE}[STEP]${NC} $1"; }

# Help function
show_help() {
    cat << EOF
Developer Setup Script for $PROJECT_NAME

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --skip-system-checks    Skip system requirement checks
    --no-git-hooks         Skip git hooks installation
    --dev-tools-only       Only install development tools
    --docker               Set up for Docker environment
    --help                 Show this help

EXAMPLES:
    $0                      # Full setup
    $0 --skip-system-checks # Skip system checks
    $0 --dev-tools-only     # Development tools only

This script will:
1. Check system requirements
2. Install Rust toolchain
3. Set up development tools
4. Configure git hooks
5. Install optional dependencies
6. Verify the setup
EOF
}

# Parse command line arguments
SKIP_SYSTEM_CHECKS=false
SKIP_GIT_HOOKS=false
DEV_TOOLS_ONLY=false
DOCKER_ENV=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-system-checks)
            SKIP_SYSTEM_CHECKS=true
            shift
            ;;
        --no-git-hooks)
            SKIP_GIT_HOOKS=true
            shift
            ;;
        --dev-tools-only)
            DEV_TOOLS_ONLY=true
            shift
            ;;
        --docker)
            DOCKER_ENV=true
            shift
            ;;
        --help)
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

# Check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Check version requirement
check_version() {
    local cmd="$1"
    local min_version="$2"
    local current_version

    if ! command_exists "$cmd"; then
        return 1
    fi

    case "$cmd" in
        rustc)
            current_version=$(rustc --version | cut -d' ' -f2)
            ;;
        cargo)
            current_version=$(cargo --version | cut -d' ' -f2)
            ;;
        *)
            current_version=$("$cmd" --version 2>/dev/null | head -n1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' | head -n1)
            ;;
    esac

    if [[ -z "$current_version" ]]; then
        return 1
    fi

    # Simple version comparison
    if printf '%s\n%s\n' "$min_version" "$current_version" | sort -V | head -n1 | grep -q "^$min_version$"; then
        return 0
    else
        return 1
    fi
}

# System requirements check
check_system_requirements() {
    log_step "Checking system requirements..."

    # Check operating system
    local os=$(uname -s)
    log_info "Operating System: $os"

    # Check architecture
    local arch=$(uname -m)
    log_info "Architecture: $arch"

    # Check required tools
    local missing_tools=()
    for tool in "${REQUIRED_TOOLS[@]}"; do
        if command_exists "$tool"; then
            local version=$("$tool" --version 2>/dev/null | head -n1)
            log_info "✓ $tool: $version"
        else
            log_error "✗ $tool: not found"
            missing_tools+=("$tool")
        fi
    done

    # Check Rust version
    if command_exists rustc; then
        if check_version rustc "$MIN_RUST_VERSION"; then
            log_success "✓ Rust version meets requirements"
        else
            log_error "✗ Rust version too old. Minimum required: $MIN_RUST_VERSION"
            missing_tools+=("rustc>=${MIN_RUST_VERSION}")
        fi
    fi

    # Check optional tools
    log_info "Checking optional tools..."
    for tool in "${OPTIONAL_TOOLS[@]}"; do
        if command_exists "$tool"; then
            local version=$("$tool" --version 2>/dev/null | head -n1 || echo "unknown")
            log_info "✓ $tool: $version"
        else
            log_warning "⚠ $tool: not found (optional)"
        fi
    done

    # Check system resources
    local memory_gb=$(free -g | awk '/^Mem:/ {print $2}')
    local cpu_cores=$(nproc)
    local disk_space=$(df -BG . | awk 'NR==2 {print $4}' | sed 's/G//')

    log_info "System Resources:"
    log_info "  Memory: ${memory_gb}GB"
    log_info "  CPU Cores: $cpu_cores"
    log_info "  Available Disk: ${disk_space}GB"

    # Minimum requirements
    if [[ $memory_gb -lt 4 ]]; then
        log_warning "⚠ Low memory detected. Recommended: 8GB+"
    fi

    if [[ $disk_space -lt 10 ]]; then
        log_warning "⚠ Low disk space. Recommended: 20GB+"
    fi

    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install missing tools and run the script again."
        exit 1
    fi

    log_success "System requirements check passed"
}

# Install Rust toolchain
install_rust_toolchain() {
    log_step "Setting up Rust toolchain..."

    # Install required components
    log_info "Installing Rust components..."
    rustup component add rustfmt clippy rust-src llvm-tools-preview

    # Install useful tools
    log_info "Installing Rust development tools..."
    local rust_tools=(
        "cargo-watch"      # Watch for file changes and rebuild
        "cargo-audit"      # Security audit
        "cargo-deny"       # License/dependency checker
        "cargo-expand"     # Macro expansion
        "cargo-flamegraph"  # Performance profiling
        "cargo-nextest"    # Improved test runner
        "cargo-criterion"  # Benchmarking
        "cargo-llvm-cov"   # Code coverage
        "cargo-bloat"      # Dependency size analysis
        "cargo-tree"       # Dependency tree visualization
    )

    for tool in "${rust_tools[@]}"; do
        if ! command_exists "$tool"; then
            log_info "Installing $tool..."
            cargo install "$tool" --locked
        else
            log_info "✓ $tool already installed"
        fi
    done

    log_success "Rust toolchain setup completed"
}

# Set up development environment
setup_dev_environment() {
    log_step "Setting up development environment..."

    # Create development directories
    local dev_dirs=("logs" "tmp" "data/models" "data/audio")
    for dir in "${dev_dirs[@]}"; do
        if [[ ! -d "$dir" ]]; then
            mkdir -p "$dir"
            log_info "Created directory: $dir"
        fi
    done

    # Set up environment file
    if [[ ! -f ".env" ]]; then
        if [[ -f ".env.example" ]]; then
            cp .env.example .env
            log_info "Created .env from .env.example"
            log_warning "Please edit .env with your configuration"
        else
            cat > .env << EOF
# Development Environment Configuration
RUST_LOG=debug
RUST_BACKTRACE=1
CUDA_VISIBLE_DEVICES=0
MODEL_CACHE_DIR=./data/models
AUDIO_CACHE_DIR=./data/audio
EOF
            log_info "Created default .env file"
        fi
    fi

    # Create VS Code configuration
    if [[ ! -d ".vscode" ]]; then
        mkdir -p .vscode
        cat > .vscode/settings.json << EOF
{
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.cargo.features": "cuda",
    "rust-analyzer.cargo.loadOutDirsFromCheck": true,
    "rust-analyzer.procMacro.enable": true,
    "files.watcherExclude": {
        "**/target/**": true
    },
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
        "source.fixAll": true
    }
}
EOF

        cat > .vscode/launch.json << EOF
{
    "version": "0.2.0",
    "configurations": [
        {
            "name": "Debug moshi-server",
            "type": "lldb",
            "request": "launch",
            "program": "\${workspaceFolder}/target/debug/moshi-server",
            "args": ["--config", "configs/stt/config-stt-en_fr-hf.toml"],
            "cwd": "\${workspaceFolder}",
            "environment": [
                { "name": "RUST_LOG", "value": "debug" }
            ]
        }
    ]
}
EOF

        cat > .vscode/tasks.json << EOF
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "cargo build",
            "type": "shell",
            "command": "cargo",
            "args": ["build", "--features", "cuda"],
            "group": {
                "kind": "build",
                "isDefault": true
            },
            "problemMatcher": ["\$rustc"]
        },
        {
            "label": "cargo test",
            "type": "shell",
            "command": "cargo",
            "args": ["test", "--features", "cuda"],
            "group": {
                "kind": "test",
                "isDefault": true
            },
            "problemMatcher": ["\$rustc"]
        },
        {
            "label": "cargo clippy",
            "type": "shell",
            "command": "cargo",
            "args": ["clippy", "--features", "cuda", "--", "-D", "warnings"],
            "group": "build",
            "problemMatcher": ["\$rustc"]
        }
    ]
}
EOF

        log_info "Created VS Code configuration"
    fi

    log_success "Development environment setup completed"
}

# Set up git hooks
setup_git_hooks() {
    if [[ "$SKIP_GIT_HOOKS" == true ]]; then
        log_info "Skipping git hooks setup"
        return
    fi

    log_step "Setting up git hooks..."

    # Create pre-commit hook
    cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
# Pre-commit hook for Delayed Streams Modeling

set -euo pipefail

echo "Running pre-commit checks..."

# Format check
echo "Checking code formatting..."
if ! cargo fmt --all -- --check; then
    echo "Code formatting issues found. Run 'cargo fmt' to fix."
    exit 1
fi

# Clippy check
echo "Running clippy..."
if ! cargo clippy --workspace --all-targets --features cuda -- -D warnings; then
    echo "Clippy found issues. Please fix them before committing."
    exit 1
fi

# Run tests
echo "Running tests..."
if ! cargo test --workspace --features cuda; then
    echo "Tests failed. Please fix them before committing."
    exit 1
fi

echo "Pre-commit checks passed!"
EOF

    # Create pre-push hook
    cat > .git/hooks/pre-push << 'EOF'
#!/bin/bash
# Pre-push hook for Delayed Streams Modeling

set -euo pipefail

echo "Running pre-push checks..."

# Security audit
echo "Running security audit..."
if ! cargo audit --workspace; then
    echo "Security audit failed. Please review vulnerabilities."
    exit 1
fi

# Build in release mode
echo "Building in release mode..."
if ! cargo build --workspace --release --features cuda; then
    echo "Release build failed."
    exit 1
fi

echo "Pre-push checks passed!"
EOF

    # Make hooks executable
    chmod +x .git/hooks/pre-commit
    chmod +x .git/hooks/pre-push

    log_success "Git hooks setup completed"
}

# Install optional dependencies
install_optional_deps() {
    log_step "Installing optional dependencies..."

    # Python dependencies (for some tools)
    if command_exists python3; then
        log_info "Setting up Python environment..."

        # Create virtual environment if it doesn't exist
        if [[ ! -d "venv" ]]; then
            python3 -m venv venv
            log_info "Created Python virtual environment"
        fi

        # Activate and install packages
        source venv/bin/activate
        pip install --upgrade pip

        local python_packages=(
            "numpy"           # For audio processing
            "matplotlib"      # For plotting
            "jupyter"         # For notebooks
            "black"           # Python formatter
            "flake8"          # Python linter
        )

        for package in "${python_packages[@]}"; do
            if ! pip show "$package" &>/dev/null; then
                log_info "Installing Python package: $package"
                pip install "$package"
            else
                log_info "✓ Python package $package already installed"
            fi
        done

        deactivate
    fi

    # Node.js dependencies (for TypeScript components)
    if command_exists npm && [[ -d "server/typescript" ]]; then
        log_info "Setting up Node.js environment..."
        cd server/typescript
        if [[ -f "package.json" ]]; then
            npm install
            log_info "Installed Node.js dependencies"
        fi
        cd - >/dev/null
    fi

    log_success "Optional dependencies installation completed"
}

# Verify setup
verify_setup() {
    log_step "Verifying setup..."

    # Test basic cargo commands
    log_info "Testing cargo commands..."
    cargo --version
    rustc --version

    # Test build
    log_info "Testing project build..."
    if cargo check --workspace --features cuda; then
        log_success "✓ Project builds successfully"
    else
        log_error "✗ Project build failed"
        return 1
    fi

    # Test tools
    log_info "Testing development tools..."
    local tools_tested=0
    local tools_passed=0

    for tool in cargo-watch cargo-audit cargo-deny; do
        ((tools_tested++))
        if command_exists "$tool"; then
            ((tools_passed++))
            log_info "✓ $tool available"
        else
            log_warning "⚠ $tool not available"
        fi
    done

    log_info "Tools tested: $tools_passed/$tools_tested"

    # Generate setup report
    local report_file="setup-report-$(date +%Y%m%d-%H%M%S).txt"
    {
        echo "Developer Setup Report - $PROJECT_NAME"
        echo "======================================="
        echo "Timestamp: $(date)"
        echo "User: $(whoami)"
        echo "System: $(uname -s) $(uname -r)"
        echo "Architecture: $(uname -m)"
        echo ""
        echo "Rust Toolchain:"
        echo "  Rustc: $(rustc --version)"
        echo "  Cargo: $(cargo --version)"
        echo ""
        echo "Project Status:"
        echo "  Build: ✓ Passed"
        echo "  Tools: $tools_passed/$tools_tested available"
        echo ""
        echo "Next Steps:"
        echo "  1. Review and edit .env file"
        echo "  2. Download required models"
        echo "  3. Run 'cargo test' to verify everything works"
        echo "  4. Start development with 'cargo watch -x run'"
    } > "$report_file"

    log_success "Setup verification completed"
    log_info "Report saved to: $report_file"
}

# Show next steps
show_next_steps() {
    log_step "Setup completed! Next steps:"

    echo
    echo -e "${CYAN}1. Configure Environment:${NC}"
    echo "   Edit .env file with your settings"
    echo "   Download required models to data/models/"
    echo
    echo -e "${CYAN}2. Development Commands:${NC}"
    echo "   cargo build                    # Build project"
    echo "   cargo test                     # Run tests"
    echo "   cargo run --bin moshi-server   # Run server"
    echo "   cargo watch -x run            # Auto-reload development"
    echo "   cargo clippy                   # Lint code"
    echo "   cargo fmt                      # Format code"
    echo
    echo -e "${CYAN}3. Useful Scripts:${NC}"
    echo "   ./scripts/build.sh             # Optimized builds"
    echo "   ./scripts/ci.sh                # CI/CD pipeline"
    echo
    echo -e "${CYAN}4. Documentation:${NC}"
    echo "   docs/DEVELOPMENT.md            # Development guide"
    echo "   docs/SECURITY.md               # Security policy"
    echo "   README.md                      # Project overview"
    echo
    echo -e "${GREEN}Happy coding! 🚀${NC}"
}

# Main execution
main() {
    show_banner

    if [[ "$SKIP_SYSTEM_CHECKS" != true ]]; then
        check_system_requirements
    fi

    install_rust_toolchain

    if [[ "$DEV_TOOLS_ONLY" != true ]]; then
        setup_dev_environment
        setup_git_hooks
        install_optional_deps
    fi

    verify_setup
    show_next_steps
}

# Error handling
trap 'log_error "Script failed at line $LINENO"' ERR

# Run main function
main "$@"
