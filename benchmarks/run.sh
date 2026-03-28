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
MCP_CONFIG="$BENCH_DIR/mcp-config.json"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%S)"

ARM="${1:-both}"

if [[ "$ARM" != "control" && "$ARM" != "treatment" && "$ARM" != "both" ]]; then
    echo "Usage: run.sh [control|treatment|both]" >&2
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
    git add -A
    git commit -q -m "initial fixture state"

    # Build claude command base
    local claude_base=(
        claude -p
        --model sonnet
        --output-format json
        --permission-mode bypassPermissions
        --system-prompt-file "$SYSTEM_PROMPT"
    )

    if [[ "$arm" == "treatment" ]]; then
        claude_base+=(--mcp-config "$MCP_CONFIG")
    fi

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

        # Run Claude
        cd "$work_dir"
        echo "Running Claude..."
        local session_json
        if session_json=$("${claude_base[@]}" -- "$prompt" 2>&1); then
            echo "$session_json" > "$scenario_dir/session.json"
            echo "Claude completed successfully"
        else
            echo "$session_json" > "$scenario_dir/session.json"
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
if [[ "$ARM" == "control" || "$ARM" == "both" ]]; then
    run_arm "control"
fi

if [[ "$ARM" == "treatment" || "$ARM" == "both" ]]; then
    run_arm "treatment"
fi

echo ""
echo "Run complete. Analyze with: /bench/analyze.sh"
