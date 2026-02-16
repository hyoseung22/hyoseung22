#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if ! python -c "import yaml" >/dev/null 2>&1; then
  echo "[INFO] Installing dependencies from requirements.txt"
  pip install -r requirements.txt
fi

mkdir -p logs

OUT_FILE="logs/demo_run_output.txt"

echo "[INFO] Running dry-run..."
python -m band_auto_poster.main --config config.example.yaml --dry-run | tee "$OUT_FILE"

echo "[INFO] Saved output to $OUT_FILE"
