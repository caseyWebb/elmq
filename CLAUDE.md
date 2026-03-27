# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is elmq?

A Rust CLI (and future MCP server) for querying and editing Elm files — like jq for Elm. Uses tree-sitter-elm for parsing. Early stage; currently supports `elmq list` to summarize declarations in an Elm file.

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
- **`src/parser.rs`** — All tree-sitter-elm parsing logic: `parse()` returns a tree, `extract_summary()` walks it to produce a `FileSummary`. Unit tests live here inline.
- **`src/cli.rs`** — clap derive definitions (`Cli`, `Command`, `Format`).
- **`src/main.rs`** — Thin CLI entry point. Reads file, calls parser, formats output (compact or JSON). Not part of the library crate.

The library (`lib.rs` + `parser.rs`) is fully testable without the CLI binary.

## Conventions

- Requires Rust edition 2024 (nightly features like let-chains are used)
- Run `cargo fmt` before committing
- Test fixtures go in `test-fixtures/`; add/update sample `.elm` files when adding parser features
- `openspec/` contains spec-driven development artifacts (changes and specs)
