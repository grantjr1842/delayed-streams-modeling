#!/usr/bin/env bash

set -euo pipefail

cargo install --path ./moshi/rust/moshi-server --features cuda --verbose

moshi-server worker --config configs/config-stt-en_fr-hf.toml