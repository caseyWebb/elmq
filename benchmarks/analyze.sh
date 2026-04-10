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

# Auto-pipe through glow when stdout is a TTY and glow is available.
if [ -t 1 ] && command -v glow >/dev/null 2>&1; then
    exec > >(glow -w "$(tput cols)")
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
# Returns: input_tokens output_tokens cache_read cache_create duration_ms num_turns
extract_session_stats() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "0 0 0 0 0 0"
        return
    fi
    grep '"type":"result"' "$file" | tail -1 | jq -r '
        ((.usage // {}) as $u |
         "\($u.input_tokens // 0) \($u.output_tokens // 0) \($u.cache_read_input_tokens // 0) \($u.cache_creation_input_tokens // 0) \(.duration_ms // 0) \(.num_turns // 0)")
    ' 2>/dev/null || echo "0 0 0 0 0 0"
}

# Anthropic Sonnet 4.6 pricing, USD per million tokens.
# Cache write uses the 5-minute TTL price; adjust if the harness moves to 1h caching.
# Source: https://www.anthropic.com/pricing
PRICE_INPUT="3.00"
PRICE_OUTPUT="15.00"
PRICE_CACHE_READ="0.30"
PRICE_CACHE_WRITE="3.75"

# Compute cost in integer micro-dollars (millionths of a dollar) from raw token counts.
# Integer output keeps downstream comparisons pure-bash; format_cost renders it as $X.XXXX.
compute_cost_micros() {
    local in="$1" out="$2" cr="$3" cc="$4"
    awk -v i="${in:-0}" -v o="${out:-0}" -v cr="${cr:-0}" -v cc="${cc:-0}" \
        -v pi="$PRICE_INPUT" -v po="$PRICE_OUTPUT" -v pr="$PRICE_CACHE_READ" -v pw="$PRICE_CACHE_WRITE" \
        'BEGIN { printf "%d", int(i*pi + o*po + cr*pr + cc*pw + 0.5) }'
}

# Format micro-dollars as "$X.XXXX"
format_cost() {
    local micros="${1:-0}"
    awk -v m="$micros" 'BEGIN { printf "$%.4f", m/1000000 }'
}

# Compute sample standard deviation from space-separated values.
# Usage: echo "v1 v2 v3" | compute_stddev
# Returns integer (rounded). For n<=1, returns 0.
compute_stddev() {
    awk '{
        n = NF
        if (n <= 1) { print 0; exit }
        sum = 0
        for (i = 1; i <= n; i++) sum += $i
        mean = sum / n
        ss = 0
        for (i = 1; i <= n; i++) ss += ($i - mean)^2
        printf "%d", int(sqrt(ss / (n - 1)) + 0.5)
    }'
}

# Find outliers (>2σ from mean). Takes space-separated values.
# Prints "index:value" for each outlier (1-indexed). Empty output = no outliers.
find_outliers() {
    awk '{
        n = NF
        if (n <= 2) exit
        sum = 0
        for (i = 1; i <= n; i++) sum += $i
        mean = sum / n
        ss = 0
        for (i = 1; i <= n; i++) ss += ($i - mean)^2
        sd = sqrt(ss / (n - 1))
        if (sd == 0) exit
        for (i = 1; i <= n; i++) {
            if ( ($i - mean) > 2*sd || (mean - $i) > 2*sd )
                printf "%d:%s\n", i, $i
        }
    }'
}

# Return a winner marker for a metric comparison.
# Lower wins by default; pass "reverse" as $3 for metrics where higher is better.
# Color mode: ANSI bold-green prefix/suffix wrapping the value (caller must use wrap_winner).
# No-color mode: 🟢 prefix for winners only.
# Returns empty for losses, ties, or missing opponents.
is_winner() {
    local mine="${1:-}" theirs="${2:-}" reverse="${3:-0}"
    if [ -z "$mine" ] || [ -z "$theirs" ]; then
        return 1
    fi
    if [ "$reverse" = "1" ]; then
        [ "$mine" -gt "$theirs" ]
    else
        [ "$mine" -lt "$theirs" ]
    fi
}

# Format a cell value with winner highlighting.
# Usage: format_cell DISPLAY_VALUE RAW_MINE RAW_OPPONENT [reverse]
# RAW values are used for numeric comparison; DISPLAY_VALUE is what gets printed.
# Color mode: bold green for winners. No-color mode: 🟢 prefix.
format_cell() {
    local display="$1" raw_mine="${2:-}" raw_opponent="${3:-}" reverse="${4:-0}"
    if is_winner "$raw_mine" "$raw_opponent" "$reverse" 2>/dev/null; then
        printf '🟢 **%s**' "$display"
    else
        printf '%s' "$display"
    fi
}

# Count tool calls in a session.json (stream-json format)
# Tool uses appear as content blocks with type=tool_use in assistant messages
count_tool_calls() {
    local file="$1"
    if [ ! -f "$file" ]; then
        echo "0"
        return
    fi
    local n
    n=$(grep -c '"tool_use"' "$file" 2>/dev/null) || true
    echo "${n:-0}"
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

echo "# elmq Benchmark Analysis"
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
        total_turns=0
        total_duration=0
        pass_count=0
        run_count=0
        cost_vals=""
        turns_vals=""
        run_names=""

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

            read -r input output cache_read cache_create duration_ms turns <<< "$(extract_session_stats "$session_file")"
            total_input=$((total_input + input))
            total_output=$((total_output + output))
            total_cache_read=$((total_cache_read + cache_read))
            total_cache_create=$((total_cache_create + cache_create))
            total_duration=$((total_duration + duration_ms))
            total_turns=$((total_turns + turns))

            run_cost="$(compute_cost_micros "$input" "$output" "$cache_read" "$cache_create")"
            cost_vals="$cost_vals $run_cost"
            turns_vals="$turns_vals $turns"
            run_names="$run_names $(basename "$run_dir")"

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

        agg_set COST_VALS  "$scenario" "$arm" "$cost_vals"
        agg_set TURNS_VALS "$scenario" "$arm" "$turns_vals"
        agg_set RUN_NAMES  "$scenario" "$arm" "$run_names"
        agg_set COST_SD    "$scenario" "$arm" "$(echo "$cost_vals" | compute_stddev)"
        agg_set TURNS_SD   "$scenario" "$arm" "$(echo "$turns_vals" | compute_stddev)"

        avg_input=$((total_input / run_count))
        avg_output=$((total_output / run_count))
        avg_cache_read=$((total_cache_read / run_count))
        avg_cache_create=$((total_cache_create / run_count))

        agg_set INPUT    "$scenario" "$arm" "$avg_input"
        agg_set OUTPUT   "$scenario" "$arm" "$avg_output"
        agg_set CACHE_R  "$scenario" "$arm" "$avg_cache_read"
        agg_set CACHE_C  "$scenario" "$arm" "$avg_cache_create"
        agg_set CACHED   "$scenario" "$arm" $((avg_cache_read + avg_cache_create))
        agg_set TOKENS   "$scenario" "$arm" $((avg_input + avg_output))
        agg_set COST     "$scenario" "$arm" "$(compute_cost_micros "$avg_input" "$avg_output" "$avg_cache_read" "$avg_cache_create")"
        agg_set TOOLS    "$scenario" "$arm" $((total_tools / run_count))
        agg_set TURNS    "$scenario" "$arm" $((total_turns / run_count))
        agg_set DURATION "$scenario" "$arm" $((total_duration / run_count))
        agg_set PASS     "$scenario" "$arm" "$pass_count"
    done
done

# ---------------------------------------------------------------------------
# Phase 2: detect outliers (>2σ from mean) — warn but still include in stats
# ---------------------------------------------------------------------------
outlier_warnings=""
for scenario in "${SCENARIOS[@]}"; do
    for arm in "${ARMS[@]}"; do
        runs=$(agg_get RUNS "$scenario" "$arm"); runs=${runs:-0}
        [ "$runs" -le 2 ] && continue

        cost_vals=$(agg_get COST_VALS "$scenario" "$arm")
        turns_vals=$(agg_get TURNS_VALS "$scenario" "$arm")
        run_names_str=$(agg_get RUN_NAMES "$scenario" "$arm")

        # Convert run_names to an array for index lookup
        read -ra _rn_arr <<< "$run_names_str"

        cost_outliers=$(echo "$cost_vals" | find_outliers)
        turns_outliers=$(echo "$turns_vals" | find_outliers)

        if [ -n "$cost_outliers" ]; then
            while IFS=: read -r idx val; do
                [ -z "$idx" ] && continue
                rname="${_rn_arr[$((idx-1))]:-run#$idx}"
                outlier_warnings="${outlier_warnings}${scenario} ${arm} ${rname} COST $(format_cost "$val")
"
            done <<< "$cost_outliers"
        fi
        if [ -n "$turns_outliers" ]; then
            while IFS=: read -r idx val; do
                [ -z "$idx" ] && continue
                rname="${_rn_arr[$((idx-1))]:-run#$idx}"
                outlier_warnings="${outlier_warnings}${scenario} ${arm} ${rname} TURNS ${val}
"
            done <<< "$turns_outliers"
        fi
    done
done

# ---------------------------------------------------------------------------
# Render: Per-scenario detail table (markdown)
# ---------------------------------------------------------------------------
echo "## Per-Scenario Detail"
echo ""
echo "> Pricing: input \$$PRICE_INPUT/M · output \$$PRICE_OUTPUT/M · cache read \$$PRICE_CACHE_READ/M · cache write \$$PRICE_CACHE_WRITE/M (Sonnet 4.6)"
echo ""
# Track grand totals across all scenarios where both arms produced data.
overall_ctrl_cost=0
overall_trt_cost=0
overall_ctrl_tokens=0
overall_trt_tokens=0
overall_ctrl_cached=0
overall_trt_cached=0
overall_ctrl_duration=0
overall_trt_duration=0
overall_ctrl_turns=0
overall_trt_turns=0
overall_ctrl_pass=0
overall_trt_pass=0
overall_ctrl_runs=0
overall_trt_runs=0

print_detail_row() {
    local scenario="$1" arm="$2"
    local runs pass input output cache_r cache_c cost turns duration
    local o_input o_output o_cache_r o_cache_c o_cost o_turns o_duration o_pass o_runs
    local other_arm

    if [ "$arm" = "control" ]; then other_arm="treatment"; else other_arm="control"; fi

    runs=$(agg_get RUNS "$scenario" "$arm"); runs=${runs:-0}
    o_runs=$(agg_get RUNS "$scenario" "$other_arm"); o_runs=${o_runs:-0}

    if [ "$runs" -eq 0 ]; then
        echo "| $arm | - | - | - | - | - | - | - | - |"
        return
    fi

    input=$(agg_get INPUT "$scenario" "$arm")
    output=$(agg_get OUTPUT "$scenario" "$arm")
    cache_r=$(agg_get CACHE_R "$scenario" "$arm")
    cache_c=$(agg_get CACHE_C "$scenario" "$arm")
    cost=$(agg_get COST "$scenario" "$arm")
    turns=$(agg_get TURNS "$scenario" "$arm")
    duration=$(agg_get DURATION "$scenario" "$arm")
    pass=$(agg_get PASS "$scenario" "$arm"); pass=${pass:-0}

    if [ "$o_runs" -gt 0 ]; then
        o_input=$(agg_get INPUT "$scenario" "$other_arm")
        o_output=$(agg_get OUTPUT "$scenario" "$other_arm")
        o_cache_r=$(agg_get CACHE_R "$scenario" "$other_arm")
        o_cache_c=$(agg_get CACHE_C "$scenario" "$other_arm")
        o_cost=$(agg_get COST "$scenario" "$other_arm")
        o_turns=$(agg_get TURNS "$scenario" "$other_arm")
        o_duration=$(agg_get DURATION "$scenario" "$other_arm")
        o_pass=$(agg_get PASS "$scenario" "$other_arm"); o_pass=${o_pass:-0}
    else
        o_input=""; o_output=""; o_cache_r=""; o_cache_c=""
        o_cost=""; o_turns=""; o_duration=""; o_pass=""
    fi

    # Build each cell with winner highlighting
    local c_in c_out c_cr c_cc c_cost c_turns c_time
    c_in=$(format_cell "$(format_int "$input")" "$input" "$o_input")
    c_out=$(format_cell "$(format_int "$output")" "$output" "$o_output")
    c_cr=$(format_cell "$(format_int "$cache_r")" "$cache_r" "$o_cache_r")
    c_cc=$(format_cell "$(format_int "$cache_c")" "$cache_c" "$o_cache_c")

    # Cost cell with optional stddev
    local cost_str
    cost_str="$(format_cost "$cost")"
    if [ "$runs" -gt 1 ]; then
        local cost_sd
        cost_sd=$(agg_get COST_SD "$scenario" "$arm"); cost_sd=${cost_sd:-0}
        local high_cv
        high_cv=$(awk -v sd="$cost_sd" -v m="$cost" \
            'BEGIN { print (m > 0 && sd/m > 0.5) ? 1 : 0 }')
        local warn=""
        [ "$high_cv" = "1" ] && warn=" ⚠️"
        cost_str="${cost_str} ±$(format_cost "$cost_sd")${warn}"
    fi
    c_cost=$(format_cell "$cost_str" "$cost" "$o_cost")

    # Turns cell with optional stddev
    local turns_str="$turns"
    if [ "$runs" -gt 1 ]; then
        local turns_sd
        turns_sd=$(agg_get TURNS_SD "$scenario" "$arm"); turns_sd=${turns_sd:-0}
        local high_cv
        high_cv=$(awk -v sd="$turns_sd" -v m="$turns" \
            'BEGIN { print (m > 0 && sd/m > 0.5) ? 1 : 0 }')
        local warn=""
        [ "$high_cv" = "1" ] && warn=" ⚠️"
        turns_str="${turns} ±${turns_sd}${warn}"
    fi
    c_turns=$(format_cell "$turns_str" "$turns" "$o_turns")

    local time_str
    time_str="$(format_duration "$duration")"
    c_time=$(format_cell "$time_str" "$duration" "$o_duration")

    local pass_cell
    if [ "$pass" -lt "$runs" ]; then
        pass_cell="❌ ${pass}/${runs}"
    else
        pass_cell="✅ ${pass}/${runs}"
    fi

    echo "| $arm | ${c_in} | ${c_out} | ${c_cr} | ${c_cc} | ${c_cost} | ${c_turns} | ${c_time} | ${pass_cell} |"
}

for scenario in "${SCENARIOS[@]}"; do
    echo "### $scenario"
    echo ""
    echo "| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |"
    echo "|---|---:|---:|---:|---:|---:|---:|---:|---|"
    print_detail_row "$scenario" "control"
    print_detail_row "$scenario" "treatment"
    echo ""

    cruns=$(agg_get RUNS "$scenario" control); cruns=${cruns:-0}
    truns=$(agg_get RUNS "$scenario" treatment); truns=${truns:-0}
    if [ "$cruns" -gt 0 ] && [ "$truns" -gt 0 ]; then
        overall_ctrl_cost=$((overall_ctrl_cost + $(agg_get COST "$scenario" control)))
        overall_trt_cost=$((overall_trt_cost + $(agg_get COST "$scenario" treatment)))
        overall_ctrl_tokens=$((overall_ctrl_tokens + $(agg_get TOKENS "$scenario" control)))
        overall_trt_tokens=$((overall_trt_tokens + $(agg_get TOKENS "$scenario" treatment)))
        overall_ctrl_cached=$((overall_ctrl_cached + $(agg_get CACHED "$scenario" control)))
        overall_trt_cached=$((overall_trt_cached + $(agg_get CACHED "$scenario" treatment)))
        overall_ctrl_duration=$((overall_ctrl_duration + $(agg_get DURATION "$scenario" control)))
        overall_trt_duration=$((overall_trt_duration + $(agg_get DURATION "$scenario" treatment)))
        overall_ctrl_turns=$((overall_ctrl_turns + $(agg_get TURNS "$scenario" control)))
        overall_trt_turns=$((overall_trt_turns + $(agg_get TURNS "$scenario" treatment)))
        overall_ctrl_pass=$((overall_ctrl_pass + $(agg_get PASS "$scenario" control)))
        overall_trt_pass=$((overall_trt_pass + $(agg_get PASS "$scenario" treatment)))
        overall_ctrl_runs=$((overall_ctrl_runs + cruns))
        overall_trt_runs=$((overall_trt_runs + truns))
    fi
done

# ---------------------------------------------------------------------------
# OVERALL summary (when both arms have data)
# ---------------------------------------------------------------------------
if [ "$overall_ctrl_runs" -gt 0 ] && [ "$overall_trt_runs" -gt 0 ]; then
    echo ""
    echo "## Overall"
    echo ""

    c_pass_cell="✅ ${overall_ctrl_pass}/${overall_ctrl_runs}"
    [ "$overall_ctrl_pass" -lt "$overall_ctrl_runs" ] && c_pass_cell="❌ ${overall_ctrl_pass}/${overall_ctrl_runs}"
    t_pass_cell="✅ ${overall_trt_pass}/${overall_trt_runs}"
    [ "$overall_trt_pass" -lt "$overall_trt_runs" ] && t_pass_cell="❌ ${overall_trt_pass}/${overall_trt_runs}"

    echo "| ARM | COST | TURNS | TIME | PASS |"
    echo "|---|---:|---:|---:|---|"
    echo "| control | $(format_cell "$(format_cost "$overall_ctrl_cost")" "$overall_ctrl_cost" "$overall_trt_cost") | $(format_cell "$overall_ctrl_turns" "$overall_ctrl_turns" "$overall_trt_turns") | $(format_cell "$(format_duration "$overall_ctrl_duration")" "$overall_ctrl_duration" "$overall_trt_duration") | ${c_pass_cell} |"
    echo "| treatment | $(format_cell "$(format_cost "$overall_trt_cost")" "$overall_trt_cost" "$overall_ctrl_cost") | $(format_cell "$overall_trt_turns" "$overall_trt_turns" "$overall_ctrl_turns") | $(format_cell "$(format_duration "$overall_trt_duration")" "$overall_trt_duration" "$overall_ctrl_duration") | ${t_pass_cell} |"

    echo ""

    # Delta summary
    cost_delta=$((overall_trt_cost - overall_ctrl_cost))
    cost_pct=$(awk -v d="$cost_delta" -v c="$overall_ctrl_cost" \
        'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')
    turns_delta=$((overall_trt_turns - overall_ctrl_turns))
    turns_pct=$(awk -v d="$turns_delta" -v c="$overall_ctrl_turns" \
        'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')

    cost_indicator=""
    [ "$cost_delta" -lt 0 ] && cost_indicator="🟢 "
    turns_indicator=""
    [ "$turns_delta" -lt 0 ] && turns_indicator="🟢 "

    # Pooled std dev for delta = sqrt(sum of per-scenario variances for both arms)
    all_cost_sds=""
    all_turns_sds=""
    for scenario in "${SCENARIOS[@]}"; do
        cruns=$(agg_get RUNS "$scenario" control); cruns=${cruns:-0}
        truns=$(agg_get RUNS "$scenario" treatment); truns=${truns:-0}
        [ "$cruns" -gt 0 ] && [ "$truns" -gt 0 ] || continue
        all_cost_sds="$all_cost_sds $(agg_get COST_SD "$scenario" control) $(agg_get COST_SD "$scenario" treatment)"
        all_turns_sds="$all_turns_sds $(agg_get TURNS_SD "$scenario" control) $(agg_get TURNS_SD "$scenario" treatment)"
    done
    delta_cost_sd=$(echo "$all_cost_sds" | awk '{
        ss = 0; for (i=1; i<=NF; i++) ss += $i * $i
        printf "%d", int(sqrt(ss) + 0.5)
    }')
    delta_turns_sd=$(echo "$all_turns_sds" | awk '{
        ss = 0; for (i=1; i<=NF; i++) ss += $i * $i
        printf "%d", int(sqrt(ss) + 0.5)
    }')

    echo "- ${cost_indicator}**Δ cost** (trt − ctrl): $(format_cost "$cost_delta") (${cost_pct}%) ± $(format_cost "$delta_cost_sd")"
    echo "- ${turns_indicator}**Δ turns** (trt − ctrl): ${turns_delta} (${turns_pct}%) ± ${delta_turns_sd}"
fi

echo ""

# ---------------------------------------------------------------------------
# Outlier warnings (>2σ from mean) — included in stats but flagged here
# ---------------------------------------------------------------------------
if [ -n "$outlier_warnings" ]; then
    echo "## ⚠️ Outlier Warnings"
    echo ""
    echo "> Included in stats but flagged (>2σ from mean)"
    echo ""
    echo "| SCENARIO | ARM | RUN | METRIC | VALUE |"
    echo "|---|---|---|---|---|"
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        read -r scn arm run metric val <<< "$line"
        echo "| $scn | $arm | $run | $metric | $val |"
    done <<< "$outlier_warnings"
    echo ""
fi

# ---------------------------------------------------------------------------
# Tool Breakdown (per scenario, latest run)
# ---------------------------------------------------------------------------
echo "## Tool Breakdown"
echo ""

for arm in "${ARMS[@]}"; do
    arm_dir="$RESULTS_DIR/$arm"
    [ -d "$arm_dir" ] || continue

    latest_run=$(ls -d "$arm_dir"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$latest_run" ] || continue

    echo "### $arm ($(basename "$latest_run"))"
    echo ""

    for scenario in "${SCENARIOS[@]}"; do
        session_file="$latest_run/$scenario/session.json"
        [ -f "$session_file" ] || continue

        status_file="$latest_run/$scenario/verify.status"
        status="?"
        [ -f "$status_file" ] && status="$(cat "$status_file")"

        local_status_emoji="❓"
        [ "$status" = "PASSED" ] && local_status_emoji="✅"
        [ "$status" = "FAILED" ] && local_status_emoji="❌"

        echo "#### ${local_status_emoji} $scenario"
        echo ""

        breakdown=$(tool_breakdown "$session_file")
        if [ -n "$breakdown" ]; then
            echo "| Count | Tool |"
            echo "|---:|---|"
            echo "$breakdown" | while read -r count tool; do
                echo "| $count | $tool |"
            done
        else
            echo "*(no tool calls)*"
        fi
        echo ""
    done
done

# ---------------------------------------------------------------------------
# Tool Call Details (per scenario, latest run)
# ---------------------------------------------------------------------------
echo "## Tool Call Details"
echo ""

for arm in "${ARMS[@]}"; do
    arm_dir="$RESULTS_DIR/$arm"
    [ -d "$arm_dir" ] || continue

    latest_run=$(ls -d "$arm_dir"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$latest_run" ] || continue

    echo "### $arm ($(basename "$latest_run"))"
    echo ""

    for scenario in "${SCENARIOS[@]}"; do
        session_file="$latest_run/$scenario/session.json"
        [ -f "$session_file" ] || continue

        status_file="$latest_run/$scenario/verify.status"
        status="?"
        [ -f "$status_file" ] && status="$(cat "$status_file")"

        local_status_emoji="❓"
        [ "$status" = "PASSED" ] && local_status_emoji="✅"
        [ "$status" = "FAILED" ] && local_status_emoji="❌"

        echo "#### ${local_status_emoji} $scenario"
        echo ""

        grep '"tool_use"' "$session_file" | jq -c '
            .message.content[]? | select(.type=="tool_use") | {name, input}
        ' 2>/dev/null | {
            n=0
            while IFS= read -r line; do
                n=$((n + 1))
                name=$(echo "$line" | jq -r '.name')
                input=$(echo "$line" | jq -c '.input')
                summary=$(tool_call_summary "$name" "$input")
                echo "${n}. \`${name}\` ${summary}"
            done
            if [ "$n" -eq 0 ]; then
                echo "*(no tool calls)*"
            fi
        }
        echo ""
    done
done
