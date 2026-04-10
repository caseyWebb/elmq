---
update-when: build steps, test commands, or dev setup changes
---

# Contributing

## Setup

1. Install [Rust](https://rustup.rs/) via rustup
2. Clone the repo (rustup will install the pinned toolchain automatically):

```sh
git clone https://github.com/caseyWebb/elmq.git
cd elmq
```

## Development

```sh
cargo build --release --locked   # compile release binary
cargo test                       # run tests
cargo clippy -- -D warnings      # lint
cargo fmt                        # auto-format
cargo fmt --check                # check formatting
```

## Project Structure

```
src/
├── main.rs       # CLI entry point, output formatting
├── cli.rs        # clap argument definitions
├── lib.rs        # public types (Declaration, DeclarationKind)
├── guide.md      # agent integration guide (compiled into binary via include_str!)
├── imports.rs    # import context abstraction (resolve, emit, merge imports)
├── move_decl.rs  # move declarations between modules with body rewriting
├── parser.rs     # tree-sitter-elm parsing and declaration extraction
├── project.rs    # project discovery (elm.json, source-directories, module resolution)
├── refs.rs       # project-wide reference finding (qualified, aliased, exposed)
└── writer.rs     # write operations (upsert, patch, rm, imports, module, rename)
```

- `lib.rs` + `parser.rs` + `imports.rs` + `writer.rs` + `project.rs` + `refs.rs` + `move_decl.rs` contain all core logic, testable without the CLI
- `main.rs` is a thin wrapper that wires CLI args to library functions

## Testing

Unit tests live alongside the code in `parser.rs`, `imports.rs`, `writer.rs`, `project.rs`, and `refs.rs`. Integration tests are in `tests/` with one file per command (`get.rs`, `set.rs`, `patch.rs`, `rm.rs`, `import.rs`, `expose.rs`, `mv.rs`, `refs.rs`, `rename.rs`, `move_decl.rs`, `variant.rs`, `grep.rs`, `guide.rs`). Run with `cargo test`.

Test fixtures are in `test-fixtures/`. When adding parser features, add or update the sample Elm files there and write corresponding tests. Integration tests use `tempfile` to create temporary copies for write operations.

## Code Style

- Run `cargo fmt` before committing
- All clippy warnings must be clean (`cargo clippy -- -D warnings`)
- PR titles must follow [Conventional Commits](https://www.conventionalcommits.org/) (e.g. `feat: add parser option`, `fix: handle empty files`) — PRs are squash-merged using the title as the commit message
- Keep functions small and focused
- No unnecessary abstractions — prefer straightforward code
