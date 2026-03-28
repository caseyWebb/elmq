#!/usr/bin/env bash
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null)

if [[ "$FILE_PATH" == *.elm ]]; then
    echo '{"decision":"block","reason":"Do NOT use Edit on .elm files. Use elm_edit instead — it handles declarations, imports, exposing lists, renames, moves, and variant propagation atomically. Load it first with ToolSearch(\"select:elm_summary,elm_get,elm_edit,elm_refs\") if you have not already."}'
else
    echo '{"decision":"approve"}'
fi
