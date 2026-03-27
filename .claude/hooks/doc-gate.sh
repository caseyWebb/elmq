#!/bin/bash
# doc-gate.sh — Stop hook that reminds Claude to review docs for updates.
# Reads `update-when` front-matter from .md files and presents a table.
# Blocks once per stop cycle (skips when stop_hook_active=true).

source "$(dirname "$0")/lib.sh"

parse_input

[ "$STOP_ACTIVE" = "true" ] && exit 0

cd "$CWD" || exit 0

has_code_changes || exit 0

# Scan project docs for update-when front-matter.
# Only checks root-level and docs/ — add new doc directories here if needed.
# update-when values must be single-line, unquoted YAML.
TABLE=""
for file in ./*.md docs/*.md; do
  [ -f "$file" ] || continue
  WHEN=$(awk '/^---$/{n++; next} n==1 && /^update-when:/{sub(/^update-when:[[:space:]]*/, ""); print; exit}' "$file" 2>/dev/null)
  if [ -n "$WHEN" ]; then
    TABLE="${TABLE}  ${file} — ${WHEN}
"
  fi
done

[ -z "$TABLE" ] && exit 0

REASON="Review these docs for any that need updating based on your work:

${TABLE}"

block_with_reason "$REASON"
