# MCP Benchmark Harness

Measures token usage of Claude Code with and without the elmq MCP server on identical Elm coding tasks.

## Setup

### 1. Build the Docker image

```sh
./benchmarks/build.sh
```

This compiles the elmq release binary and builds the `elmq-bench` Docker image with Node, Elm, Claude Code, and the fixture project (rtfeldman/elm-spa-example).

### 2. Authenticate Claude (once)

```sh
docker run -it -v claude-auth:/root/.claude elmq-bench claude setup-token
```

Follow the interactive prompts. Credentials persist in the `claude-auth` Docker volume.

## Running Benchmarks

Run both arms (control then treatment):

```sh
docker run \
  -v claude-auth:/root/.claude \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh
```

Run a single arm:

```sh
docker run \
  -v claude-auth:/root/.claude \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh control
```

```sh
docker run \
  -v claude-auth:/root/.claude \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/run.sh treatment
```

Results accumulate in `benchmarks/results/` (gitignored). Run as many times as you like.

## Analyzing Results

```sh
docker run \
  -v "$(pwd)/benchmarks/results:/bench/results" \
  elmq-bench /bench/analyze.sh
```

Outputs a table comparing control vs treatment across all runs:
- Input/output/cache tokens per scenario
- Tool call counts
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
