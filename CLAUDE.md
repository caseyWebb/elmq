---
update-when: architecture, commands, conventions, or build instructions change
---

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is elmq?

A Rust CLI (and future MCP server) for querying and editing Elm files — like jq for Elm. Uses tree-sitter-elm for parsing. Supports reading (`list`, `get`) and writing (`set`, `patch`, `rm`, `import`, `expose`/`unexpose`) Elm declarations.

## Build & Test Commands

```sh
cargo build          # compile
cargo test           # run all tests
cargo test <name>    # run a single test by name
cargo clippy         # lint (must be warning-free)
cargo fmt --check    # check formatting
cargo fmt            # auto-format
cargo install --path . # install locally
```

Rust toolchain is managed via mise (`mise install` to set up).

## Architecture

- **`src/lib.rs`** — Public types (`FileSummary`, `Declaration`, `DeclarationKind`). The library crate root.
- **`src/parser.rs`** — All tree-sitter-elm parsing logic: `parse()` returns a tree, `extract_summary()` walks it to produce a `FileSummary`, `find_declaration()` looks up by name, `extract_declaration_name()` parses a name from source text. Unit tests live here inline.
- **`src/writer.rs`** — All write operations: `upsert_declaration()`, `patch_declaration()`, `remove_declaration()`, `add_import()`, `remove_import()`, `expose()`, `unexpose()`, plus `atomic_write()` for safe file writes.
- **`src/cli.rs`** — clap derive definitions (`Cli`, `Command`, `ImportCommand`, `Format`).
- **`src/main.rs`** — Thin CLI entry point. Reads file, calls parser/writer, formats output. Not part of the library crate.

The library (`lib.rs` + `parser.rs` + `writer.rs`) is fully testable without the CLI binary.

- **`tests/`** — Integration tests per command: `get.rs`, `set.rs`, `patch.rs`, `rm.rs`, `import.rs`, `expose.rs`.

## Conventions

- Requires Rust edition 2024 (nightly features like let-chains are used)
- Run `cargo fmt` before committing
- Test fixtures go in `test-fixtures/`; add/update sample `.elm` files when adding parser features
- `openspec/` contains spec-driven development artifacts (changes and specs)
