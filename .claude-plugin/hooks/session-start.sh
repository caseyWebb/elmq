#!/bin/bash
set -uo pipefail

INPUT=$(cat)

# Parse CWD from hook input JSON
if command -v jq &>/dev/null; then
  CWD=$(echo "$INPUT" | jq -r '.cwd')
else
  CWD=$(echo "$INPUT" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
fi

# elmq must be on PATH
command -v elmq &>/dev/null || exit 0

# Walk up from CWD looking for elm.json
dir="$CWD"
while [ "$dir" != "/" ]; do
  [ -f "$dir/elm.json" ] && exec elmq guide
  dir=$(dirname "$dir")
done

# If not found walking up, check nested directories (monorepo support)
find "$CWD" -maxdepth 3 -name elm.json -print -quit 2>/dev/null | grep -q . && exec elmq guide
