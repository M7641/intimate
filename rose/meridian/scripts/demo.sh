#!/usr/bin/env sh
# demo.sh — end-to-end walkthrough of Atlas's VERSIONED workflow against SQLite.
#
# Each step prints the exact command before running it, so you can read the
# pilot as a transcript. Safe to re-run: it starts from a clean database.
set -eu

cd "$(dirname "$0")/.."

# Atlas is pinned by proto; its shim should be on PATH after `proto use`.
if command -v atlas >/dev/null 2>&1; then
  ATLAS=atlas
else
  echo "Atlas CLI not found. Run 'make setup' (or 'proto use') first." >&2
  exit 1
fi

step() { printf '\n\033[1;36m== %s\033[0m\n' "$1"; }
run()  { printf '\033[2m$ %s\033[0m\n' "$*"; "$@"; }

step "0. Start from a clean database"
run rm -f meridian.db meridian.db-shm meridian.db-wal

step "1. Generate the initial migration from schema.sql"
echo "   Atlas diffs schema.sql against an empty in-memory dev DB and writes the SQL."
if [ -z "$(ls migrations/*.sql 2>/dev/null || true)" ]; then
  run "$ATLAS" migrate diff init --env local
else
  echo "   (migrations already exist — skipping generation)"
fi

step "2. Show the generated migration directory"
run ls -1 migrations
echo
echo "   --- contents ---"
cat migrations/*.sql

step "3. Lint the latest migration for unsafe changes"
echo "   (clean here; lint shines once you start ALTERing existing tables)"
run "$ATLAS" migrate lint --env local --latest 1 || true

step "4. Apply pending migrations to meridian.db"
run "$ATLAS" migrate apply --env local

step "5. Show migration status"
run "$ATLAS" migrate status --env local

step "6. Inspect the live database as SQL"
run "$ATLAS" schema inspect --env local --format '{{ sql . }}'

step "Done"
echo "Next: edit schema.sql (e.g. add an 'isbn TEXT' column to works), then run"
echo "  make migrate-diff name=add_isbn   &&   make migrate-lint   &&   make migrate-apply"
