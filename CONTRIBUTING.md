---
update-when: build steps, test commands, or dev setup changes
---

# Contributing

## Setup

1. Install [mise](https://mise.jdx.dev/)
2. Clone the repo and install the toolchain:

```sh
git clone https://github.com/user/elmq.git
cd elmq
mise install
```

## Development

```sh
cargo build          # compile
cargo test           # run tests
cargo clippy         # lint
cargo fmt --check    # check formatting
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

- Run `cargo fmt` before committing
- All clippy warnings must be clean (`cargo clippy`)
- Keep functions small and focused
- No unnecessary abstractions — prefer straightforward code
