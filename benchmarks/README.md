# MCP Benchmark Harness

Measures token usage of Claude Code across three configurations on identical Elm coding tasks.

| Arm | What's different |
|-----|-----------------|
| `control` | No MCP server, no plugin — baseline |
| `treatment` | elmq MCP server via `--mcp-config` |
| `treatment-plugin` | elmq Claude Code plugin via `--plugin-dir` (MCP server + SessionStart hook guidance) |

## Setup

### 1. Build the Docker image

```sh
./benchmarks/build.sh
```

This compiles the elmq release binary and builds the `elmq-bench` Docker image with Node, Elm, Claude Code, the plugin, and the fixture project (rtfeldman/elm-spa-example).

### 2. Create auth credentials

Run `claude setup-token` to get an OAuth token, then create `benchmarks/.env`:

```
CLAUDE_CODE_OAUTH_TOKEN=your-token-here
```

This file is gitignored.

## Running Benchmarks

Run all three arms:

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh
```

Run a single arm:

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh control
```

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh treatment
```

```sh
docker run --env-file benchmarks/.env \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh treatment-plugin
```

Arms can run in parallel in separate terminals. Results accumulate in `benchmarks/results/` (gitignored).

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
| 1 | Add a Bookmarks page with routing | `elm_summary`, `elm_get` for pattern discovery |
| 2 | Rename `Article.Body` → `Article.Content` | `elm_edit` mv (project-wide rename) |
| 3 | Extract `Cred` from `Api.elm` into `Api.Cred` | `elm_edit` move-decl |
| 4 | Add a Drafts route with full page wiring | `elm_refs`, `elm_get` for navigation |
| 5 | Add `BookmarkedFeed` variant to `FeedTab` | `elm_edit` variant add |

## Clearing Results

When making significant changes to elmq, clear historical data:

```sh
rm -rf benchmarks/results/*
```
