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
found=false
dir="$CWD"
while [ "$dir" != "/" ]; do
  if [ -f "$dir/elm.json" ]; then
    found=true
    break
  fi
  dir=$(dirname "$dir")
done

# If not found walking up, check nested directories (monorepo support)
if [ "$found" = false ]; then
  if find "$CWD" -maxdepth 3 -name elm.json -print -quit 2>/dev/null | grep -q .; then
    found=true
  fi
fi

[ "$found" = true ] || exit 0

# Resolve the guide path relative to this script
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GUIDE="$SCRIPT_DIR/../elmq-guide.md"

[ -f "$GUIDE" ] && cat "$GUIDE"
