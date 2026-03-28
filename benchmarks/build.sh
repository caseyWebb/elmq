#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building elmq release binary..."
cargo build --release --locked --manifest-path "$PROJECT_ROOT/Cargo.toml"

echo "Copying binary to benchmarks directory..."
cp "$PROJECT_ROOT/target/release/elmq" "$SCRIPT_DIR/elmq"

echo "Building Docker image..."
docker build -t elmq-bench "$SCRIPT_DIR"

echo "Cleaning up..."
rm "$SCRIPT_DIR/elmq"

echo "Done. Image: elmq-bench"
echo ""
echo "To set up auth (once):"
echo "  docker run -it -v claude-auth:/root/.claude elmq-bench claude setup-token"
echo ""
echo "To run benchmarks:"
echo "  docker run -v claude-auth:/root/.claude -v \$(pwd)/benchmarks/results:/bench/results elmq-bench /bench/run.sh [control|treatment|both]"
