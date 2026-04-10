## ADDED Requirements

### Requirement: Results directory names must not trigger Claude Code path-safety heuristics

The benchmark harness SHALL generate per-run results directory names that do not contain characters Claude Code's `Write`-tool path-safety heuristics classify as "suspicious Windows path patterns." In particular, the directory name SHALL NOT contain a colon (`:`) adjacent to an uppercase ASCII letter, since that sequence resembles a Windows drive letter (`C:\…`) and is silently rejected by the tool as a permission check that cannot be approved in a non-interactive benchmark run.

#### Scenario: Timestamp format is colonless

- **WHEN** `benchmarks/run.sh` computes its default `TIMESTAMP` variable via `date`
- **THEN** the format string SHALL produce a value with no `:` characters
- **AND** the resulting value SHALL still be lexicographically sortable as a string so that `ls benchmarks/results/<arm>/` orders runs chronologically

#### Scenario: Wrapper timestamp format matches run.sh format

- **WHEN** `benchmarks/run.sh` computes `TIMESTAMP_BASE` for a parallel batch
- **THEN** the format SHALL use no colons, with the same ordering guarantees as the container-mode timestamp
- **AND** the `BENCHMARK_RUN_ID` environment variable passed to each container SHALL inherit that colon-free format

### Requirement: Fix is verified by observing a clean Write tool_result

The change SHALL be verified by running at least one scenario in at least one arm and confirming, by direct inspection of the resulting `session.json`, that the first `Write` tool call returns a `tool_result` with `is_error` unset or `false`. A passing verify.status is NOT sufficient proof.

#### Scenario: Smoke-test confirms Write tool is unblocked

- **WHEN** a fresh run with the new timestamp format produces `session.json` files
- **THEN** `jq` filtering for `tool_result` records with `is_error==true` SHALL NOT find any records containing the string "suspicious Windows path pattern"
- **AND** scenarios that create new `.elm` files SHALL show a single `Write` call followed by the expected elmq edit sequence, not a Bash fallback loop

### Requirement: Historical results are preserved

Existing directories under `benchmarks/results/` SHALL NOT be renamed, moved, or deleted as part of this change. They are immutable historical data from past benchmark runs. The new format applies only to future runs.

#### Scenario: analyze.sh processes mixed-format directories

- **WHEN** `benchmarks/analyze.sh` is run against a `results/` tree containing both old and new format directories
- **THEN** it SHALL process both without error and include both in per-arm averages
- **AND** the broken-run filter, tool-call breakdown, and winners table SHALL all operate correctly across the mixed set
