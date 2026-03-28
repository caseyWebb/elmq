#!/usr/bin/env bash

# Only emit guidance in Elm projects
if [ ! -f elm.json ]; then
    exit 0
fi

# Check if elmq is available
if ! command -v elmq &>/dev/null; then
    echo "WARNING: This is an Elm project but 'elmq' is not on PATH."
    echo "Install elmq for structured Elm file operations: https://github.com/caseyWebb/elmq"
    exit 0
fi

cat << 'EOF'
This is an Elm project with elmq MCP tools available. Prefer these over built-in tools for .elm files:

- **elm_summary** instead of Read — returns file structure (module line, imports, declarations with types and line numbers) in ~10% of the tokens. Use this first to understand a file before reading specific parts.
- **elm_get** instead of Read — extracts a single declaration's full source by name. Far fewer tokens than reading the entire file when you only need one function or type.
- **elm_edit** instead of Write/Edit — performs atomic, correct modifications. Handles imports, declarations, exposing lists, project-wide renames, module moves, declaration moves between modules, and variant propagation. One tool call replaces multi-step Read+Edit+verify cycles.
- **elm_refs** instead of Grep — finds all references to a module or declaration across the project, resolving qualified, aliased, and explicitly exposed names through import context. More accurate than text search.

Use Read/Write/Edit only when you need raw file content that isn't a declaration, or when creating a brand-new .elm file from scratch.
EOF
