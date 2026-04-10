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

# Pick a color based on whether $1 beats $2. Lower wins by default; pass reverse=1 for
# metrics where higher is better (e.g. PASS counts). Returns bold-green for a win,
# empty (default) for a loss, and dim for ties or missing opponents.
metric_color() {
    local mine="${1:-}" theirs="${2:-}" reverse="${3:-0}"
    if [ -z "$mine" ] || [ -z "$theirs" ]; then
        printf '%s' "$C_DIM"
        return
    fi
    if [ "$reverse" = "1" ]; then
        if [ "$mine" -gt "$theirs" ]; then printf '%s' "$C_BOLD$C_GREEN"
        elif [ "$mine" -lt "$theirs" ]; then printf '%s' ""
        else printf '%s' "$C_DIM"
        fi
    else
        if [ "$mine" -lt "$theirs" ]; then printf '%s' "$C_BOLD$C_GREEN"
        elif [ "$mine" -gt "$theirs" ]; then printf '%s' ""
        else printf '%s' "$C_DIM"
        fi
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
# Render: DETAIL table (per-arm, per-metric winner highlighting)
#
# Each scenario gets two rows (control, treatment). For every numeric metric
# we compare the two arms and bold-green the winner (lower for tokens/cost/
# time/turns; higher for PASS). Ties and missing opponents render dim.
#
# Columns: INPUT / OUTPUT / CACHE_R / CACHE_C / COST / TURNS / TIME / PASS.
# TOTAL is intentionally gone — the old "all-billable tokens" sum weighted
# cheap cache reads equally with expensive output tokens, which hid real
# wins and exaggerated regressions. COST is the realistic substitute.
# ---------------------------------------------------------------------------
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "  Per-scenario detail (averages across runs, winners bolded)" "$C_RESET"
printf '%s%s%s\n' "$C_BOLD$C_CYAN" "============================================" "$C_RESET"
echo ""
printf '%sPricing: input $%s/M · output $%s/M · cache read $%s/M · cache write $%s/M (Sonnet 4.6)%s\n' \
    "$C_DIM" "$PRICE_INPUT" "$PRICE_OUTPUT" "$PRICE_CACHE_READ" "$PRICE_CACHE_WRITE" "$C_RESET"
echo ""

detail_header_fmt="%-20s %-10s %10s %10s %11s %10s %10s %7s %7s %7s\n"
printf "$detail_header_fmt" \
    "SCENARIO" "ARM" "INPUT" "OUTPUT" "CACHE_R" "CACHE_C" "COST" "TURNS" "TIME" "PASS"
printf '%s\n' "$(printf '%.0s-' {1..110})"

# Track grand totals across all scenarios where both arms produced data, so we
# can render a final OVERALL row.
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
    # Prints one arm's row with per-cell coloring. Opponent values drive the
    # winner decision for each metric; when the opponent is missing we dim.
    local scenario="$1" arm="$2"
    local runs pass input output cache_r cache_c cost turns duration
    local o_input o_output o_cache_r o_cache_c o_cost o_turns o_duration o_pass o_runs
    local other_arm

    if [ "$arm" = "control" ]; then other_arm="treatment"; else other_arm="control"; fi

    runs=$(agg_get RUNS "$scenario" "$arm"); runs=${runs:-0}
    o_runs=$(agg_get RUNS "$scenario" "$other_arm"); o_runs=${o_runs:-0}

    if [ "$runs" -eq 0 ]; then
        printf "%-20s %s%-10s %10s %10s %11s %10s %10s %7s %7s %7s%s\n" \
            "$scenario" "$C_DIM" "$arm" "-" "-" "-" "-" "-" "-" "-" "-" "$C_RESET"
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

    local c_in c_out c_cr c_cc c_cost c_turns c_time c_pass
    c_in=$(metric_color    "$input"    "$o_input")
    c_out=$(metric_color   "$output"   "$o_output")
    c_cr=$(metric_color    "$cache_r"  "$o_cache_r")
    c_cc=$(metric_color    "$cache_c"  "$o_cache_c")
    c_cost=$(metric_color  "$cost"     "$o_cost")
    c_turns=$(metric_color "$turns"    "$o_turns")
    c_time=$(metric_color  "$duration" "$o_duration")
    # PASS: higher is better; we compare pass rates as pass*other_runs vs other_pass*runs
    # to avoid floating-point, but simpler: just compare fractions via cross-multiplication.
    local pass_cell_color=""
    if [ -n "$o_pass" ] && [ "$o_runs" -gt 0 ]; then
        if [ $((pass * o_runs)) -gt $((o_pass * runs)) ]; then
            pass_cell_color="$C_BOLD$C_GREEN"
        elif [ $((pass * o_runs)) -lt $((o_pass * runs)) ]; then
            pass_cell_color=""
        else
            pass_cell_color="$C_DIM"
        fi
    else
        pass_cell_color="$C_DIM"
    fi
    # But ALSO: within a single arm, a non-100% pass rate is a red flag regardless of
    # the other arm. Show that in the PASS cell color when we'd otherwise render default.
    if [ "$pass" -lt "$runs" ]; then
        pass_cell_color="$C_RED"
    fi

    local time_str
    time_str="$(format_duration "$duration")"

    printf "%-20s %-10s %s%10s%s %s%10s%s %s%11s%s %s%10s%s %s%10s%s %s%7s%s %s%7s%s %s%7s%s\n" \
        "$scenario" "$arm" \
        "$c_in"    "$(format_int "$input")"   "$C_RESET" \
        "$c_out"   "$(format_int "$output")"  "$C_RESET" \
        "$c_cr"    "$(format_int "$cache_r")" "$C_RESET" \
        "$c_cc"    "$(format_int "$cache_c")" "$C_RESET" \
        "$c_cost"  "$(format_cost "$cost")"   "$C_RESET" \
        "$c_turns" "$turns"                   "$C_RESET" \
        "$c_time"  "$time_str"                "$C_RESET" \
        "$pass_cell_color" "${pass}/${runs}"  "$C_RESET"

    # Sub-row: standard deviation for COST and TURNS (dim, only when n>1)
    # Yellow when coefficient of variation (σ/μ) > 50% — high relative spread.
    if [ "$runs" -gt 1 ]; then
        local cost_sd turns_sd cost_sd_color turns_sd_color
        cost_sd=$(agg_get COST_SD "$scenario" "$arm"); cost_sd=${cost_sd:-0}
        turns_sd=$(agg_get TURNS_SD "$scenario" "$arm"); turns_sd=${turns_sd:-0}
        cost_sd_color=$(awk -v sd="$cost_sd" -v m="$cost" \
            'BEGIN { print (m > 0 && sd/m > 0.5) ? 1 : 0 }')
        turns_sd_color=$(awk -v sd="$turns_sd" -v m="$turns" \
            'BEGIN { print (m > 0 && sd/m > 0.5) ? 1 : 0 }')
        local c_cost_sd="$C_DIM" c_turns_sd="$C_DIM"
        [ "$cost_sd_color" = "1" ] && c_cost_sd="$C_YELLOW"
        [ "$turns_sd_color" = "1" ] && c_turns_sd="$C_YELLOW"
        # Pre-format ± values to exact column widths matching the main row.
        # ±$X.XXXX = 9 display cols, pad to 10; ±N = variable, pad to 7.
        local cost_sd_str turns_sd_str
        cost_sd_str="±$(format_cost "$cost_sd")"
        turns_sd_str="±${turns_sd}"
        while [ ${#cost_sd_str} -lt 10 ]; do cost_sd_str=" $cost_sd_str"; done
        while [ ${#turns_sd_str} -lt 7 ]; do turns_sd_str=" $turns_sd_str"; done
        printf "%-20s %-10s %10s %10s %11s %10s %s%s%s %s%s%s %7s %7s\n" \
            "" "" "" "" "" "" \
            "$c_cost_sd" "$cost_sd_str" "$C_RESET" \
            "$c_turns_sd" "$turns_sd_str" "$C_RESET" \
            "" ""
    fi
}

for scenario in "${SCENARIOS[@]}"; do
    print_detail_row "$scenario" "control"
    print_detail_row "$scenario" "treatment"

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
    printf '\n'
done

# OVERALL row: sum of per-scenario averages across the scenarios where both
# arms have data. Uses the same per-metric winner coloring as per-scenario rows.
if [ "$overall_ctrl_runs" -gt 0 ] && [ "$overall_trt_runs" -gt 0 ]; then
    printf '%s\n' "$(printf '%.0s-' {1..110})"

    ov_c_cost_color=$(metric_color "$overall_ctrl_cost" "$overall_trt_cost")
    ov_t_cost_color=$(metric_color "$overall_trt_cost" "$overall_ctrl_cost")
    ov_c_tok_color=$(metric_color "$overall_ctrl_tokens" "$overall_trt_tokens")
    ov_t_tok_color=$(metric_color "$overall_trt_tokens" "$overall_ctrl_tokens")
    ov_c_cached_color=$(metric_color "$overall_ctrl_cached" "$overall_trt_cached")
    ov_t_cached_color=$(metric_color "$overall_trt_cached" "$overall_ctrl_cached")
    ov_c_turns_color=$(metric_color "$overall_ctrl_turns" "$overall_trt_turns")
    ov_t_turns_color=$(metric_color "$overall_trt_turns" "$overall_ctrl_turns")
    ov_c_dur_color=$(metric_color "$overall_ctrl_duration" "$overall_trt_duration")
    ov_t_dur_color=$(metric_color "$overall_trt_duration" "$overall_ctrl_duration")

    # Pass coloring mirrors per-scenario logic: red if below 100% for the arm,
    # otherwise green for the arm with the higher pass rate.
    if [ "$overall_ctrl_pass" -lt "$overall_ctrl_runs" ]; then
        ov_c_pass_color="$C_RED"
    elif [ $((overall_ctrl_pass * overall_trt_runs)) -gt $((overall_trt_pass * overall_ctrl_runs)) ]; then
        ov_c_pass_color="$C_BOLD$C_GREEN"
    else
        ov_c_pass_color=""
    fi
    if [ "$overall_trt_pass" -lt "$overall_trt_runs" ]; then
        ov_t_pass_color="$C_RED"
    elif [ $((overall_trt_pass * overall_ctrl_runs)) -gt $((overall_ctrl_pass * overall_trt_runs)) ]; then
        ov_t_pass_color="$C_BOLD$C_GREEN"
    else
        ov_t_pass_color=""
    fi

    # Render OVERALL as two rows using a combined-token column layout.
    # We reuse the detail_header_fmt but collapse INPUT/OUTPUT into dashes (not
    # meaningful as sums across different workloads); the meaningful totals are
    # CACHE_R/CACHE_C/COST/TURNS/TIME/PASS.
    printf "%-20s %-10s %10s %10s %11s %10s %s%10s%s %s%7s%s %s%7s%s %s%7s%s\n" \
        "OVERALL (sum)" "control" "-" "-" "-" "-" \
        "$ov_c_cost_color"   "$(format_cost "$overall_ctrl_cost")"         "$C_RESET" \
        "$ov_c_turns_color"  "$overall_ctrl_turns"                          "$C_RESET" \
        "$ov_c_dur_color"    "$(format_duration "$overall_ctrl_duration")"  "$C_RESET" \
        "$ov_c_pass_color"   "${overall_ctrl_pass}/${overall_ctrl_runs}"    "$C_RESET"
    printf "%-20s %-10s %10s %10s %11s %10s %s%10s%s %s%7s%s %s%7s%s %s%7s%s\n" \
        "" "treatment" "-" "-" "-" "-" \
        "$ov_t_cost_color"   "$(format_cost "$overall_trt_cost")"           "$C_RESET" \
        "$ov_t_turns_color"  "$overall_trt_turns"                           "$C_RESET" \
        "$ov_t_dur_color"    "$(format_duration "$overall_trt_duration")"   "$C_RESET" \
        "$ov_t_pass_color"   "${overall_trt_pass}/${overall_trt_runs}"      "$C_RESET"

    echo ""

    # Compact delta summary beneath the table: call out the headline deltas in
    # plain English so the reader doesn't have to do the arithmetic.
    cost_delta=$((overall_trt_cost - overall_ctrl_cost))
    cost_pct=$(awk -v d="$cost_delta" -v c="$overall_ctrl_cost" \
        'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')
    turns_delta=$((overall_trt_turns - overall_ctrl_turns))
    turns_pct=$(awk -v d="$turns_delta" -v c="$overall_ctrl_turns" \
        'BEGIN{ if (c==0) print "0.0"; else printf "%.1f", (d/c)*100 }')

    cost_color="$C_DIM"
    if [ "$cost_delta" -lt 0 ]; then cost_color="$C_GREEN"
    elif [ "$cost_delta" -gt 0 ]; then cost_color="$C_RED"
    fi
    turns_color="$C_DIM"
    if [ "$turns_delta" -lt 0 ]; then turns_color="$C_GREEN"
    elif [ "$turns_delta" -gt 0 ]; then turns_color="$C_RED"
    fi

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

    printf '  Δ cost  (trt − ctrl): %s%s (%s%%) ± %s%s\n' \
        "$cost_color" "$(format_cost "$cost_delta")" "$cost_pct" \
        "$(format_cost "$delta_cost_sd")" "$C_RESET"
    printf '  Δ turns (trt − ctrl): %s%s (%s%%) ± %s%s\n' \
        "$turns_color" "$turns_delta" "$turns_pct" \
        "$delta_turns_sd" "$C_RESET"
fi

echo ""

# ---------------------------------------------------------------------------
# Outlier warnings (>2σ from mean) — included in stats but flagged here
# ---------------------------------------------------------------------------
if [ -n "$outlier_warnings" ]; then
    printf '%s%s%s\n' "$C_BOLD$C_YELLOW" "============================================" "$C_RESET"
    printf '%s%s%s\n' "$C_BOLD$C_YELLOW" "  Outlier Warnings (>2σ, included in stats)" "$C_RESET"
    printf '%s%s%s\n' "$C_BOLD$C_YELLOW" "============================================" "$C_RESET"
    echo ""
    printf '%s%-20s %-12s %-30s %-8s %s%s\n' "$C_YELLOW" "SCENARIO" "ARM" "RUN" "METRIC" "VALUE" "$C_RESET"
    printf '%s\n' "$(printf '%.0s-' {1..85})"
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        read -r scn arm run metric val <<< "$line"
        printf '%s%-20s %-12s %-30s %-8s %s%s\n' "$C_YELLOW" "$scn" "$arm" "$run" "$metric" "$val" "$C_RESET"
    done <<< "$outlier_warnings"
    echo ""
fi

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
