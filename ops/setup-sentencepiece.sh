#!/usr/bin/env bash
set -euo pipefail

SENTENCEPIECE_SYS_VERSION="${SENTENCEPIECE_SYS_VERSION:-0.12.0}"
SENTENCEPIECE_PREFIX="${SENTENCEPIECE_PREFIX:-$HOME/.local/sentencepiece}"
SENTENCEPIECE_BUILD="${SENTENCEPIECE_BUILD:-$HOME/.local/sentencepiece-build}"

if ! command -v cmake >/dev/null 2>&1; then
  echo "cmake is required." >&2
  exit 1
fi

if ! command -v pkg-config >/dev/null 2>&1; then
  echo "pkg-config is required." >&2
  exit 1
fi

if [[ -n "${SENTENCEPIECE_SRC:-}" ]]; then
  SOURCE_DIR="$SENTENCEPIECE_SRC"
else
  CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
  REGISTRY_SRC="$CARGO_HOME/registry/src"
  if [[ ! -d "$REGISTRY_SRC" ]]; then
    echo "Cargo registry not found at $REGISTRY_SRC. Run 'cargo fetch' first or set SENTENCEPIECE_SRC." >&2
    exit 1
  fi
  SOURCE_DIR=$(ls -d "$REGISTRY_SRC"/*/sentencepiece-sys-"$SENTENCEPIECE_SYS_VERSION"/source 2>/dev/null | head -n 1 || true)
  if [[ -z "$SOURCE_DIR" ]]; then
    echo "sentencepiece-sys source not found in cargo registry. Set SENTENCEPIECE_SRC to the source directory." >&2
    exit 1
  fi
fi

rm -rf "$SENTENCEPIECE_BUILD"
cmake \
  -S "$SOURCE_DIR" \
  -B "$SENTENCEPIECE_BUILD" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$SENTENCEPIECE_PREFIX" \
  -DSPM_ENABLE_SHARED=ON

cmake --build "$SENTENCEPIECE_BUILD" --target install --config Release

PKGCONFIG_DIR="$SENTENCEPIECE_PREFIX/lib/pkgconfig"
mkdir -p "$PKGCONFIG_DIR"
if [[ ! -f "$PKGCONFIG_DIR/protobuf-lite.pc" ]]; then
  cat > "$PKGCONFIG_DIR/protobuf-lite.pc" <<PC
prefix=${SENTENCEPIECE_PREFIX}
exec_prefix=\${prefix}
libdir=\${exec_prefix}/lib
includedir=\${prefix}/include

Name: protobuf-lite
Description: Bundled protobuf-lite from sentencepiece
Version: 0
Libs:
Cflags:
PC
fi

cat <<EOF
SentencePiece installed to: $SENTENCEPIECE_PREFIX

Build env:
  export PKG_CONFIG_PATH="$SENTENCEPIECE_PREFIX/lib/pkgconfig:\$PKG_CONFIG_PATH"

Runtime env (if needed):
  export LD_LIBRARY_PATH="$SENTENCEPIECE_PREFIX/lib:\$LD_LIBRARY_PATH"
EOF
