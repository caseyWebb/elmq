#!/bin/bash
# lib.sh — Shared utilities for Claude Code hooks.
# Source this from hook scripts: source "$(dirname "$0")/lib.sh"

# -e deliberately omitted: grep -v in has_code_changes exits 1 when all files
# are .md, which is expected (not an error). Adding -e would break that path.
set -uo pipefail

# Parse hook input JSON from stdin.
# Sets: CWD, STOP_ACTIVE, PROMPT
parse_input() {
  INPUT=$(cat)
  if command -v jq &>/dev/null; then
    CWD=$(echo "$INPUT" | jq -r '.cwd')
    STOP_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
    PROMPT=$(echo "$INPUT" | jq -r '.prompt // ""')
  else
    CWD=$(echo "$INPUT" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
    STOP_ACTIVE=$(echo "$INPUT" | sed -n 's/.*"stop_hook_active"[[:space:]]*:[[:space:]]*\(true\|false\).*/\1/p')
    PROMPT=$(echo "$INPUT" | sed -n 's/.*"prompt"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
  fi
  STOP_ACTIVE="${STOP_ACTIVE:-false}"
  PROMPT="${PROMPT:-}"
}

# Returns 0 if non-.md files have been modified, staged, or are untracked.
has_code_changes() {
  local changes
  changes=$(
    { git diff --name-only 2>/dev/null; git diff --cached --name-only 2>/dev/null; git ls-files --others --exclude-standard 2>/dev/null; } \
    | grep -v '\.md$' | grep -v '^openspec/' | head -1
  )
  [ -n "$changes" ]
}

# Returns 0 if .md files have been modified, staged, or are untracked.
has_md_changes() {
  local changes
  changes=$(
    { git diff --name-only -- '*.md' 2>/dev/null; git diff --cached --name-only -- '*.md' 2>/dev/null; git ls-files --others --exclude-standard -- '*.md' 2>/dev/null; } \
    | grep -v '^openspec/'
  )
  [ -n "$changes" ]
}

# Output a JSON block decision. Uses jq for safe escaping with printf fallback.
# Usage: block_with_reason "Your message here"
block_with_reason() {
  local reason="$1"
  if command -v jq &>/dev/null; then
    jq -n --arg reason "$reason" '{ decision: "block", reason: $reason }'
  else
    # Fallback: escape backslashes, quotes, and convert newlines to \n
    local escaped
    escaped=$(printf '%s' "$reason" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '{printf "%s\\n", $0}' | sed 's/\\n$//')
    printf '{"decision":"block","reason":"%s"}' "$escaped"
  fi
}
