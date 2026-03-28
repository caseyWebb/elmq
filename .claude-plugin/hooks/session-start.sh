#!/usr/bin/env bash

# Only emit guidance in Elm projects — check ancestors and descendants
find_elm_json() {
    # Check cwd and ancestors
    local dir="$PWD"
    while [ "$dir" != "/" ]; do
        if [ -f "$dir/elm.json" ]; then
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    # Check descendants (monorepo with nested Elm projects)
    find "$PWD" -name elm.json -not -path "*/node_modules/*" -not -path "*/elm-stuff/*" -print -quit 2>/dev/null | grep -q .
}

if ! find_elm_json; then
    exit 0
fi

# Check if elmq is available
if ! command -v elmq &>/dev/null; then
    echo "WARNING: This is an Elm project but 'elmq' is not on PATH."
    echo "Install elmq for structured Elm file operations: https://github.com/caseyWebb/elmq"
    exit 0
fi

cat << 'EOF'
IMPORTANT: This is an Elm project with elmq MCP tools available. Before doing ANY work on .elm files, you MUST first load the elmq tools by calling: ToolSearch("select:elm_summary,elm_get,elm_edit,elm_refs"). Do this at the very start — the tools are deferred and won't appear in your tool list until you search for them.

Once loaded, you MUST use the elmq tools instead of built-in tools for .elm files. Do NOT use Read, Write, Edit, or Grep on .elm files — use the elmq equivalents below.

RULES:
1. To understand a file's structure → use elm_summary (NOT Read). Returns module line, imports, all declarations with types and line numbers in ~10% of the tokens.
2. To read a specific function, type, or port → use elm_get with the declaration name (NOT Read). Returns just that declaration's source.
3. To modify any .elm file → use elm_edit (NOT Write or Edit). Supports:
   - action: "set" — upsert a declaration (provide full source)
   - action: "patch" — find-replace within a declaration (provide name, old, new)
   - action: "rm" — remove a declaration
   - action: "add_import" / "remove_import" — manage imports
   - action: "expose" / "unexpose" — manage exposing list
   - action: "mv" — rename/move a module and update all references project-wide
   - action: "rename" — rename a declaration and update all references project-wide
   - action: "move_decl" — move declarations between modules with automatic import rewriting
   - action: "add_variant" / "rm_variant" — add/remove type constructors and propagate through all case expressions
4. To find references → use elm_refs (NOT Grep). Resolves qualified, aliased, and exposed names through import context.

The ONLY acceptable uses of built-in tools on .elm files:
- Write: creating a brand-new .elm file that doesn't exist yet
- Bash: running elm make, elm-format, elm-test, elm-review, or other CLI tools
EOF
