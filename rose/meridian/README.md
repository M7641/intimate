# meridian

A pilot for **[Atlas](https://atlasgo.io)** — *"Terraform, but for databases."*
You declare the schema you **want** in one file; Atlas plans, lints and applies
the SQL needed to get any database there.

> **Tool:** Atlas Community Edition — <https://atlasgo.io> (Apache-2.0)
> Pinned by proto (`atlas = "latest"` in [`/.prototools`](../../.prototools),
> vendored plugin [`/proto-plugins/atlas.toml`](../../proto-plugins/atlas.toml)).

This pilot runs against **SQLite with an in-memory dev database**, so the entire
workflow works with **no Docker and no cloud login**. See
[Switching to Postgres](#switching-to-postgres) for the one-line change.

---

## The idea in one picture

```
                 schema.sql                         (you edit this)
            "the state I want"
                    │
                    │   atlas migrate diff   ← compares want vs. have using a
                    ▼                          throwaway in-memory "dev" DB
            migrations/*.sql   +  atlas.sum    (committed, reviewed in PRs)
                    │
                    │   atlas migrate apply
                    ▼
              meridian.db                          (the real database)
```

Atlas offers **two workflows over the same `schema.sql`**:

| | Versioned (this pilot's default) | Declarative |
|---|---|---|
| What you commit | reviewable `migrations/*.sql` files | just `schema.sql` |
| Command | `atlas migrate diff` → `apply` | `atlas schema apply` |
| Analogy | Rails/Flyway migrations, but **generated** | `terraform apply` |
| Best for | production schemas, audited change history | quick prototyping, dev DBs |

The killer feature is **`atlas migrate lint`**: static analysis that catches
destructive or locking changes *before* they reach a database.

---

## Why this matters here

The monorepo already runs real databases — `cocoon`'s `tako-database` /
`warehouse`, `blank/io/database`, the Postgres-backed integration tests. Today
their schema lives in hand-written SQL/DDL. Atlas would let those projects keep
a single declarative `schema.sql`, **generate** migrations from it, and gate
every change in CI with `atlas migrate lint` — the same way `cargo-deny` and
`gitleaks` already gate dependencies and secrets via lefthook.

---

## Layout

```
meridian/
  schema.sql            # the desired state — the single source of truth
  atlas.hcl             # one "local" env: url + in-memory dev + migration dir
  migrations/           # generated, committed, reviewed
    20260607090740_init.sql
    atlas.sum           # checksum file — tamper-evident migration integrity
  Makefile              # thin wrappers: make migrate-diff / apply / lint / demo
  scripts/demo.sh       # end-to-end transcript you can run
```

## Prerequisites

- **proto** (already the repo's toolchain manager). Atlas is pinned in
  `.prototools`; nothing else to install.
- No Docker, no database server, no `atlas login`.

## Quick start

```bash
cd rose/meridian
make setup          # = proto use → installs the pinned Atlas CLI
make demo           # runs the whole versioned workflow, printing each command
```

`make demo` starts from a clean DB and walks through diff → lint → apply →
status → inspect. Re-runnable any time.

## The versioned workflow by hand

```bash
# 1. Generate a migration from schema.sql (already committed as the "init" file)
make migrate-diff name=init

# 2. Eyeball it, then lint for unsafe changes
make migrate-lint

# 3. Apply to meridian.db
make migrate-apply

# 4. Confirm state
make migrate-status
```

The committed [`migrations/20260607090740_init.sql`](migrations/20260607090740_init.sql)
was produced by step 1 and looks like:

```sql
-- Create "authors" table
CREATE TABLE `authors` (`id` integer NULL, `name` text NOT NULL, ...);
-- Create index "authors_email_idx" to table: "authors"
CREATE UNIQUE INDEX `authors_email_idx` ON `authors` (`email`);
-- Create "works" table
CREATE TABLE `works` (..., CONSTRAINT `0` FOREIGN KEY (`author_id`) REFERENCES `authors` (`id`) ...);
-- Create index "works_author_id_idx" to table: "works"
CREATE INDEX `works_author_id_idx` ON `works` (`author_id`);
```

### Making a change

Edit `schema.sql` — say, add a column to `works`:

```sql
    year       INTEGER,
    isbn       TEXT,        -- NEW
```

Then:

```bash
make migrate-diff name=add_isbn
```

Atlas diffs the new desired state against the migrations already applied and
writes **only the delta**:

```sql
-- Add column "isbn" to table: "works"
ALTER TABLE `works` ADD COLUMN `isbn` text NULL;
```

You never wrote the `ALTER` — Atlas did. Lint it, apply it, commit it.

## The declarative alternative

Skip migration files entirely and sync the database straight to `schema.sql`:

```bash
make schema-apply       # plans the diff, asks to confirm, applies
make schema-inspect     # dump the live schema as SQL
```

Same source of truth, no migration history — handy for throwaway dev databases.

## Switching to Postgres

`atlas.hcl` is wired to SQLite purely to stay Docker-free. To target Postgres,
change two URLs (the `dev` URL spins up a *throwaway* container Atlas manages
for you):

```hcl
env "local" {
  url = "postgres://user:pass@localhost:5432/meridian?sslmode=disable"
  dev = "docker://postgres/17/dev?search_path=public"   # needs Docker running
  src = "file://schema.sql"
  migration { dir = "file://migrations" }
}
```

The migration files regenerate with Postgres-native types — everything else
(commands, Makefile, CI) is identical.

---

## Evaluation notes

**What worked well**

- **Zero-friction local loop.** SQLite + in-memory dev DB means the full
  diff/lint/apply cycle runs offline in milliseconds — ideal for a pilot.
- **Generated migrations.** You maintain *one* declarative file; the `ALTER`
  statements are derived, not hand-written, eliminating a whole class of
  drift-vs-DDL mistakes.
- **`atlas.sum` integrity.** A checksum file makes migration tampering or
  out-of-order edits a CI failure, not a silent prod surprise.
- **Clean proto integration.** Pinning Atlas was a vendored TOML plugin plus two
  lines in `.prototools`, matching how `sqruff`/`cargo-deny` are managed.

**Limitations found (Community Edition)**

- On `schema inspect` / `migrate` Atlas prints a notice that the **community
  build lacks**: checkpoints, **down (rollback) migrations**, migration
  *testing*, and advanced objects (**views, triggers, stored procedures**).
  Schema-heavy services that rely on those would need the paid edition.
- Inspect also surfaces Atlas's own bookkeeping table, `atlas_schema_revisions`
  (its applied-migration journal) — expected, but worth knowing it lives in your
  target database.
- SQLite has no native `ALTER COLUMN`; destructive changes there are emulated.
  Lint's most interesting warnings (locks, backfills) are Postgres/MySQL-centric.

**Suggested next steps if we adopt this**

1. Re-run this pilot against a real Postgres (cocoon's `warehouse`) to see the
   Postgres diff/lint output and confirm `docker://` dev URLs in our setup.
2. Wire `atlas migrate lint` into `lefthook.yml` (pre-push) and/or a moon
   `:lint` task, so schema changes are gated like everything else.
3. Decide versioned vs declarative per service, and whether the missing
   down-migrations / testing features justify the paid edition anywhere.

## Reference

- Atlas docs: <https://atlasgo.io/getting-started>
- Community Edition limits: <https://atlasgo.io/community-edition>
- Vendored proto plugin: [`/proto-plugins/atlas.toml`](../../proto-plugins/atlas.toml)
