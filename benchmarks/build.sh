#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building Docker image (includes Rust compilation)..."
docker build -t elmq-bench -f "$SCRIPT_DIR/Dockerfile" "$PROJECT_ROOT"

echo "Done. Image: elmq-bench"
echo ""
echo "To run benchmarks:"
echo "  docker run --env-file benchmarks/.env -v \$(pwd)/benchmarks/results:/bench/results elmq-bench /bench/run.sh [control|treatment|both]"
