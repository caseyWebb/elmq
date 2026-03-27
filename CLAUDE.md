---
update-when: architecture, commands, conventions, or build instructions change
---

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is elmq?

A Rust CLI and MCP server for querying and editing Elm files — like jq for Elm. Uses tree-sitter-elm for parsing. Supports reading (`list`, `get`) and writing (`set`, `patch`, `rm`, `import`, `expose`/`unexpose`) Elm declarations. The MCP server (`elmq mcp`) exposes 4 consolidated tools over stdio transport.

## Build & Test Commands

All commands go through mise:

```sh
mise run build       # compile release binary
mise run test        # run all tests
mise run lint        # clippy (must be warning-free)
mise run fmt:check   # check formatting
mise run fmt         # auto-format
mise run check       # all of the above
mise run install     # install locally
```

To run a single test: `cargo test <name>`

Rust toolchain is managed via mise (`mise install` to set up).

## Architecture

- **`src/lib.rs`** — Public types (`FileSummary`, `Declaration`, `DeclarationKind`). The library crate root.
- **`src/parser.rs`** — All tree-sitter-elm parsing logic: `parse()` returns a tree, `extract_summary()` walks it to produce a `FileSummary`, `find_declaration()` looks up by name, `extract_declaration_name()` parses a name from source text. Unit tests live here inline.
- **`src/writer.rs`** — All write operations: `upsert_declaration()`, `patch_declaration()`, `remove_declaration()`, `add_import()`, `remove_import()`, `expose()`, `unexpose()`, plus `atomic_write()` for safe file writes.
- **`src/cli.rs`** — clap derive definitions (`Cli`, `Command`, `ImportCommand`, `Format`).
- **`src/mcp.rs`** — MCP stdio server: `ElmqServer` handler with 4 tools (`elm_summary`, `elm_get`, `elm_edit`, `elm_module`), parameter types, and `run_mcp_server()` entry point. Uses `rmcp` SDK.
- **`src/main.rs`** — Thin CLI entry point. Reads file, calls parser/writer, formats output. The `mcp` subcommand creates a tokio runtime and starts the MCP server. Not part of the library crate.

The library (`lib.rs` + `parser.rs` + `writer.rs`) is fully testable without the CLI binary.

- **`tests/`** — Integration tests per command: `get.rs`, `set.rs`, `patch.rs`, `rm.rs`, `import.rs`, `expose.rs`, `mcp.rs`.

## Conventions

- Requires Rust edition 2024 (nightly features like let-chains are used)
- Run `mise run fmt` before committing
- PR titles must follow [Conventional Commits](https://www.conventionalcommits.org/) (enforced by CI, used for squash merge commit messages)
- Test fixtures go in `test-fixtures/`; add/update sample `.elm` files when adding parser features
- `openspec/` contains spec-driven development artifacts (changes and specs)
