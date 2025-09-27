#!/bin/bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
PYHOME_VENV="$ROOT/tts-venv/.venv"
PYBIN_VENV="$PYHOME_VENV/bin/python"
if [ ! -x "$PYBIN_VENV" ]; then
  echo "Python binary not found at $PYBIN_VENV" >&2
  exit 1
fi
PYTHON_BASE_BIN=$(uv python find 3.12.8)
PYTHON_BASE_HOME=$(cd "$(dirname "$PYTHON_BASE_BIN")/.." && pwd)
LD_PATH=$("$PYTHON_BASE_BIN" -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))')
export LD_LIBRARY_PATH="$LD_PATH:${LD_LIBRARY_PATH-}"
export PYTHONHOME="$PYTHON_BASE_HOME"
SITE_PACKAGES="$PYHOME_VENV/lib/python3.12/site-packages"
export PYTHONPATH="$SITE_PACKAGES"
cd "$ROOT/tts-venv"
uv run --locked moshi-server worker --cpu --config ../configs/config-tts.toml --port 8081
