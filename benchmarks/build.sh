#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building Docker image (includes Rust compilation)..."
docker build -t elmq-bench -f "$SCRIPT_DIR/Dockerfile" "$PROJECT_ROOT"

echo "Done. Image: elmq-bench"
echo ""
echo "To set up auth (once):"
echo "  docker run -it -v claude-auth:/root/.claude elmq-bench claude setup-token"
echo ""
echo "To run benchmarks:"
echo "  docker run -v claude-auth:/root/.claude -v \$(pwd)/benchmarks/results:/bench/results elmq-bench /bench/run.sh [control|treatment|both]"
