---
update-when: features are completed or new phases are planned
---

# elmq Roadmap

Query and edit Elm files — like jq for Elm. A next-gen LSP for agents and scripts, optimized for token efficiency.

## Phase 0: Foundation ✓

- Rust CLI scaffold with mise-managed toolchain
- tree-sitter-elm parsing
- `elmq list <file>` — basic declaration listing
- Compact and JSON output formats

## Phase 0.5: File Summary ✓

- Evolve `list` into a grouped file summary (module, imports, types, functions, ports)
- Absorb `list-imports` and `list-exposing` into `list`
- Add `--docs` flag for inline doc comments
- Omit empty sections, remove redundant per-declaration exposed status

## Phase 1: Read Tools ✓

- ~~`list-modules`~~ — deferred (agents find files effectively; not needed yet)
- `get` — extract the full source text of a declaration by name

## Phase 2: Write Tools ✓

- `set` — upsert a top-level declaration (full source from stdin, name parsed or `--name` override)
- `patch` — surgical `--old`/`--new` find-and-replace scoped to a declaration
- `rm` — remove a declaration (including doc comment and type annotation)
- `import add` / `import remove` — manage import clauses
- `expose` / `unexpose` — granularly manage module exposing list
- Round-trip formatting preservation (comments, whitespace outside edited region)
- Atomic file writes (write-to-temp, rename-over-original)

## Phase 3: MCP Server (retired)

Built and later removed. The elmq MCP stdio server (`elmq mcp`, `src/mcp.rs`, `rmcp` SDK) was retired as part of the `drop-mcp-server` change because a Claude Code upstream bug (anthropics/claude-code#24762, #36914) prevents local stdio MCP servers from registering their tools in Claude Code sessions, making the entire surface unreachable from the only harness that matters today. The benchmark harness in `benchmarks/` was reshaped to answer "does elmq save tokens?" via a CLI-oriented oracle arm instead of via MCP.

LLM-harness packaging (MCP, skill, plugin, npm) will return as a dedicated phase once the benchmark proves the token-savings thesis and the data informs which packaging model is worth building.

## Phase 4: Multi-File Operations ✓

- Project-aware mode (elm.json, source-directories) ✓
- `mv` — rename a module and update imports and qualified references across the project ✓
- `refs` — find all references to a module or declaration across the project ✓
- `rename` — rename a declaration and update all references across the project ✓
- `move-decl` — move declarations between modules with import-aware body rewriting, automatic helper detection, and project-wide reference updates ✓
- `variant add`/`variant rm` — add or remove type constructors with project-wide case expression propagation ✓

## Phase 5: Benchmarks and validation (in progress)

The benchmark harness in `benchmarks/` runs Claude Code against `rtfeldman/elm-spa-example` on five sequential Elm editing scenarios (add feature, rename module, extract module, add route, add variant). Two arms:

- **`control`** — Claude works with built-in Read/Write/Edit/Grep, no elmq guidance
- **`treatment`** — Claude gets elmq CLI guidance via a system-prompt injection (`--append-system-prompt-file benchmarks/elmq-guide.md`)

The thesis under test (**Q1**): given Claude knows how to use elmq, does it actually save tokens on Elm editing tasks? A positive answer unlocks the follow-up question (**Q2**): which delivery mechanism (plugin, skill, hook, MCP, etc.) is worth investing in to deliver elmq's guidance at runtime?

Planned follow-ups once Q1 has preliminary data:

- `benchmark-independent-scenarios` — replace sequential scenarios with git-tag-based reference checkpoints per scenario (option B from the explore session), eliminating survivorship bias in multi-run averages
- Q2 packaging experiments — skill arm, hook arm, maybe reintroduce MCP if the upstream Claude Code stdio registration bug is fixed

## Phase 6: v2 — guide and CLI improvements from benchmark findings

The first benchmark run (5 scenarios × 3 runs, control vs. treatment with `elmq-guide.md` v1) showed treatment losing overall by ~61% (3.26M vs 2.02M tokens) while winning decisively on the one scenario with a perfect one-shot command (`02-rename-module`, −84% via `elmq mv`). Analysis of the tool-call traces surfaced a set of guide rules and CLI affordances that should flip the losing scenarios. Each bullet below will get its own OpenSpec change as we pick it up.

### 6a. Guide v2 (`benchmarks/elmq-guide.md`)

Rules to add, in priority order:

- **Progressive exploration.** Use `find`/`Glob` for project layout. `elmq list` only on files about to be read or modified. `elmq get` only the specific declarations needed. No reconnaissance sweeps.
- **Prefer `patch` over `set`.** `patch` is the default for any fragment edit (new case branch, record field, list item, parameter, body tweak). `set` is reserved for whole-body rewrites where the new body bears no resemblance to the old. Explain the cost: `set` emits the whole declaration as output tokens; `patch` emits only the delta.
- **Task → command decision table** near the top: rename module → `mv`; extract decls into submodule → `move-decl` (never manual copy + rm + import fixup); rename decl → `rename`; add/remove variant → `variant add`/`variant rm` (never `set`); modify inside body → `patch`; replace entire decl → `set`; find references → `refs`.
- **After `variant add`, re-`get` before `patch`.** Inserted placeholders are `Debug.todo "<Variant>"`; don't guess.
- **Trust exit codes.** Exit 0 means success. Do not re-read, `list`, `get`, `cat`, or otherwise verify after a successful write. `elm make` at the end is the single source of truth.
- **No repeated `cd`.** Bash cwd persists across calls.
- **Batch read-only queries.** Chain `list`/`get`/`refs` calls with `&&` in a single Bash invocation when several are needed at once.
- **Plan, then execute.** After targeted reads, write a 3–5 line plan (files × decls × operation) and stop exploring.

### 6b. CLI improvements (ordered by expected impact)

- **`elmq new <Module.Path> [--expose ...]`** — create a new module file with correct header. Eliminates the `Write`→`cat`→`touch`→`python`→`node` file-creation loop observed in both arms on 01/03/04.
- **Multi-arg read commands** — `elmq list file1 file2 ...`, `elmq get file decl1 decl2 ...`, `elmq refs Module.decl1 Module.decl2 ...`. Output uses a per-item header delimiter.
- **Multi-arg write commands (all-or-nothing)** — `elmq expose file sym1 sym2 ...`, `elmq unexpose file sym1 sym2 ...`, `elmq import add file 'Mod1' 'Mod2 as M2' ...`, `elmq import remove file Mod1 Mod2 ...`, and `elmq move-decl src/Api.elm decl1 decl2 decl3 src/Api/Cred.elm` (multi-decl extract). Any failure rolls back the whole batch and exits non-zero so the "trust exit codes" guide rule stays honest.
- **Better `patch` match-failure errors.** On `--old` mismatch, show the closest candidate region in the file so the agent can correct without re-`get`ing and guessing.
- **`elmq set` soft-warning when it looks like a `patch`.** If the new content differs from the existing decl by only a small insertion/replacement, emit a stderr hint: `consider 'elmq patch' — only N lines differ`. Non-blocking.
- **Idempotent `expose`/`unexpose`.** `expose Cred(..)` when `Cred` is already exposed should upgrade cleanly; no error, no duplicate. Reverse for `unexpose`.
- **`elmq refs --unused <file>`** — list imports in a file no longer referenced. Helps post-extraction cleanup.

Multi-arg `patch` and `set` are deferred until the simpler read/write batching proves the transactional pattern.

### 6c. Benchmark harness fix

- **Investigate new-file-creation flakiness.** Both arms fall into a `Write` → heredoc → `python` → `node` loop when creating a new `.elm` file (observed on 01, 03, 04). Either the sandbox has a quirk making writes look like they failed or the agent is being misled by a verification step. This is noise polluting both arms' totals and should be fixed before publishing Q1 numbers.

### Sequencing

1. Ship guide v2 first (no code changes) and re-run the benchmark to isolate guide-only gains.
2. Ship `elmq new` and multi-arg read commands; re-run.
3. Ship multi-arg write commands (`expose`/`unexpose`/`import`/`move-decl`), idempotent expose, and `patch` error improvements; re-run.
4. Investigate benchmark sandbox file-creation flakiness in parallel — ideally before step 1 so the numbers are clean.

## Phase 7: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
