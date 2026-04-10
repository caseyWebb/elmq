# elmq Benchmark Harness

Measures token usage of Claude Code on identical Elm coding tasks. Two arms so we can compare "Claude without elmq guidance" against "Claude with elmq guidance delivered via system prompt." This answers **Q1: does elmq save tokens on Elm editing tasks, given Claude knows how to use it?** — the ceiling on what any delivery mechanism could achieve.

| Arm | What's different |
|-----|-----------------|
| `control` | No elmq guidance — Claude works with built-in Read/Write/Edit/Grep on the fixture |
| `treatment` | elmq CLI guidance injected via a second `--append-system-prompt-file` pointing at `benchmarks/elmq-guide.md` (the "oracle" arm — no MCP, no plugin, no hook) |

The treatment arm is deliberately the simplest possible delivery mechanism. Q2 ("which delivery mechanism is best?" — skill vs. hook vs. MCP vs. something else) is a separate, future experiment that can only be evaluated once Q1 has an answer.

## Setup

### Create auth credentials

Run `claude setup-token` to get an OAuth token, then create `benchmarks/.env`:

```
CLAUDE_CODE_OAUTH_TOKEN=your-token-here
```

This file is gitignored.

The Docker image is built automatically on every `./benchmarks/run.sh` invocation (Docker's layer cache makes this cheap when sources are unchanged; the Rust compile and `COPY benchmarks/*` layers only re-run when their inputs change). If you want to build manually — for example, to surface compile errors before a benchmark run — use:

```sh
./benchmarks/build.sh
```

This compiles the elmq release binary and builds the `elmq-bench` image with Node, Elm, Claude Code, and the fixture project (rtfeldman/elm-spa-example).

## Running Benchmarks

```sh
./benchmarks/run.sh                     # 1 control + 1 treatment in parallel
./benchmarks/run.sh -n 3                # 3 of each (6 parallel runs)
./benchmarks/run.sh control             # 1 control only
./benchmarks/run.sh treatment -n 5      # 5 treatments in parallel
```

Each run is scoped as `benchmarks/results/<arm>/<TIMESTAMP>-<arm>-<index>/` and its stdout/stderr goes to `benchmarks/results/logs/<TIMESTAMP>-<arm>-<index>.log`. All timestamped directories in each arm accumulate across batches; `analyze.sh` averages across every run it finds, so running `./benchmarks/run.sh -n 3` today and again tomorrow gives you 6 samples per arm to compare.

Rate limits and system resources cap the practical value of `N`. Start with `-n 2` or `-n 3`; very large values will hit Anthropic rate limits and/or saturate Docker.

### Direct invocation (advanced)

You can still invoke `run.sh` inside the container directly if you want to bypass the wrapper (e.g. for a one-off run without parallelization). The wrapper passes a `BENCHMARK_RUN_ID` environment variable to scope the results dir; if you omit it, `run.sh` falls back to a `date`-based timestamp.

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh control
```

## Analyzing Results

```sh
docker run \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/analyze.sh
```

Output is GitHub-flavored markdown with emoji indicators (🟢 winner, 🔴 loser, ✅/❌ pass/fail). When run interactively with `glow` installed, output is auto-piped through `glow` for styled terminal rendering. Pipe to a file for raw markdown (e.g. `> BENCHMARKS.md`).

Contents:
- Per-scenario token averages (input, output, cache) with winner highlighting
- Cost and turns with standard deviation (±) and high-variance warnings (⚠️)
- Overall summary with deltas (treatment − control)
- Outlier warnings (>2σ)
- Tool call counts, breakdown, and per-call details
- Broken-run filtering (if scenario N fails, scenarios N+1..5 are excluded)

## Scenarios

Five sequential tasks, each building on the previous result:

| # | Scenario | elmq Advantage |
|---|----------|---------------|
| 1 | Add a Bookmarks page with routing | `elmq list`, `elmq get` for pattern discovery |
| 2 | Rename `Article.Body` → `Article.Content` | `elmq mv` (project-wide rename) |
| 3 | Extract `Cred` from `Api.elm` into `Api.Cred` | `elmq move-decl` |
| 4 | Add a Drafts route with full page wiring | `elmq refs`, `elmq get` for navigation |
| 5 | Add `BookmarkedFeed` variant to `FeedTab` | `elmq variant add` |

## Clearing Results

When making significant changes to elmq, clear historical data:

```sh
rm -rf benchmarks/results/*
```
