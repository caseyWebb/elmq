#!/usr/bin/env bash
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null)

if [[ "$FILE_PATH" == *.elm ]]; then
    echo '{"decision":"block","reason":"Do NOT use Read on .elm files. Use the elmq MCP tools instead: elm_summary for file structure, or elm_get for a specific declaration. Load them first with ToolSearch(\"select:elm_summary,elm_get,elm_edit,elm_refs\") if you have not already."}'
else
    echo '{"decision":"approve"}'
fi
