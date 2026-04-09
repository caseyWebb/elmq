#!/usr/bin/env bash
# benchmark.sh — parallel benchmark runner wrapper.
# Launches N copies of each selected arm (control and/or treatment) in
# parallel, each in its own Docker container with a unique results dir.
#
# Usage:
#   ./benchmark.sh                    # 1 control + 1 treatment in parallel
#   ./benchmark.sh -n 3               # 3 control + 3 treatment (6 total)
#   ./benchmark.sh control            # 1 control only
#   ./benchmark.sh treatment -n 5     # 5 treatments in parallel

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
BENCH_DIR="$REPO_ROOT/benchmarks"
RESULTS_DIR="$BENCH_DIR/results"
LOGS_DIR="$RESULTS_DIR/logs"
ENV_FILE="$BENCH_DIR/.env"

usage() {
    cat <<'USAGE' >&2
Usage: ./benchmark.sh [control|treatment] [-n N]

Arguments:
  control|treatment  Optional. If omitted, runs both arms.
  -n N               Optional. Number of runs per arm. Default: 1.

Examples:
  ./benchmark.sh                    # 1 control + 1 treatment
  ./benchmark.sh -n 3               # 3 of each (6 parallel runs)
  ./benchmark.sh control            # 1 control only
  ./benchmark.sh treatment -n 5     # 5 treatments in parallel
USAGE
}

ARM=""
N=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        control|treatment)
            if [[ -n "$ARM" ]]; then
                echo "Error: arm specified twice ($ARM and $1)" >&2
                usage
                exit 1
            fi
            ARM="$1"
            shift
            ;;
        -n)
            if [[ $# -lt 2 ]]; then
                echo "Error: -n requires a value" >&2
                usage
                exit 1
            fi
            N="$2"
            if ! [[ "$N" =~ ^[1-9][0-9]*$ ]]; then
                echo "Error: -n must be a positive integer (got: $N)" >&2
                exit 1
            fi
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Error: unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

# Determine which arms to run
if [[ -n "$ARM" ]]; then
    ARMS=("$ARM")
else
    ARMS=(control treatment)
fi

# Pre-flight: credentials
if [[ ! -f "$ENV_FILE" ]]; then
    echo "Error: $ENV_FILE does not exist." >&2
    echo "Create it with: CLAUDE_CODE_OAUTH_TOKEN=your-token-here" >&2
    echo "(get a token via 'claude setup-token')" >&2
    exit 1
fi

# Build (or rebuild) the Docker image. Docker's layer cache makes this
# cheap when sources are unchanged: the Rust compilation layer and the
# benchmarks/* COPY layers all short-circuit if their inputs haven't
# changed. When sources *have* changed, this ensures parallel runs use
# the latest elmq binary and guide content.
echo "Building elmq-bench image (cached layers will be reused)..."
if ! "$REPO_ROOT/benchmarks/build.sh" >/dev/null; then
    echo "Error: ./benchmarks/build.sh failed. Re-run it directly to see the error output." >&2
    exit 1
fi

mkdir -p "$LOGS_DIR"

TIMESTAMP_BASE="$(date -u +%Y-%m-%dT%H:%M:%S)"
total_runs=$(( ${#ARMS[@]} * N ))

echo "Launching $total_runs benchmark run(s) in parallel:"
echo "  arms:    ${ARMS[*]}"
echo "  N:       $N per arm"
echo "  batch:   $TIMESTAMP_BASE"
echo "  logs:    $LOGS_DIR/"
echo ""

declare -a PIDS
declare -a RUN_IDS

for arm in "${ARMS[@]}"; do
    for ((i = 1; i <= N; i++)); do
        run_id="${TIMESTAMP_BASE}-${arm}-${i}"
        log_file="$LOGS_DIR/${run_id}.log"
        echo "  → Started $arm run #$i  →  $log_file"

        docker run --rm \
            --env-file "$ENV_FILE" \
            -e "BENCHMARK_RUN_ID=$run_id" \
            -v "$RESULTS_DIR:/bench/results" \
            elmq-bench /bench/run.sh "$arm" >"$log_file" 2>&1 &

        PIDS+=("$!")
        RUN_IDS+=("$run_id")
    done
done

echo ""
echo "Waiting for all $total_runs run(s) to complete..."
echo "(follow individual runs with: tail -f $LOGS_DIR/<run_id>.log)"
echo ""

fail=0
pass=0
for idx in "${!PIDS[@]}"; do
    pid="${PIDS[$idx]}"
    run_id="${RUN_IDS[$idx]}"
    if wait "$pid"; then
        echo "  ✓ $run_id"
        pass=$((pass + 1))
    else
        echo "  ✗ $run_id  (see $LOGS_DIR/${run_id}.log)"
        fail=$((fail + 1))
    fi
done

echo ""
echo "Summary: $pass passed, $fail failed (of $total_runs total)"
echo "Results: $RESULTS_DIR/"
echo "Analyze: docker run -v \"$RESULTS_DIR:/bench/results\" elmq-bench /bench/analyze.sh"

if [[ "$fail" -gt 0 ]]; then
    exit 1
fi
