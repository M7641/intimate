//! galicia-rs — PoC : **DuckDB + Apache Iceberg sur "S3" local**.
//!
//! Interprétation Rust du PoC Python `galicia`. Trois couches, volontairement
//! découplées (c'est tout l'intérêt pédagogique) :
//!
//! 1. [`objstore`] — le « bucket S3 », ici un simple dossier local. C'est le
//!    *seam* : passer en vrai S3 = changer une URI (`file://` → `s3://`).
//! 2. [`catalog`] — le table-format **Apache Iceberg** (crate `iceberg`) + un
//!    catalogue **SQLite** (crate `iceberg-catalog-sql`), qui joue le rôle de
//!    Glue/Nessie. C'est lui qui donne snapshots + time-travel.
//! 3. [`warehouse`] — le moteur de requête **DuckDB**, qui ne connaît que des
//!    fichiers Parquet ; le catalogue lui résout « nom logique → fichiers ».
//!
//! Le writer Iceberg ([`writer`]) écrit de vrais Parquet + metadata ; DuckDB
//! les relit. On ne fait JAMAIS transiter d'Arrow entre les deux mondes —
//! `iceberg` et le crate `duckdb` embarquent des versions d'`arrow` distinctes,
//! donc Iceberg **écrit des fichiers**, DuckDB **lit des fichiers**.

pub mod catalog;
pub mod demo;
pub mod objstore;
pub mod pipeline;
pub mod seed;
pub mod warehouse;
pub mod writer;
