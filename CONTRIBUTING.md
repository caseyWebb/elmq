---
update-when: build steps, test commands, or dev setup changes
---

# Contributing

## Setup

1. Install [mise](https://mise.jdx.dev/)
2. Clone the repo and install the toolchain:

```sh
git clone https://github.com/caseyWebb/elmq.git
cd elmq
mise install
```

## Development

```sh
mise run build       # compile release binary
mise run test        # run tests
mise run lint        # clippy
mise run fmt         # auto-format
mise run fmt:check   # check formatting
mise run check       # all checks (fmt, lint, test)
```

## Project Structure

```
src/
├── main.rs       # CLI entry point, output formatting, MCP startup
├── cli.rs        # clap argument definitions
├── lib.rs        # public types (Declaration, DeclarationKind)
├── mcp.rs        # MCP stdio server (rmcp SDK, 4 tools)
├── parser.rs     # tree-sitter-elm parsing and declaration extraction
└── writer.rs     # write operations (upsert, patch, rm, imports, module)
```

- `lib.rs` + `parser.rs` + `writer.rs` contain all core logic, testable without the CLI
- `main.rs` is a thin wrapper that wires CLI args to library functions

## Testing

Unit tests live alongside the code in `parser.rs`. Integration tests are in `tests/` with one file per command (`get.rs`, `set.rs`, `patch.rs`, `rm.rs`, `import.rs`, `expose.rs`, `mcp.rs`). Run with `cargo test`.

Test fixtures are in `test-fixtures/`. When adding parser features, add or update the sample Elm files there and write corresponding tests. Integration tests use `tempfile` to create temporary copies for write operations.

## Code Style

- Run `mise run fmt` before committing
- All clippy warnings must be clean (`mise run lint`)
- PR titles must follow [Conventional Commits](https://www.conventionalcommits.org/) (e.g. `feat: add parser option`, `fix: handle empty files`) — PRs are squash-merged using the title as the commit message
- Keep functions small and focused
- No unnecessary abstractions — prefer straightforward code
