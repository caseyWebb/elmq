#!/usr/bin/env bash
set -euo pipefail

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
        cp "$BENCH_DIR/elmq-guide.md" "$work_dir/CLAUDE.md"
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
