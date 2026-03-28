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

## Phase 3: MCP Server ✓

- `elmq mcp` — stdio MCP server using `rmcp` SDK
- 4 consolidated tools: `elm_summary`, `elm_get`, `elm_edit`, `elm_refs`
- Tool descriptions optimized for token efficiency
- Compact output as default for MCP responses

## Phase 4: Multi-File Operations ✓

- Project-aware mode (elm.json, source-directories) ✓
- `mv` — rename a module and update imports and qualified references across the project ✓
- `refs` — find all references to a module or declaration across the project ✓
- `rename` — rename a declaration and update all references across the project ✓
- `move-decl` — move declarations between modules with import-aware body rewriting, automatic helper detection, and project-wide reference updates ✓
- Propagating edits (add type constructor → update case expressions at all use sites)

## Phase 5: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
