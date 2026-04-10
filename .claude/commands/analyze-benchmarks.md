---
description: Analyze benchmark results for token inefficiencies and optimization opportunities
---

Analyze the elmq benchmark results to identify token waste and optimization opportunities.

**Input**: $ARGUMENTS — optional filter: `treatment`, `control`, a scenario name like `01-add-feature`, or a full run path. If omitted, analyze all available results.

## Data location

Results live in `benchmarks/results/{control,treatment}/<run-id>/<scenario>/`. Each scenario directory contains:
- `session.json` — stream-JSON (one JSON object per line). Key line types:
  - `type=system` (first line): init metadata
  - `type=assistant`: `.message.content[]` has `tool_use` blocks (`{type, id, name, input}`) and `.message.usage` has per-turn token counts (`input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`)
  - `type=user`: `.message.content[]` has `tool_result` blocks (`{type, tool_use_id, content}`) — the content length is the payload size returned to the model
  - `type=result` (last line): aggregate `usage`, `total_cost_usd`, `duration_ms`, `num_turns`
- `verify.status` — `PASSED` or `FAILED`
- `diff.patch` — git diff of changes made

## Analysis steps

### 0. Run the analysis script

First, run `./benchmarks/analyze.sh ./benchmarks/results` to get deterministic statistics (markdown format): per-scenario averages, standard deviations, outlier warnings (>2σ), and overall deltas with uncertainty. Include this output in your report as the quantitative baseline — do not re-derive these numbers manually.

### 1. Inventory available runs

Use Bash to list what's in `benchmarks/results/`. Apply the user's filter from $ARGUMENTS if provided. Summarize: how many runs per arm, which scenarios have data.

### 2. Extract per-scenario metrics

For each session.json, extract using `jq`:
- **Aggregate**: total cost, turns, duration, token breakdown (from `type=result` line)
- **Tool call sequence**: ordered list of `(name, id, input_summary)` from assistant messages
- **Tool result sizes**: for each `tool_use_id` in user messages, capture `content | tostring | length`
- **Per-turn token growth**: from each assistant message's `.message.usage`, track how `cache_creation_input_tokens` grows (this is context accumulation)

### 3. Detect inefficiency patterns

Analyze the extracted data for these specific patterns. For each pattern found, note the scenario, arm, turn number, and estimated token cost:

#### a) Redundant file reads
Files read (via Read tool or `cat`/`head` in Bash) multiple times without an intervening edit/write to that file. Each re-read forces the full file content back into context.
- jq query: extract Read tool file_path and Bash commands containing `cat`/`elmq get`/`elmq list` targeting the same file, track edit/write/set/patch operations between them.

#### b) Failed-then-retry cycles
Tool calls that produce an error result followed by a retry. Each failed attempt adds to context without progress. Look for:
- `tool_result` with `is_error: true` or content containing common error strings
- The same tool called again with similar arguments shortly after

#### c) Oversized tool results
Tool results returning large payloads (>3000 chars). These inflate context on every subsequent turn. Flag:
- Read calls on entire files when only a specific function was needed
- Bash commands that dump large output (e.g., full `elm make` error output, `cat` of large files)
- `elmq list` or `elmq get` calls returning more than needed

#### d) Unnecessary exploration
Tool calls that gather information not reflected in the final diff. Compare:
- Files read/grepped/globbed during the session vs files actually modified in `diff.patch`
- High exploration-to-edit ratios suggest the agent is searching broadly instead of navigating directly

#### e) Re-reading after writes
Reading back a file immediately after writing/editing it without a compile error in between. This is wasted context — the agent already knows what it wrote.

#### f) Context accumulation rate
Track `cache_creation_input_tokens` per turn. Sharp jumps indicate large tool results entering context. Identify which tool calls cause the biggest context jumps.

#### g) Wasted turns on failed verification
If the scenario ends with `FAILED` status, all tokens were wasted. Note total cost of failed runs.

### 4. Cross-arm comparison

For scenarios with both control and treatment data, compare:
- Total tool calls and which tools dominate
- Token efficiency: cost per "useful" tool call (ones that contributed to the diff)
- Whether elmq commands are being used effectively or if the agent falls back to Read/Edit patterns despite having elmq

### 5. Produce report

Structure your output as:

```
## Benchmark Efficiency Analysis

### Overview
- Runs analyzed: ...
- Total cost: control $X.XX, treatment $X.XX
- Pass rates: ...

### Top Inefficiencies (by estimated token waste)

1. **[Pattern name]** — scenario, arm
   - What happened: ...
   - Estimated waste: ~N tokens (cache_creation cost)
   - Fix: ...

2. ...

### Per-Scenario Breakdown
For each scenario, show:
- Tool call count and sequence summary
- Largest tool results (top 3 by content length)
- Redundant operations found
- Context growth curve (which turns caused biggest jumps)

### Recommendations for elmq-guide.md
Concrete changes to the treatment guidance that would reduce token usage:
- Guidance to add/modify
- Anti-patterns to warn against
- Missing elmq commands that could replace expensive Read/Edit sequences

### Recommendations for Scenario Prompts
Changes to scenario prompt.md files that would help the agent be more efficient.
```

## Important notes

- Use `jq` for all JSON parsing — session.json files can be large
- Strip workdir paths (`/bench/results/.../workdir/` or absolute fixture paths) from output for readability
- When computing "waste", use Sonnet pricing: input $3/M, output $15/M, cache_read $0.30/M, cache_write $3.75/M
- Focus on actionable findings — skip minor issues, highlight the biggest token sinks
- If only one arm has data, still analyze it for inefficiencies; just skip the cross-arm comparison
