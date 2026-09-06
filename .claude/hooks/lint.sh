#!/usr/bin/env bash
# PostToolUse hook: format/lint the file Claude just wrote or edited.
# Best-effort and non-blocking. Skips any language whose tool is not installed.
set -u

f="$(jq -r '.tool_input.file_path // .tool_response.filePath // empty')"
[ -z "$f" ] && exit 0
[ -f "$f" ] || exit 0

have() { command -v "$1" >/dev/null 2>&1; }

# Silence formatter output. This hook is invisible on success by design.
exec >/dev/null 2>&1

case "$f" in
  *.rs)
    have rustfmt && rustfmt --edition 2021 "$f"
    ;;
  *.py)
    if have ruff; then
      ruff format "$f"; ruff check --fix "$f"
    elif have uvx; then
      uvx ruff format "$f"; uvx ruff check --fix "$f"
    fi
    ;;
  *.sql)
    # --config is required: sqruff does not auto-discover .config/sqruff.ini.
    have sqruff && sqruff fix --config "$CLAUDE_PROJECT_DIR/.config/sqruff.ini" "$f"
    ;;
  *.toml)
    have taplo && taplo fmt "$f"
    ;;
  *.md|*.json|*.jsonc|*.yaml|*.yml|*.css|*.html|*.ts|*.tsx|*.js|*.jsx)
    if have prettier; then
      prettier --write "$f"
    elif have npx; then
      npx --no-install prettier --write "$f"
    fi
    ;;
esac

exit 0
