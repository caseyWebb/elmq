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

## Phase 1: Read Tools

- `list-modules` — scan a directory for Elm modules
- `get-declaration` — extract the full source text of a declaration by name

## Phase 2: Write Tools

- `upsert-declaration` — replace or append a top-level declaration (full source)
- `edit-declaration` — apply old/new diff within a declaration
- `add-import` / `remove-import`
- `set-exposing` / `set-module`
- Round-trip formatting preservation (comments, whitespace)

## Phase 3: MCP Server

- Expose read/write tools as MCP tools via stdio transport
- Tool descriptions optimized for token efficiency
- Compact output as default for MCP responses

## Phase 4: Multi-File Operations

- Project-aware mode (elm.json, source-directories)
- Cross-file queries (who exposes X, who imports Y)
- Propagating edits (add type constructor → update case expressions at all use sites)

## Phase 5: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
