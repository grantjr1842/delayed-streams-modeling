#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

if [[ -x "${REPO_ROOT}/.venv/bin/python" ]]; then
  PYO3_PYTHON_BIN="${REPO_ROOT}/.venv/bin/python"
else
  PYO3_PYTHON_BIN="$(command -v python3 || true)"
fi

if [[ -z "${PYO3_PYTHON_BIN}" ]]; then
  echo "python3 not found on PATH and ${REPO_ROOT}/.venv/bin/python does not exist" >&2
  exit 1
fi

env -u VIRTUAL_ENV -u CONDA_PREFIX -u PYO3_CONFIG_FILE PYO3_PYTHON="${PYO3_PYTHON_BIN}" \
  cargo install --path "${REPO_ROOT}/server/rust/moshi/moshi-server" --features cuda --verbose

moshi-server worker --config configs/config-stt-en_fr-hf.toml