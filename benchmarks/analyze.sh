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

ARMS=("control" "treatment")

# Color setup: auto-disable when stdout is not a TTY or NO_COLOR is set.
# Force-enable with FORCE_COLOR=1; disable with --no-color or FORCE_COLOR=0.
_use_color=1
if [ "${1:-}" = "--no-color" ]; then
    _use_color=0
    shift
    RESULTS_DIR="${1:-/bench/results}"
fi
if [ -n "${NO_COLOR:-}" ]; then _use_color=0; fi
if [ "${FORCE_COLOR:-}" = "0" ]; then _use_color=0; fi
if [ -z "${FORCE_COLOR:-}" ] && [ ! -t 1 ]; then _use_color=0; fi

if [ "$_use_color" = "1" ]; then
    C_RESET=$'\033[0m'
    C_BOLD=$'\033[1m'
    C_DIM=$'\033[2m'
    C_RED=$'\033[31m'
    C_GREEN=$'\033[32m'
    C_YELLOW=$'\033[33m'
    C_CYAN=$'\033[36m'
else
    C_RESET=""; C_BOLD=""; C_DIM=""
    C_RED=""; C_GREEN=""; C_YELLOW=""; C_CYAN=""
fi

# Format ms → "M:SS" or "H:MM:SS"
format_duration() {
    local ms="${1:-0}"
    local s=$((ms / 1000))
    if [ "$s" -ge 3600 ]; then
        printf '%d:%02d:%02d' $((s/3600)) $(((s%3600)/60)) $((s%60))
    else
        printf '%d:%02d' $((s/60)) $((s%60))
    fi
}

# Format an integer with thousands separators (portable).
format_int() {
    local n="${1:-0}"
    local sign=""
    if [ "${n:0:1}" = "-" ]; then
        sign="-"
        n="${n:1}"
    elif [ "${n:0:1}" = "+" ]; then
        sign="+"
        n="${n:1}"
    fi
    # Insert commas every 3 digits from the right.
    local out=""
    while [ ${#n} -gt 3 ]; do
        out=",${n: -3}${out}"
        n="${n:0:${#n}-3}"
    done
    printf '%s%s%s' "$sign" "$n" "$out"
}

# Extract metrics from a session.json (stream-json format: one JSON object per line).
# The result message (last line with type=result) contains usage and duration.
# Returns: input_tokens output_tokens cache_read cache_create duration_ms
extract_session_stats() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "0 0 0 0 0"
        return
    fi
    grep '"type":"result"' "$file" | tail -1 | jq -r '
        ((.usage // {}) as $u |
         "\($u.input_tokens // 0) \($u.output_tokens // 0) \($u.cache_read_input_tokens // 0) \($u.cache_creation_input_tokens // 0) \(.duration_ms // 0)")
    ' 2>/dev/null || echo "0 0 0 0 0"
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

printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  elmq MCP Benchmark Analysis" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
echo ""

# ---------------------------------------------------------------------------
# Phase 1: aggregate per-(scenario,arm) stats into dynamically-named variables
# (portable across bash 3.2, which has no associative arrays) so the winners
# and detail tables can render from a single pass.
#
# Keys are of the form AGG_<FIELD>_<SCENARIO>_<ARM>, with scenario hyphens
# replaced by underscores. Accessors:
#   agg_set FIELD SCENARIO ARM VALUE
#   agg_get FIELD SCENARIO ARM            (prints value; empty if unset)
# ---------------------------------------------------------------------------
_sani() { local s="$1"; echo "${s//[^a-zA-Z0-9]/_}"; }
agg_set() {
    local var="AGG_$1_$(_sani "$2")_$3"
    printf -v "$var" '%s' "$4"
}
agg_get() {
    local var="AGG_$1_$(_sani "$2")_$3"
    eval "printf '%s' \"\${$var:-}\""
}

for scenario in "${SCENARIOS[@]}"; do
    for arm in "${ARMS[@]}"; do
        arm_dir="$RESULTS_DIR/$arm"
        agg_set RUNS "$scenario" "$arm" 0

        [ -d "$arm_dir" ] || continue

        total_input=0
        total_output=0
        total_cache_read=0
        total_cache_create=0
        total_tools=0
        total_duration=0
        pass_count=0
        run_count=0

        for run_dir in "$arm_dir"/*/; do
            [ -d "$run_dir" ] || continue
            if is_run_broken_at "$run_dir" "$scenario"; then
                continue
            fi

            scenario_dir="$run_dir/$scenario"
            [ -d "$scenario_dir" ] || continue
            session_file="$scenario_dir/session.json"
            [ -f "$session_file" ] || continue

            run_count=$((run_count + 1))

            read -r input output cache_read cache_create duration_ms <<< "$(extract_session_stats "$session_file")"
            total_input=$((total_input + input))
            total_output=$((total_output + output))
            total_cache_read=$((total_cache_read + cache_read))
            total_cache_create=$((total_cache_create + cache_create))
            total_duration=$((total_duration + duration_ms))

            tools="$(count_tool_calls "$session_file")"
            total_tools=$((total_tools + tools))

            status_file="$scenario_dir/verify.status"
            if [ -f "$status_file" ] && [[ "$(cat "$status_file")" == "PASSED" ]]; then
                pass_count=$((pass_count + 1))
            fi
        done

        agg_set RUNS "$scenario" "$arm" "$run_count"
        if [ "$run_count" -eq 0 ]; then
            continue
        fi

        agg_set INPUT    "$scenario" "$arm" $((total_input / run_count))
        agg_set OUTPUT   "$scenario" "$arm" $((total_output / run_count))
        agg_set CACHE_R  "$scenario" "$arm" $((total_cache_read / run_count))
        agg_set CACHE_C  "$scenario" "$arm" $((total_cache_create / run_count))
        agg_set TOTAL    "$scenario" "$arm" $(( (total_input + total_output + total_cache_read + total_cache_create) / run_count ))
        agg_set TOOLS    "$scenario" "$arm" $((total_tools / run_count))
        agg_set DURATION "$scenario" "$arm" $((total_duration / run_count))
        agg_set PASS     "$scenario" "$arm" "$pass_count"
    done
done

# ---------------------------------------------------------------------------
# Render: WINNERS table
# ---------------------------------------------------------------------------
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Winners (all-billable tokens, lower = better)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
echo ""

winners_header_fmt="%-22s %14s %14s %14s %9s   %-8s\n"
winners_row_fmt="%-22s %14s %14s %s%14s%s %s%8s%s   %s%-8s%s\n"
printf "$winners_header_fmt" "SCENARIO" "CTRL_TOTAL" "TRT_TOTAL" "Δ TOKENS" "Δ %" "WINNER"
printf '%s\n' "$(printf '%.0s-' {1..90})"

overall_ctrl=0
overall_trt=0
overall_gated=1   # 1 = still eligible for overall winner (all scenarios fully passing)

for scenario in "${SCENARIOS[@]}"; do
    cruns=$(agg_get RUNS "$scenario" control); cruns=${cruns:-0}
    truns=$(agg_get RUNS "$scenario" treatment); truns=${truns:-0}
    if [ "$cruns" -eq 0 ] || [ "$truns" -eq 0 ]; then
        overall_gated=0
        continue
    fi

    c_total=$(agg_get TOTAL "$scenario" control)
    t_total=$(agg_get TOTAL "$scenario" treatment)
    overall_ctrl=$((overall_ctrl + c_total))
    overall_trt=$((overall_trt + t_total))

    delta=$((t_total - c_total))
    # pct = delta / c_total * 100 (awk for float)
    pct=$(awk -v d="$delta" -v c="$c_total" 'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')

    # Sign-prefix the delta display.
    if [ "$delta" -gt 0 ]; then
        delta_str="+$(format_int "$delta")"
        pct_str="+${pct}%"
        delta_color="$C_RED"
    elif [ "$delta" -lt 0 ]; then
        delta_str="$(format_int "$delta")"
        pct_str="${pct}%"
        delta_color="$C_GREEN"
    else
        delta_str="0"
        pct_str="0.0%"
        delta_color="$C_DIM"
    fi

    # Winner gating: both arms must have all runs passing.
    cpass=$(agg_get PASS "$scenario" control); cpass=${cpass:-0}
    tpass=$(agg_get PASS "$scenario" treatment); tpass=${tpass:-0}
    both_pass=0
    if [ "$cpass" -eq "$cruns" ] && [ "$tpass" -eq "$truns" ]; then
        both_pass=1
    else
        overall_gated=0
    fi

    if [ "$both_pass" -eq 1 ]; then
        if [ "$delta" -lt 0 ]; then
            winner_str="trt"
            winner_color="$C_GREEN"
        elif [ "$delta" -gt 0 ]; then
            winner_str="ctrl"
            winner_color="$C_GREEN"
        else
            winner_str="tie"
            winner_color="$C_DIM"
        fi
    else
        winner_str="— (c:${cpass}/${cruns} t:${tpass}/${truns})"
        winner_color="$C_DIM"
    fi

    printf "$winners_row_fmt" \
        "$scenario" \
        "$(format_int "$c_total")" \
        "$(format_int "$t_total")" \
        "$delta_color" "$delta_str" "$C_RESET" \
        "$delta_color" "$pct_str" "$C_RESET" \
        "$winner_color" "$winner_str" "$C_RESET"
done

# OVERALL row (sum of per-scenario averages)
if [ "$overall_ctrl" -gt 0 ]; then
    printf '%s\n' "$(printf '%.0s-' {1..90})"
    o_delta=$((overall_trt - overall_ctrl))
    o_pct=$(awk -v d="$o_delta" -v c="$overall_ctrl" 'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')
    if [ "$o_delta" -gt 0 ]; then
        o_delta_str="+$(format_int "$o_delta")"
        o_pct_str="+${o_pct}%"
        o_color="$C_RED"
    elif [ "$o_delta" -lt 0 ]; then
        o_delta_str="$(format_int "$o_delta")"
        o_pct_str="${o_pct}%"
        o_color="$C_GREEN"
    else
        o_delta_str="0"
        o_pct_str="0.0%"
        o_color="$C_DIM"
    fi

    if [ "$overall_gated" -eq 1 ]; then
        if [ "$o_delta" -lt 0 ]; then
            o_winner="trt"
            o_winner_color="$C_BOLD$C_GREEN"
        elif [ "$o_delta" -gt 0 ]; then
            o_winner="ctrl"
            o_winner_color="$C_BOLD$C_GREEN"
        else
            o_winner="tie"
            o_winner_color="$C_DIM"
        fi
    else
        o_winner="—"
        o_winner_color="$C_DIM"
    fi

    printf "$winners_row_fmt" \
        "OVERALL (sum)" \
        "$(format_int "$overall_ctrl")" \
        "$(format_int "$overall_trt")" \
        "$o_color" "$o_delta_str" "$C_RESET" \
        "$o_color" "$o_pct_str" "$C_RESET" \
        "$o_winner_color" "$o_winner" "$C_RESET"
fi

echo ""

# ---------------------------------------------------------------------------
# Render: DETAIL table (per-arm, with TIME column; TOTAL = all-billable)
# ---------------------------------------------------------------------------
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Detail (averages across runs)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
echo ""

printf "%-22s %-10s %10s %10s %10s %10s %12s %6s %8s %8s\n" \
    "SCENARIO" "ARM" "IN_TOK" "OUT_TOK" "CACHE_R" "CACHE_C" "TOTAL" "TOOLS" "TIME" "PASS"
printf '%s\n' "$(printf '%.0s-' {1..116})"

for scenario in "${SCENARIOS[@]}"; do
    for arm in "${ARMS[@]}"; do
        runs=$(agg_get RUNS "$scenario" "$arm"); runs=${runs:-0}
        if [ "$runs" -eq 0 ]; then
            printf "%-22s %-10s %10s %10s %10s %10s %12s %6s %8s %8s\n" \
                "$scenario" "$arm" "-" "-" "-" "-" "-" "-" "-" "-"
            continue
        fi

        pass=$(agg_get PASS "$scenario" "$arm"); pass=${pass:-0}
        if [ "$pass" -eq "$runs" ]; then
            pass_color="$C_GREEN"
        else
            pass_color="$C_RED"
        fi

        time_str="$(format_duration "$(agg_get DURATION "$scenario" "$arm")")"

        printf "%-22s %-10s %10s %10s %10s %10s %12s %6d %8s %s%5s%s\n" \
            "$scenario" "$arm" \
            "$(format_int "$(agg_get INPUT    "$scenario" "$arm")")" \
            "$(format_int "$(agg_get OUTPUT   "$scenario" "$arm")")" \
            "$(format_int "$(agg_get CACHE_R  "$scenario" "$arm")")" \
            "$(format_int "$(agg_get CACHE_C  "$scenario" "$arm")")" \
            "$(format_int "$(agg_get TOTAL    "$scenario" "$arm")")" \
            "$(agg_get TOOLS "$scenario" "$arm")" \
            "$time_str" \
            "$pass_color" "${pass}/${runs}" "$C_RESET"
    done
done

echo ""
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Summary (averaged across all scenarios)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
echo ""

# Aggregate per-arm means from the per-(scenario,arm) aggregates already computed.
for arm in "${ARMS[@]}"; do
    grand_input=0
    grand_output=0
    grand_total=0
    grand_tools=0
    grand_duration=0
    grand_pass=0
    grand_total_runs=0
    scenario_count=0

    for scenario in "${SCENARIOS[@]}"; do
        runs=$(agg_get RUNS "$scenario" "$arm"); runs=${runs:-0}
        [ "$runs" -gt 0 ] || continue

        grand_input=$((grand_input + $(agg_get INPUT "$scenario" "$arm")))
        grand_output=$((grand_output + $(agg_get OUTPUT "$scenario" "$arm")))
        grand_total=$((grand_total + $(agg_get TOTAL "$scenario" "$arm")))
        grand_tools=$((grand_tools + $(agg_get TOOLS "$scenario" "$arm")))
        grand_duration=$((grand_duration + $(agg_get DURATION "$scenario" "$arm")))
        _p=$(agg_get PASS "$scenario" "$arm"); grand_pass=$((grand_pass + ${_p:-0}))
        grand_total_runs=$((grand_total_runs + runs))
        scenario_count=$((scenario_count + 1))
    done

    if [ "$scenario_count" -gt 0 ]; then
        avg_input=$((grand_input / scenario_count))
        avg_output=$((grand_output / scenario_count))
        avg_total=$((grand_total / scenario_count))
        avg_tools=$((grand_tools / scenario_count))
        avg_duration=$((grand_duration / scenario_count))

        if [ "$grand_pass" -eq "$grand_total_runs" ]; then
            pass_color="$C_GREEN"
        else
            pass_color="$C_RED"
        fi

        printf '%s%s:%s\n' "$C_BOLD" "$arm" "$C_RESET"
        printf "  Avg input tokens/scenario:       %s\n"    "$(format_int "$avg_input")"
        printf "  Avg output tokens/scenario:      %s\n"    "$(format_int "$avg_output")"
        printf "  Avg all-billable tokens/scen.:   %s\n"    "$(format_int "$avg_total")"
        printf "  Avg tool calls/scenario:         %s\n"    "$avg_tools"
        printf "  Avg wall time/scenario:          %s\n"    "$(format_duration "$avg_duration")"
        printf "  Verification pass rate:          %s%s/%s%s\n" "$pass_color" "$grand_pass" "$grand_total_runs" "$C_RESET"
        echo ""
    fi
done

printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Tool Breakdown (per scenario, latest run)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
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

printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Tool Call Details (per scenario, latest run)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
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
