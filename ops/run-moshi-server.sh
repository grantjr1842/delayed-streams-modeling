#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

cargo install --path "${REPO_ROOT}/server/rust/moshi/moshi-server" --features cuda --verbose

moshi-server worker --config configs/config-stt-en_fr-hf.toml
