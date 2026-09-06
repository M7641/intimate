#!/usr/bin/env bash
# SessionStart hook: inject the repo's AGENTS.md into context so agents follow it
# alongside CLAUDE.md.
set -u

root="${CLAUDE_PROJECT_DIR:-$PWD}"
f="$root/AGENTS.md"
[ -f "$f" ] || exit 0

jq -n --rawfile c "$f" \
  '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$c}}'
