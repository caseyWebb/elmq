#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building Docker image (includes Rust compilation)..."
docker build -t elmq-bench -f "$SCRIPT_DIR/Dockerfile" "$PROJECT_ROOT"

echo "Done. Image: elmq-bench"
echo ""
echo "To run benchmarks:"
echo "  ./benchmark.sh              # 1 control + 1 treatment in parallel"
echo "  ./benchmark.sh -n 3         # 3 of each (6 parallel runs)"
echo ""
echo "(./benchmark.sh auto-rebuilds the image on each invocation; docker's"
echo " layer cache makes it fast when sources are unchanged.)"
