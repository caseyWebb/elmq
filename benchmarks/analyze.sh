#!/usr/bin/env bash
set -euo pipefail

RESULTS_DIR="${1:-/bench/results}"

SCENARIOS=(
    "01-add-feature"
    "02-rename-module"
    "03-extract-module"
    "04-add-route"
    "05-add-variant"
)

ARMS=("control")

# Extract token metrics from a session.json (stream-json format: one JSON object per line)
# The result message (last line with type=result) contains usage data.
# Returns: input_tokens output_tokens cache_read cache_create
extract_tokens() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "0 0 0 0"
        return
    fi
    grep '"type":"result"' "$file" | tail -1 | jq -r '
        (.usage // {}) |
        "\(.input_tokens // 0) \(.output_tokens // 0) \(.cache_read_input_tokens // 0) \(.cache_creation_input_tokens // 0)"
    ' 2>/dev/null || echo "0 0 0 0"
}

# Count tool calls in a session.json (stream-json format)
# Tool uses appear as content blocks with type=tool_use in assistant messages
count_tool_calls() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "0"
        return
    fi
    grep -c '"tool_use"' "$file" 2>/dev/null || echo "0"
}

# Get tool call breakdown from a session.json
# Returns sorted "count toolname" lines
tool_breakdown() {
    local file="$1"
    if [ ! -f "$file" ]; then
        return
    fi
    grep -oE '"(Read|Write|Edit|Bash|Glob|Grep|Agent|ToolSearch)"' "$file" \
        | sed 's/"//g' | sort | uniq -c | sort -rn
}

# Check if a run is "broken" (early scenario failed)
is_run_broken_at() {
    local run_dir="$1"
    local target_scenario="$2"

    for scenario in "${SCENARIOS[@]}"; do
        if [[ "$scenario" == "$target_scenario" ]]; then
            return 1  # not broken — we reached the target
        fi
        local status_file="$run_dir/$scenario/verify.status"
        if [ -f "$status_file" ] && [[ "$(cat "$status_file")" == "FAILED" ]]; then
            return 0  # broken — earlier scenario failed
        fi
    done
    return 1
}

echo "============================================"
echo "  elmq MCP Benchmark Analysis"
echo "============================================"
echo ""

# Print header
printf "%-22s %-10s %8s %8s %8s %8s %8s %6s %6s\n" \
    "SCENARIO" "ARM" "IN_TOK" "OUT_TOK" "CACHE_R" "CACHE_C" "TOTAL" "TOOLS" "PASS"
printf "%s\n" "$(printf '%.0s-' {1..100})"

for scenario in "${SCENARIOS[@]}"; do
    for arm in "${ARMS[@]}"; do
        arm_dir="$RESULTS_DIR/$arm"

        if [ ! -d "$arm_dir" ]; then
            continue
        fi

        # Collect metrics across all runs
        total_input=0
        total_output=0
        total_cache_read=0
        total_cache_create=0
        total_tools=0
        pass_count=0
        run_count=0

        for run_dir in "$arm_dir"/*/; do
            [ -d "$run_dir" ] || continue

            # Skip if an earlier scenario failed in this run
            if is_run_broken_at "$run_dir" "$scenario"; then
                continue
            fi

            scenario_dir="$run_dir/$scenario"
            [ -d "$scenario_dir" ] || continue

            session_file="$scenario_dir/session.json"
            if [ ! -f "$session_file" ]; then
                continue
            fi

            run_count=$((run_count + 1))

            read -r input output cache_read cache_create <<< "$(extract_tokens "$session_file")"
            total_input=$((total_input + input))
            total_output=$((total_output + output))
            total_cache_read=$((total_cache_read + cache_read))
            total_cache_create=$((total_cache_create + cache_create))

            tools="$(count_tool_calls "$session_file")"
            total_tools=$((total_tools + tools))

            status_file="$scenario_dir/verify.status"
            if [ -f "$status_file" ] && [[ "$(cat "$status_file")" == "PASSED" ]]; then
                pass_count=$((pass_count + 1))
            fi
        done

        if [ "$run_count" -eq 0 ]; then
            printf "%-22s %-10s %8s %8s %8s %8s %8s %6s %6s\n" \
                "$scenario" "$arm" "-" "-" "-" "-" "-" "-" "-"
            continue
        fi

        avg_input=$((total_input / run_count))
        avg_output=$((total_output / run_count))
        avg_cache_read=$((total_cache_read / run_count))
        avg_cache_create=$((total_cache_create / run_count))
        avg_total=$((avg_input + avg_output))
        avg_tools=$((total_tools / run_count))

        printf "%-22s %-10s %8d %8d %8d %8d %8d %6d %3d/%-2d\n" \
            "$scenario" "$arm" \
            "$avg_input" "$avg_output" "$avg_cache_read" "$avg_cache_create" "$avg_total" \
            "$avg_tools" "$pass_count" "$run_count"
    done
done

echo ""
echo "============================================"
echo "  Summary (averaged across all scenarios)"
echo "============================================"
echo ""

for arm in "${ARMS[@]}"; do
    arm_dir="$RESULTS_DIR/$arm"
    [ -d "$arm_dir" ] || continue

    grand_input=0
    grand_output=0
    grand_tools=0
    grand_pass=0
    grand_total_runs=0
    scenario_count=0

    for scenario in "${SCENARIOS[@]}"; do
        total_input=0
        total_output=0
        total_tools=0
        pass_count=0
        run_count=0

        for run_dir in "$arm_dir"/*/; do
            [ -d "$run_dir" ] || continue
            is_run_broken_at "$run_dir" "$scenario" && continue

            scenario_dir="$run_dir/$scenario"
            [ -d "$scenario_dir" ] || continue
            [ -f "$scenario_dir/session.json" ] || continue

            run_count=$((run_count + 1))

            read -r input output _ _ <<< "$(extract_tokens "$scenario_dir/session.json")"
            total_input=$((total_input + input))
            total_output=$((total_output + output))

            tools="$(count_tool_calls "$scenario_dir/session.json")"
            total_tools=$((total_tools + tools))

            status_file="$scenario_dir/verify.status"
            if [ -f "$status_file" ] && [[ "$(cat "$status_file")" == "PASSED" ]]; then
                pass_count=$((pass_count + 1))
            fi
        done

        if [ "$run_count" -gt 0 ]; then
            grand_input=$((grand_input + total_input / run_count))
            grand_output=$((grand_output + total_output / run_count))
            grand_tools=$((grand_tools + total_tools / run_count))
            grand_pass=$((grand_pass + pass_count))
            grand_total_runs=$((grand_total_runs + run_count))
            scenario_count=$((scenario_count + 1))
        fi
    done

    if [ "$scenario_count" -gt 0 ]; then
        avg_input=$((grand_input / scenario_count))
        avg_output=$((grand_output / scenario_count))
        avg_total=$((avg_input + avg_output))
        avg_tools=$((grand_tools / scenario_count))
        echo "$arm:"
        echo "  Avg input tokens/scenario:  $avg_input"
        echo "  Avg output tokens/scenario: $avg_output"
        echo "  Avg total tokens/scenario:  $avg_total"
        echo "  Avg tool calls/scenario:    $avg_tools"
        echo "  Verification pass rate:     $grand_pass/$grand_total_runs"
        echo ""
    fi
done

echo "============================================"
echo "  Tool Breakdown (per scenario, latest run)"
echo "============================================"
echo ""

for arm in "${ARMS[@]}"; do
    arm_dir="$RESULTS_DIR/$arm"
    [ -d "$arm_dir" ] || continue

    # Use the latest run
    latest_run=$(ls -d "$arm_dir"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$latest_run" ] || continue

    echo "$arm ($(basename "$latest_run")):"
    echo ""

    for scenario in "${SCENARIOS[@]}"; do
        session_file="$latest_run/$scenario/session.json"
        [ -f "$session_file" ] || continue

        status_file="$latest_run/$scenario/verify.status"
        status="?"
        [ -f "$status_file" ] && status="$(cat "$status_file")"

        echo "  $scenario [$status]:"
        breakdown=$(tool_breakdown "$session_file")
        if [ -n "$breakdown" ]; then
            echo "$breakdown" | while read -r count tool; do
                printf "    %4s  %s\n" "$count" "$tool"
            done
        else
            echo "    (no tool calls)"
        fi
        echo ""
    done
done

echo "============================================"
echo "  Tool Call Details (per scenario, latest run)"
echo "============================================"
echo ""

# Extract a one-line summary of a tool call's arguments
tool_call_summary() {
    local name="$1"
    local input="$2"
    local max_len=100

    local summary
    case "$name" in
        Read)
            summary=$(echo "$input" | jq -r '.file_path // empty' 2>/dev/null)
            ;;
        Write)
            summary=$(echo "$input" | jq -r '.file_path // empty' 2>/dev/null)
            ;;
        Edit)
            summary=$(echo "$input" | jq -r '.file_path // empty' 2>/dev/null)
            ;;
        Glob)
            summary=$(echo "$input" | jq -r '"\(.pattern)" + (if .path then " in \(.path)" else "" end)' 2>/dev/null)
            ;;
        Grep)
            summary=$(echo "$input" | jq -r '"\(.pattern)" + (if .path then " in \(.path)" else "" end)' 2>/dev/null)
            ;;
        Bash)
            summary=$(echo "$input" | jq -r '.command // empty' 2>/dev/null | tr '\n' ' ' | sed 's/  */ /g')
            ;;
        Agent)
            summary=$(echo "$input" | jq -r '(.description // .prompt[:80]) + (if .subagent_type then " [\(.subagent_type)]" else "" end)' 2>/dev/null)
            ;;
        *)
            summary=$(echo "$input" | jq -c '.' 2>/dev/null)
            ;;
    esac

    # Strip workdir paths to reduce noise
    summary=$(echo "$summary" | sed -E 's|(/bench)?/results/[^/]+/[^/]+/workdir/||g')

    if [ ${#summary} -gt $max_len ]; then
        summary="${summary:0:$max_len}..."
    fi
    echo "$summary"
}

for arm in "${ARMS[@]}"; do
    arm_dir="$RESULTS_DIR/$arm"
    [ -d "$arm_dir" ] || continue

    latest_run=$(ls -d "$arm_dir"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$latest_run" ] || continue

    echo "$arm ($(basename "$latest_run")):"
    echo ""

    for scenario in "${SCENARIOS[@]}"; do
        session_file="$latest_run/$scenario/session.json"
        [ -f "$session_file" ] || continue

        status_file="$latest_run/$scenario/verify.status"
        status="?"
        [ -f "$status_file" ] && status="$(cat "$status_file")"

        echo "  $scenario [$status]:"

        # Extract all tool_use entries in order, including from subagent lines
        grep '"tool_use"' "$session_file" | jq -c '
            .message.content[]? | select(.type=="tool_use") | {name, input}
        ' 2>/dev/null | {
            n=0
            while IFS= read -r line; do
                n=$((n + 1))
                name=$(echo "$line" | jq -r '.name')
                input=$(echo "$line" | jq -c '.input')
                summary=$(tool_call_summary "$name" "$input")
                printf "    %3d. %-12s %s\n" "$n" "$name" "$summary"
            done
            if [ "$n" -eq 0 ]; then
                echo "    (no tool calls)"
            fi
        }
        echo ""
    done
done
