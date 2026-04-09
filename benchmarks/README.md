# elmq Benchmark Harness

Measures token usage of Claude Code on identical Elm coding tasks. Currently a baseline-only setup — a treatment arm is scheduled to return in the follow-up `benchmark-oracle-arm` change.

| Arm | What's different |
|-----|-----------------|
| `control` | No elmq guidance — Claude works with built-in Read/Write/Edit on the fixture |

Previous `treatment` (MCP server via `--mcp-config`) and `treatment-plugin` (Claude Code plugin) arms were retired alongside the MCP server and the Claude Code plugin in the `drop-mcp-server` change (see `openspec/changes/`). A CLI-oriented oracle treatment arm will be added in the next change.

## Setup

### 1. Build the Docker image

```sh
./benchmarks/build.sh
```

This compiles the elmq release binary and builds the `elmq-bench` Docker image with Node, Elm, Claude Code, and the fixture project (rtfeldman/elm-spa-example).

### 2. Create auth credentials

Run `claude setup-token` to get an OAuth token, then create `benchmarks/.env`:

```
CLAUDE_CODE_OAUTH_TOKEN=your-token-here
```

This file is gitignored.

## Running Benchmarks

Run the control arm:

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh control
```

Each invocation creates a new timestamped directory under `benchmarks/results/control/` (gitignored). Results accumulate across runs; `analyze.sh` averages per-scenario metrics across every timestamped run in each arm directory, so manual one-at-a-time data collection is the supported workflow.

## Analyzing Results

```sh
docker run \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/analyze.sh
```

Outputs:
- Per-scenario token averages (input, output, cache) for each arm
- Tool call counts and per-scenario tool breakdown
- Verification pass rates
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
