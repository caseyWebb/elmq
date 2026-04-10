#!/usr/bin/env bash
# benchmarks/run.sh — benchmark runner.
#
# On the host: builds Docker, launches N parallel containers per arm.
# Inside the container: executes scenarios sequentially for one arm.
#
# Usage (host):
#   ./benchmarks/run.sh                    # 1 control + 1 treatment in parallel
#   ./benchmarks/run.sh -n 3              # 3 control + 3 treatment (6 total)
#   ./benchmarks/run.sh control            # 1 control only
#   ./benchmarks/run.sh treatment -n 5     # 5 treatments in parallel

set -euo pipefail

# ---------------------------------------------------------------------------
# Container mode — /bench/scenarios only exists inside the Docker image
# ---------------------------------------------------------------------------
if [[ -d /bench/scenarios ]]; then

if [ -z "${CLAUDE_CODE_OAUTH_TOKEN:-}" ]; then
    echo "Error: CLAUDE_CODE_OAUTH_TOKEN is not set." >&2
    echo "Create benchmarks/.env with: CLAUDE_CODE_OAUTH_TOKEN=your-token-here" >&2
    exit 1
fi

BENCH_DIR="/bench"
FIXTURE_DIR="$BENCH_DIR/fixture"
RESULTS_DIR="$BENCH_DIR/results"
SCENARIOS_DIR="$BENCH_DIR/scenarios"
SYSTEM_PROMPT="$BENCH_DIR/system-prompt.md"
TIMESTAMP="${BENCHMARK_RUN_ID:-$(date -u +%Y-%m-%dT%H%M%S)}"

ARM="${1:-control}"

if [[ "$ARM" != "control" && "$ARM" != "treatment" && "$ARM" != "all" ]]; then
    echo "Usage: run.sh [control|treatment|all]" >&2
    exit 1
fi

SCENARIOS=(
    "01-add-feature"
    "02-rename-module"
    "03-extract-module"
    "04-add-route"
    "05-add-variant"
)

run_arm() {
    local arm="$1"
    local run_dir="$RESULTS_DIR/$arm/$TIMESTAMP"
    local work_dir="$run_dir/workdir"

    echo "=== Running $arm arm at $TIMESTAMP ==="

    # Create fresh working copy from fixture
    mkdir -p "$run_dir"
    cp -r "$FIXTURE_DIR" "$work_dir"

    # Remove submodule .git reference and initialize a fresh repo
    rm -rf "$work_dir/.git"
    cd "$work_dir"
    git init -q
    git config user.email "bench@elmq"
    git config user.name "elmq-bench"

    # Treatment arm: deliver elmq guidance as project memory via CLAUDE.md in the
    # workdir. CLAUDE.md is picked up automatically by claude -p from the current
    # working directory, and unlike --append-system-prompt-file it propagates to
    # any subagents spawned via the Task tool (Explore, etc). The empirical basis
    # is in openspec/changes/elmq-guide-v2/design.md §"Delivery mechanism".
    if [[ "$arm" == "treatment" ]]; then
        elmq guide > "$work_dir/CLAUDE.md"
    fi

    git add -A
    git commit -q -m "initial fixture state"

    # Build claude command base
    local claude_base=(
        claude -p
        --verbose
        --model sonnet
        --output-format stream-json
        --dangerously-skip-permissions
        --append-system-prompt-file "$SYSTEM_PROMPT"
    )

    # (Treatment-arm guidance is delivered via CLAUDE.md in workdir, above.)

    for scenario in "${SCENARIOS[@]}"; do
        local scenario_dir="$run_dir/$scenario"
        mkdir -p "$scenario_dir"

        local prompt_file="$SCENARIOS_DIR/$scenario/prompt.md"
        local verify_script="$SCENARIOS_DIR/$scenario/verify.sh"

        echo ""
        echo "--- $arm / $scenario ---"

        # Read prompt
        local prompt
        prompt="$(cat "$prompt_file")"

        # Run Claude — stream to file, print tool calls live
        cd "$work_dir"
        echo "Running Claude..."
        if "${claude_base[@]}" -- "$prompt" 2>&1 | tee "$scenario_dir/session.json" | grep --line-buffered '"tool_use"' | jq -r '
            .message.content[]? | select(.type=="tool_use") |
            "  → \(.name) " + (
                if .name == "Read" then .input.file_path // ""
                elif .name == "Write" then .input.file_path // ""
                elif .name == "Edit" then .input.file_path // ""
                elif .name == "Bash" then (.input.command // "")[:80]
                elif .name == "Glob" then .input.pattern // ""
                elif .name == "Grep" then .input.pattern // ""
                elif .name == "Agent" then .input.description // ""
                else ""
            end)' 2>/dev/null; then
            echo "Claude completed successfully"
        else
            echo "WARNING: Claude exited with non-zero status"
        fi

        # Capture diff
        git diff > "$scenario_dir/diff.patch"

        # Commit changes
        git add -A
        git commit -q -m "after $scenario" --allow-empty

        # Run verification
        echo "Running verification..."
        if bash "$verify_script" > "$scenario_dir/verify.log" 2>&1; then
            echo "Verification: PASSED"
            echo "PASSED" > "$scenario_dir/verify.status"
        else
            echo "Verification: FAILED"
            echo "FAILED" > "$scenario_dir/verify.status"
        fi
    done

    echo ""
    echo "=== $arm arm complete. Results in $run_dir ==="
}

# Run requested arms
if [[ "$ARM" == "control" || "$ARM" == "all" ]]; then
    run_arm "control"
fi

if [[ "$ARM" == "treatment" || "$ARM" == "all" ]]; then
    run_arm "treatment"
fi

echo ""
echo "Run complete. Analyze with: /bench/analyze.sh"
exit 0
fi

# ---------------------------------------------------------------------------
# Host mode — orchestrate Docker runs
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCH_DIR="$SCRIPT_DIR"
RESULTS_DIR="$BENCH_DIR/results"
LOGS_DIR="$RESULTS_DIR/logs"
ENV_FILE="$BENCH_DIR/.env"

usage() {
    cat <<'USAGE' >&2
Usage: ./benchmarks/run.sh [control|treatment] [-n N]

Arguments:
  control|treatment  Optional. If omitted, runs both arms.
  -n N               Optional. Number of runs per arm. Default: 1.

Examples:
  ./benchmarks/run.sh                    # 1 control + 1 treatment
  ./benchmarks/run.sh -n 3              # 3 of each (6 parallel runs)
  ./benchmarks/run.sh control            # 1 control only
  ./benchmarks/run.sh treatment -n 5     # 5 treatments in parallel
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
if ! "$BENCH_DIR/build.sh" >/dev/null; then
    echo "Error: ./benchmarks/build.sh failed. Re-run it directly to see the error output." >&2
    exit 1
fi

mkdir -p "$LOGS_DIR"

TIMESTAMP_BASE="$(date -u +%Y-%m-%dT%H%M%S)"
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
