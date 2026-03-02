#!/usr/bin/env bash
# Run high-load benchmark in Docker.
# Usage: run_docker_bench.sh [multiplier]
#   multiplier: data duplication factor (default: 10)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
SESSIONS_DIR="${SESSIONS_DIR:-$HOME/src/sessions}"
MUL="${1:-10}"

TMPDIR=$(mktemp -d)
trap "echo 'Cleaning up...'; rm -rf $TMPDIR" EXIT

BINARIES_DIR="$TMPDIR/binaries"
DATA_DIR="$TMPDIR/data"
mkdir -p "$BINARIES_DIR"

echo "=== Collecting binaries ==="
cp "$PROJECT_DIR/target/release/ase" "$BINARIES_DIR/" 2>/dev/null && echo "  ase" || echo "  ase: not found (run cargo build --release)"
cp "$SESSIONS_DIR/cass/target/release/cass" "$BINARIES_DIR/" 2>/dev/null && echo "  cass" || echo "  cass: not found"
cp "$SESSIONS_DIR/cc-sessions/target/release/cc-sessions" "$BINARIES_DIR/" 2>/dev/null && echo "  cc-sessions" || echo "  cc-sessions: not found"
cp "$SESSIONS_DIR/ccrider/ccrider" "$BINARIES_DIR/" 2>/dev/null && echo "  ccrider" || echo "  ccrider: not found"
cp "$SESSIONS_DIR/ccsearch/target/release/ccsearch" "$BINARIES_DIR/" 2>/dev/null && echo "  ccsearch" || echo "  ccsearch: not found"

echo ""
echo "=== Generating ${MUL}x load data ==="
bash "$SCRIPT_DIR/gen_load.sh" "$HOME/.claude/projects" "$DATA_DIR" "$MUL"

echo ""
echo "=== Building Docker image ==="
cp "$SCRIPT_DIR/Dockerfile" "$TMPDIR/"
cp "$SCRIPT_DIR/docker_bench.sh" "$TMPDIR/"
docker build -t ase-bench "$TMPDIR"

echo ""
echo "=== Running benchmark in Docker ==="
docker run --rm \
    -v "$DATA_DIR:/home/bench/.claude/projects:ro" \
    ase-bench
