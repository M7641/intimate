# atlas.hcl — Atlas project configuration for the meridian pilot.
#
# A single environment, "local", wired entirely to SQLite so the whole workflow
# runs with NO Docker and NO external services:
#
#   url            the database Atlas manages — a plain file, meridian.db.
#   dev            a THROWAWAY database Atlas needs to parse, normalise and diff
#                  schemas. An in-memory SQLite means nothing is installed or
#                  left behind. (The Postgres/MySQL equivalents are docker://…
#                  URLs that require a running Docker daemon — see README.)
#   src            the declarative desired state, in plain SQL (schema.sql).
#                  Used by `atlas schema apply` and as the target of
#                  `atlas migrate diff`.
#   migration.dir  where versioned migration files are written and read from.
#
# Run any command against this env with `--env local`, e.g.
#   atlas migrate diff init --env local

env "local" {
  url = "sqlite://meridian.db"
  dev = "sqlite://dev?mode=memory"
  src = "file://schema.sql"

  migration {
    dir = "file://migrations"
  }
}
