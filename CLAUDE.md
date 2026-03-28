---
update-when: architecture, commands, conventions, or build instructions change
---

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is elmq?

A Rust CLI and MCP server for querying and editing Elm files — like jq for Elm. Uses tree-sitter-elm for parsing. Supports reading (`list`, `get`), writing (`set`, `patch`, `rm`, `import`, `expose`/`unexpose`), and project-wide operations (`mv` — rename a module and update all references; `refs` — find all references to a module or declaration). The MCP server (`elmq mcp`) exposes 5 consolidated tools over stdio transport.

## Build & Test Commands

```sh
cargo build --release --locked   # compile release binary
cargo test                       # run all tests
cargo clippy -- -D warnings      # lint (must be warning-free)
cargo fmt --check                # check formatting
cargo fmt                        # auto-format
cargo install --path .           # install locally
```

To run a single test: `cargo test <name>`

Rust toolchain is pinned in `rust-toolchain.toml` (rustup installs it automatically).

## Architecture

- **`src/lib.rs`** — Public types (`FileSummary`, `Declaration`, `DeclarationKind`). The library crate root.
- **`src/parser.rs`** — All tree-sitter-elm parsing logic: `parse()` returns a tree, `extract_summary()` walks it to produce a `FileSummary`, `find_declaration()` looks up by name, `extract_declaration_name()` parses a name from source text. Unit tests live here inline.
- **`src/project.rs`** — Project discovery: find `elm.json`, parse `source-directories`, resolve module names from file paths, enumerate all `.elm` files. Used by `mv` and `refs`.
- **`src/writer.rs`** — All write operations: `upsert_declaration()`, `patch_declaration()`, `remove_declaration()`, `add_import()`, `remove_import()`, `expose()`, `unexpose()`, `rename_module_references()`, `rename_module_declaration()`, plus `atomic_write()` for safe file writes.
- **`src/cli.rs`** — clap derive definitions (`Cli`, `Command`, `ImportCommand`, `Format`).
- **`src/refs.rs`** — Project-wide reference finding: `find_refs()` locates all references to a module or declaration across the project, resolving qualified, aliased, and explicitly-exposed references via tree-sitter.
- **`src/mcp.rs`** — MCP stdio server: `ElmqServer` handler with 5 tools (`elm_summary`, `elm_get`, `elm_edit`, `elm_module`, `elm_refs`), parameter types, and `run_mcp_server()` entry point. Uses `rmcp` SDK.
- **`src/main.rs`** — Thin CLI entry point. Reads file, calls parser/writer, formats output. The `mv` and `refs` commands use `project.rs` for multi-file operations. The `mcp` subcommand creates a tokio runtime and starts the MCP server. Not part of the library crate.

The library (`lib.rs` + `parser.rs` + `writer.rs` + `project.rs` + `refs.rs`) is fully testable without the CLI binary.

- **`tests/`** — Integration tests per command: `get.rs`, `set.rs`, `patch.rs`, `rm.rs`, `import.rs`, `expose.rs`, `mv.rs`, `refs.rs`, `mcp.rs`.

## Conventions

- Requires Rust edition 2024 (nightly features like let-chains are used)
- Run `cargo fmt` before committing
- PR titles must follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced by CI, used for squash merge commit messages)
- Test fixtures go in `test-fixtures/`; add/update sample `.elm` files when adding parser features
- `openspec/` contains spec-driven development artifacts (changes and specs)
