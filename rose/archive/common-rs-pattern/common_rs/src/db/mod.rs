//! Postgres / Redshift access layer for backend Rust crates.
//!
//! Wraps a `PgPool` plus the active `EnvManager`-derived schema, and exposes a
//! light query layer that takes psycopg-style named parameters in the SQL text:
//!
//! - `%(name)s` for a single scalar value (re-uses the same `$N` for repeats)
//! - `%(name)L` for an inline list, expanded into `$N, $N+1, …` at translation
//!   time (Redshift-compatible — no array binding required)
//!
//! The literal `{schema}` in the SQL is substituted with the schema chosen by
//! `EnvManager` (see `crate::env`). The schema comes from a closed enum, so it
//! is statically guaranteed to be a safe identifier — no runtime validation.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use regex::{Captures, Regex};
use serde_json::Value as JsonValue;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::{FromRow, Postgres, query, query_as};
use thiserror::Error;

pub mod redshift;

use crate::env::EnvManager;
use redshift::RedshiftConfig;

/// Heterogeneous bind value — covers every type the python `params` dict accepts,
/// plus inline-list variants for `%(name)L` expansion.
#[derive(Debug, Clone)]
pub enum BindValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    DateTime(DateTime<Utc>),
    Date(NaiveDate),
    Json(JsonValue),
    TextList(Vec<String>),
    IntList(Vec<i64>),
    DateList(Vec<NaiveDate>),
}

impl From<bool> for BindValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<i32> for BindValue {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}
impl From<i64> for BindValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<f64> for BindValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}
impl From<String> for BindValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}
impl From<&str> for BindValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}
impl From<DateTime<Utc>> for BindValue {
    fn from(v: DateTime<Utc>) -> Self {
        Self::DateTime(v)
    }
}
impl From<NaiveDate> for BindValue {
    fn from(v: NaiveDate) -> Self {
        Self::Date(v)
    }
}
impl From<Vec<String>> for BindValue {
    fn from(v: Vec<String>) -> Self {
        Self::TextList(v)
    }
}
impl From<Vec<i64>> for BindValue {
    fn from(v: Vec<i64>) -> Self {
        Self::IntList(v)
    }
}
impl From<Vec<NaiveDate>> for BindValue {
    fn from(v: Vec<NaiveDate>) -> Self {
        Self::DateList(v)
    }
}

/// Runtime parameters — bound server-side via sqlx, equivalent to python `params`.
pub type Params = HashMap<String, BindValue>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error("named-param `{0}` referenced in SQL but missing from `params`")]
    MissingParam(String),

    #[error("named-param `{name}` was used as `%({name})L` but is bound to a non-list value")]
    NotAList { name: String },
}

/// `%(name)s` — single scalar, deduped across repeated names.
/// `%(name)L` — list, expanded into `$N, $N+1, …` at translation time.
static PARAM_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%\(([a-zA-Z_][a-zA-Z0-9_]*)\)([sL])").expect("param-marker regex")
});

/// Translate `%(name)s` / `%(name)L` markers into positional `$N` placeholders,
/// returning the rewritten SQL plus the ordered values to bind.
///
/// Repeated `%(name)s` references reuse the same `$N`. `%(name)L` always
/// allocates fresh placeholders per element. An empty list expands to `NULL`,
/// which yields a always-false predicate inside `IN (...)` — the conventional
/// "list contains nothing" semantics.
pub fn translate(sql: &str, params: &Params) -> Result<(String, Vec<BindValue>), DbError> {
    let mut binds: Vec<BindValue> = Vec::new();
    let mut name_to_pos: HashMap<String, usize> = HashMap::new();
    let mut next_pos: usize = 1;
    let mut deferred_error: Option<DbError> = None;

    let rewritten = PARAM_MARKER.replace_all(sql, |caps: &Captures| -> String {
        if deferred_error.is_some() {
            return String::new();
        }
        let name = &caps[1];
        let kind = &caps[2];
        match kind {
            "s" => {
                if let Some(&pos) = name_to_pos.get(name) {
                    return format!("${pos}");
                }
                match params.get(name) {
                    Some(v) => {
                        binds.push(v.clone());
                        let p = next_pos;
                        next_pos += 1;
                        name_to_pos.insert(name.to_string(), p);
                        format!("${p}")
                    }
                    None => {
                        deferred_error = Some(DbError::MissingParam(name.to_string()));
                        String::new()
                    }
                }
            }
            "L" => {
                let value = match params.get(name) {
                    Some(v) => v,
                    None => {
                        deferred_error = Some(DbError::MissingParam(name.to_string()));
                        return String::new();
                    }
                };
                let placeholders: Vec<String> = match value {
                    BindValue::TextList(xs) => {
                        if xs.is_empty() {
                            return "NULL".to_string();
                        }
                        xs.iter()
                            .map(|x| {
                                binds.push(BindValue::Text(x.clone()));
                                let p = next_pos;
                                next_pos += 1;
                                format!("${p}")
                            })
                            .collect()
                    }
                    BindValue::IntList(xs) => {
                        if xs.is_empty() {
                            return "NULL".to_string();
                        }
                        xs.iter()
                            .map(|x| {
                                binds.push(BindValue::Int(*x));
                                let p = next_pos;
                                next_pos += 1;
                                format!("${p}")
                            })
                            .collect()
                    }
                    BindValue::DateList(xs) => {
                        if xs.is_empty() {
                            return "NULL".to_string();
                        }
                        xs.iter()
                            .map(|x| {
                                binds.push(BindValue::Date(*x));
                                let p = next_pos;
                                next_pos += 1;
                                format!("${p}")
                            })
                            .collect()
                    }
                    _ => {
                        deferred_error = Some(DbError::NotAList {
                            name: name.to_string(),
                        });
                        return String::new();
                    }
                };
                placeholders.join(", ")
            }
            _ => unreachable!("regex only matches 's' or 'L'"),
        }
    });

    if let Some(e) = deferred_error {
        return Err(e);
    }
    Ok((rewritten.into_owned(), binds))
}

fn bind_value<'q>(
    q: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    v: BindValue,
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    match v {
        BindValue::Null => q.bind(None::<String>),
        BindValue::Bool(b) => q.bind(b),
        BindValue::Int(i) => q.bind(i),
        BindValue::Float(f) => q.bind(f),
        BindValue::Text(t) => q.bind(t),
        BindValue::DateTime(d) => q.bind(d),
        BindValue::Date(d) => q.bind(d),
        BindValue::Json(j) => q.bind(j),
        // Lists are flattened by `translate` before reaching here.
        BindValue::TextList(_) | BindValue::IntList(_) | BindValue::DateList(_) => {
            unreachable!("list BindValues must be expanded by translate before binding")
        }
    }
}

fn bind_value_as<'q, T>(
    q: sqlx::query::QueryAs<'q, Postgres, T, sqlx::postgres::PgArguments>,
    v: BindValue,
) -> sqlx::query::QueryAs<'q, Postgres, T, sqlx::postgres::PgArguments>
where
    T: Send + Unpin,
{
    match v {
        BindValue::Null => q.bind(None::<String>),
        BindValue::Bool(b) => q.bind(b),
        BindValue::Int(i) => q.bind(i),
        BindValue::Float(f) => q.bind(f),
        BindValue::Text(t) => q.bind(t),
        BindValue::DateTime(d) => q.bind(d),
        BindValue::Date(d) => q.bind(d),
        BindValue::Json(j) => q.bind(j),
        BindValue::TextList(_) | BindValue::IntList(_) | BindValue::DateList(_) => {
            unreachable!("list BindValues must be expanded by translate before binding")
        }
    }
}

/// Database handle: pool plus the active environment-derived schema.
///
/// SQL passed through `load_data` / `execute_query` may use `{schema}` as a
/// literal placeholder for the schema name; it is substituted before parameter
/// binding. The schema comes from `EnvManager::schema()`, which is a closed
/// `&'static str` set, so the substitution is safe by construction.
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
    schema: &'static str,
}

impl Db {
    /// Build a pool against the given Redshift/Postgres config and bind it to
    /// the schema implied by `env`.
    pub async fn connect(env: &EnvManager, cfg: &RedshiftConfig) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .idle_timeout(Duration::from_secs(60))
            .max_lifetime(Duration::from_secs(600))
            .acquire_timeout(Duration::from_secs(2))
            .connect(&cfg.to_url())
            .await?;
        Ok(Self {
            pool,
            schema: env.schema(),
        })
    }

    /// Construct from an already-built pool. Useful in tests.
    pub fn from_parts(pool: PgPool, schema: &'static str) -> Self {
        Self { pool, schema }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn schema(&self) -> &'static str {
        self.schema
    }

    /// Run a SELECT, deserialising each row into `T` via sqlx::FromRow.
    pub async fn load_data<T>(&self, sql: &str, params: &Params) -> Result<Vec<T>, DbError>
    where
        T: for<'r> FromRow<'r, PgRow> + Send + Unpin,
    {
        let sql = sql.replace("{schema}", self.schema);
        let (rewritten, binds) = translate(&sql, params)?;
        let mut q = query_as::<Postgres, T>(&rewritten);
        for v in binds {
            q = bind_value_as(q, v);
        }
        Ok(q.fetch_all(&self.pool).await?)
    }

    /// Execute a statement (INSERT/UPDATE/DELETE/DDL); returns rows-affected count.
    pub async fn execute_query(&self, sql: &str, params: &Params) -> Result<u64, DbError> {
        let sql = sql.replace("{schema}", self.schema);
        let (rewritten, binds) = translate(&sql, params)?;
        let mut q = query::<Postgres>(&rewritten);
        for v in binds {
            q = bind_value(q, v);
        }
        Ok(q.execute(&self.pool).await?.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pairs: &[(&str, BindValue)]) -> Params {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn translates_single_scalar() {
        let (sql, binds) = translate(
            "SELECT * FROM t WHERE id = %(id)s",
            &p(&[("id", BindValue::Text("abc".into()))]),
        )
        .unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE id = $1");
        assert_eq!(binds.len(), 1);
        assert!(matches!(&binds[0], BindValue::Text(s) if s == "abc"));
    }

    #[test]
    fn dedups_repeated_scalar_names() {
        let (sql, binds) = translate(
            "WHERE a = %(x)s OR b = %(x)s OR c = %(y)s",
            &p(&[("x", BindValue::Int(1)), ("y", BindValue::Int(2))]),
        )
        .unwrap();
        assert_eq!(sql, "WHERE a = $1 OR b = $1 OR c = $2");
        assert_eq!(binds.len(), 2);
    }

    #[test]
    fn expands_text_list_into_placeholders() {
        let (sql, binds) = translate(
            "WHERE id IN (%(ids)L)",
            &p(&[(
                "ids",
                BindValue::TextList(vec!["a".into(), "b".into(), "c".into()]),
            )]),
        )
        .unwrap();
        assert_eq!(sql, "WHERE id IN ($1, $2, $3)");
        assert_eq!(binds.len(), 3);
        assert!(matches!(&binds[0], BindValue::Text(s) if s == "a"));
    }

    #[test]
    fn expands_int_list() {
        let (sql, binds) = translate(
            "WHERE n IN (%(ns)L)",
            &p(&[("ns", BindValue::IntList(vec![10, 20]))]),
        )
        .unwrap();
        assert_eq!(sql, "WHERE n IN ($1, $2)");
        assert_eq!(binds.len(), 2);
    }

    #[test]
    fn empty_list_expands_to_null() {
        let (sql, binds) = translate(
            "WHERE id IN (%(ids)L)",
            &p(&[("ids", BindValue::TextList(vec![]))]),
        )
        .unwrap();
        assert_eq!(sql, "WHERE id IN (NULL)");
        assert!(binds.is_empty());
    }

    #[test]
    fn list_after_scalar_continues_positions() {
        let (sql, binds) = translate(
            "WHERE created_by = %(actor)s AND id IN (%(ids)L)",
            &p(&[
                ("actor", BindValue::Text("svc".into())),
                ("ids", BindValue::TextList(vec!["a".into(), "b".into()])),
            ]),
        )
        .unwrap();
        assert_eq!(sql, "WHERE created_by = $1 AND id IN ($2, $3)");
        assert_eq!(binds.len(), 3);
    }

    #[test]
    fn missing_scalar_param_errors() {
        let err = translate("WHERE id = %(missing)s", &p(&[])).unwrap_err();
        assert!(matches!(err, DbError::MissingParam(n) if n == "missing"));
    }

    #[test]
    fn list_marker_against_scalar_value_errors() {
        let err = translate(
            "WHERE id IN (%(x)L)",
            &p(&[("x", BindValue::Text("oops".into()))]),
        )
        .unwrap_err();
        assert!(matches!(err, DbError::NotAList { name } if name == "x"));
    }

    #[test]
    fn ignores_bare_percent_signs() {
        let (sql, binds) = translate("SELECT * FROM t WHERE x LIKE '%abc%'", &p(&[])).unwrap();
        assert_eq!(sql, "SELECT * FROM t WHERE x LIKE '%abc%'");
        assert!(binds.is_empty());
    }
}
