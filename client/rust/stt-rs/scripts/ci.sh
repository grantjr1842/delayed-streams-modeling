#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all --check --verbose
cargo clippy --all-targets --all-features --verbose --verbose
cargo test --all-features --verbose --verbose
