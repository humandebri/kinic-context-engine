#!/usr/bin/env bash
set -euo pipefail

# Where: scripts/setup_local_embedding.sh
# What: Validate the local multilingual-e5-large directory layout expected by kinic-embed.
# Why: Keep model installation out of git while giving operators one deterministic setup check.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODEL_DIR="${KINIC_CONTEXT_EMBEDDING_MODEL_DIR:-$ROOT_DIR/.local/models/multilingual-e5-large}"

echo "Checking local embedding assets in: $MODEL_DIR"

if [[ ! -d "$MODEL_DIR" ]]; then
  echo "Missing model directory: $MODEL_DIR" >&2
  exit 1
fi

if [[ ! -f "$MODEL_DIR/tokenizer.json" ]]; then
  echo "Missing tokenizer.json in $MODEL_DIR" >&2
  exit 1
fi

if [[ ! -f "$MODEL_DIR/config.json" ]]; then
  echo "Missing config.json in $MODEL_DIR" >&2
  exit 1
fi

if [[ -f "$MODEL_DIR/onnx/model.onnx" ]]; then
  ONNX_FILE="$MODEL_DIR/onnx/model.onnx"
else
  ONNX_FILE="$(find "$MODEL_DIR/onnx" -maxdepth 1 -name '*.onnx' | head -n 1 || true)"
fi

if [[ -z "$ONNX_FILE" ]]; then
  echo "Missing ONNX file under $MODEL_DIR/onnx" >&2
  exit 1
fi

echo "Found ONNX model: $ONNX_FILE"
echo "Local embedding assets look ready."
