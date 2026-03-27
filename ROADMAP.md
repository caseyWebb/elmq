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

## Phase 3: MCP Server

- Expose read/write tools as MCP tools via stdio transport
- Tool descriptions optimized for token efficiency
- Compact output as default for MCP responses

## Phase 4: Multi-File Operations

- Project-aware mode (elm.json, source-directories)
- `mv` — rename a module and update import sites across the project
- Cross-file queries (who exposes X, who imports Y)
- Propagating edits (add type constructor → update case expressions at all use sites)

## Phase 5: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
