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

## Phase 6: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
