# Makefile — thin wrappers around the Atlas CLI for the meridian pilot.
#
# Every target runs against the "local" env defined in atlas.hcl (SQLite, no
# Docker). The `atlas` binary is pinned by proto (.prototools at the repo root,
# plugin under proto-plugins/atlas.toml); `proto use` installs it and proto's
# shims put it on PATH. Override with `make ATLAS=/path/to/atlas <target>`.

ATLAS ?= atlas
ENV   := local

.PHONY: help setup migrate-diff migrate-apply migrate-status migrate-lint \
        schema-apply schema-inspect demo clean

help:
	@echo "meridian — Atlas pilot (SQLite, no Docker)"
	@echo
	@echo "  make setup                   install the pinned Atlas CLI via proto"
	@echo "  make migrate-diff name=NAME  generate a migration from schema.sql"
	@echo "  make migrate-apply           apply pending migrations to meridian.db"
	@echo "  make migrate-status          show which migrations have run"
	@echo "  make migrate-lint            safety-lint the latest migration"
	@echo "  make schema-apply            declaratively sync the DB to schema.sql"
	@echo "  make schema-inspect          print the live DB schema as SQL"
	@echo "  make demo                    end-to-end versioned-workflow walkthrough"
	@echo "  make clean                   remove the SQLite db"
	@echo
	@echo "Using atlas: $(ATLAS)"

# Installs every tool pinned in .prototools (Atlas included) into proto.
setup:
	proto use

# Generate a new migration that moves the DB to match schema.sql.
#   make migrate-diff name=init
migrate-diff:
	@test -n "$(name)" || { echo "usage: make migrate-diff name=<migration_name>"; exit 1; }
	$(ATLAS) migrate diff $(name) --env $(ENV)

migrate-apply:
	$(ATLAS) migrate apply --env $(ENV)

migrate-status:
	$(ATLAS) migrate status --env $(ENV)

# Static analysis of the latest migration: flags destructive changes, NOT NULL
# columns added without a default, table locks, etc. The CI safety net.
migrate-lint:
	$(ATLAS) migrate lint --env $(ENV) --latest 1

# Declarative path: skip migration files entirely and sync the DB straight to
# schema.sql (Atlas plans the diff and applies it after a confirmation prompt).
schema-apply:
	$(ATLAS) schema apply --env $(ENV)

schema-inspect:
	$(ATLAS) schema inspect --env $(ENV) --format '{{ sql . }}'

demo:
	@./scripts/demo.sh

clean:
	rm -f meridian.db meridian.db-shm meridian.db-wal
