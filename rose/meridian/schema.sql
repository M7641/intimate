-- schema.sql — the DESIRED STATE of the database (the single source of truth).
--
-- You edit THIS file to describe the schema you want. You do not write ALTER
-- statements by hand: `atlas migrate diff` compares this desired state against
-- the database (via a throwaway "dev" database) and generates the SQL needed to
-- close the gap. This is the "declarative", Terraform-style half of Atlas.
--
-- Dialect: SQLite (see atlas.hcl). Keep this portable-ish — the same shapes map
-- onto Postgres/MySQL with only type-name tweaks.

-- Authors of the works catalogued by the atlas.
CREATE TABLE authors (
    id         INTEGER   PRIMARY KEY,
    name       TEXT      NOT NULL,
    email      TEXT      NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

-- An author may only be registered once.
CREATE UNIQUE INDEX authors_email_idx ON authors (email);

-- Works catalogued, each attributed to exactly one author.
CREATE TABLE works (
    id         INTEGER   PRIMARY KEY,
    title      TEXT      NOT NULL,
    author_id  INTEGER   NOT NULL REFERENCES authors (id),
    year       INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

-- Backs the foreign key and speeds up "all works by author X".
CREATE INDEX works_author_id_idx ON works (author_id);
