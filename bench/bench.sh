#!/usr/bin/env bash
# Benchmark: ase (Rust) vs fr (Python)
set -euo pipefail

FR_RS="${BASH_SOURCE[0]%/*}/../target/release/ase"
FR_PY="fr"

if ! command -v hyperfine &>/dev/null; then
    echo "hyperfine not found. Install: paru -S hyperfine"
    exit 1
fi

echo "=== List sessions (warm index) ==="
hyperfine --warmup 2 --min-runs 10 \
  "$FR_PY --no-tui --list 2>/dev/null" \
  "$FR_RS --list 2>/dev/null"

echo ""
echo "=== Search: 'niri' ==="
hyperfine --warmup 2 --min-runs 10 \
  "$FR_PY --no-tui 'niri' 2>/dev/null" \
  "$FR_RS --list 'niri' 2>/dev/null"

echo ""
echo "=== Cold index rebuild ==="
hyperfine --warmup 0 --min-runs 3 \
  --prepare "rm -rf ~/.cache/agents-sesame/tantivy_index" \
  "$FR_PY --rebuild --no-tui --list 2>/dev/null" \
  --prepare "rm -rf ~/.cache/agents-sesame/tantivy_index_rs" \
  "$FR_RS --rebuild --list 2>/dev/null"

echo ""
echo "=== TUI startup + quit ==="
hyperfine --warmup 1 --min-runs 5 \
  "echo -ne '\x1b' | $FR_PY 2>/dev/null || true" \
  "echo -ne '\x1b' | $FR_RS 2>/dev/null || true"
