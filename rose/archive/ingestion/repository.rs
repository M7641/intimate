use chrono::{DateTime, Utc};
use database::{DBActions, Row};
use serde_json::Value;
use std::sync::Arc;

use super::models::{IngestionError, IngestionJob, JobStatus};

pub struct JobRepository {
    db: Arc<DBActions>,
}

impl JobRepository {
    pub fn new(db: Arc<DBActions>) -> Self {
        Self { db }
    }

    pub async fn initialize_schema(&self) -> Result<(), IngestionError> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let sql = r#"
                CREATE TABLE IF NOT EXISTS ingestion_jobs (
                    id VARCHAR(36) PRIMARY KEY,
                    tenant VARCHAR(255) NOT NULL,
                    file_name VARCHAR(512) NOT NULL,
                    s3_path VARCHAR(1024) NOT NULL,
                    status VARCHAR(50) NOT NULL DEFAULT 'pending',
                    error TEXT,
                    row_count BIGINT,
                    created_at TIMESTAMP NOT NULL,
                    updated_at TIMESTAMP NOT NULL
                )
            "#;

            db.execute(sql, &[])?;
            Ok(())
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn create_job(&self, job: &IngestionJob) -> Result<(), IngestionError> {
        let db = self.db.clone();
        let job = job.clone();

        tokio::task::spawn_blocking(move || {
            let sql = r#"
                INSERT INTO ingestion_jobs (
                    id, tenant, file_name, s3_path, status,
                    error, row_count, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#;

            let params: Vec<Value> = vec![
                Value::String(job.id),
                Value::String(job.tenant),
                Value::String(job.file_name),
                Value::String(job.s3_path),
                Value::String(job.status.as_str().to_string()),
                job.error.map(Value::String).unwrap_or(Value::Null),
                job.row_count
                    .map(|r| Value::Number(r.into()))
                    .unwrap_or(Value::Null),
                Value::String(job.created_at.to_rfc3339()),
                Value::String(job.updated_at.to_rfc3339()),
            ];

            db.execute(sql, &params)?;
            Ok(())
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn get_job(&self, job_id: &str) -> Result<IngestionJob, IngestionError> {
        let db = self.db.clone();
        let job_id = job_id.to_string();

        tokio::task::spawn_blocking(move || {
            let sql = "SELECT * FROM ingestion_jobs WHERE id = $1";
            let params = vec![Value::String(job_id.clone())];

            let rows = db.query(sql, &params)?;
            let row = rows
                .into_iter()
                .next()
                .ok_or(IngestionError::NotFound(job_id))?;

            Self::row_to_job(row)
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: i32,
    ) -> Result<Vec<IngestionJob>, IngestionError> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let (sql, params) = if let Some(s) = status {
                (
                    "SELECT * FROM ingestion_jobs WHERE status = $1 ORDER BY created_at DESC LIMIT $2",
                    vec![
                        Value::String(s.as_str().to_string()),
                        Value::Number(limit.into()),
                    ],
                )
            } else {
                (
                    "SELECT * FROM ingestion_jobs ORDER BY created_at DESC LIMIT $1",
                    vec![Value::Number(limit.into())],
                )
            };

            let rows = db.query(sql, &params)?;
            rows.into_iter().map(Self::row_to_job).collect()
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn get_pending_jobs(&self, limit: i32) -> Result<Vec<IngestionJob>, IngestionError> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let sql = r#"
                SELECT * FROM ingestion_jobs
                WHERE status = 'pending'
                ORDER BY created_at ASC
                LIMIT $1
            "#;

            let rows = db.query(sql, &[Value::Number(limit.into())])?;
            rows.into_iter().map(Self::row_to_job).collect()
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn set_in_progress(&self, job_id: &str) -> Result<(), IngestionError> {
        let db = self.db.clone();
        let job_id = job_id.to_string();

        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let sql = r#"
                UPDATE ingestion_jobs
                SET status = 'in_progress', updated_at = $1
                WHERE id = $2
            "#;

            db.execute(
                sql,
                &[Value::String(now.to_rfc3339()), Value::String(job_id)],
            )?;

            Ok(())
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn complete_job(
        &self,
        job_id: &str,
        row_count: Option<i64>,
    ) -> Result<(), IngestionError> {
        let db = self.db.clone();
        let job_id = job_id.to_string();

        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let sql = r#"
                UPDATE ingestion_jobs
                SET status = 'completed', row_count = $1, updated_at = $2
                WHERE id = $3
            "#;

            db.execute(
                sql,
                &[
                    row_count
                        .map(|r| Value::Number(r.into()))
                        .unwrap_or(Value::Null),
                    Value::String(now.to_rfc3339()),
                    Value::String(job_id),
                ],
            )?;

            Ok(())
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), IngestionError> {
        let db = self.db.clone();
        let job_id = job_id.to_string();
        let error = error.to_string();

        tokio::task::spawn_blocking(move || {
            let now = Utc::now();
            let sql = r#"
                UPDATE ingestion_jobs
                SET status = 'failed', error = $1, updated_at = $2
                WHERE id = $3
            "#;

            db.execute(
                sql,
                &[
                    Value::String(error),
                    Value::String(now.to_rfc3339()),
                    Value::String(job_id),
                ],
            )?;

            Ok(())
        })
        .await
        .map_err(|e| IngestionError::Processing(format!("Task join error: {}", e)))?
    }

    fn row_to_job(row: Row) -> Result<IngestionJob, IngestionError> {
        let get_string = |row: &Row, key: &str| -> Result<String, IngestionError> {
            match row.get(key) {
                Some(Value::String(s)) => Ok(s.clone()),
                Some(other_value) => Err(IngestionError::Processing(format!(
                    "Invalid field type for '{}': expected String, got {:?}",
                    key, other_value
                ))),
                None => Err(IngestionError::Processing(format!(
                    "Missing required field: '{}'",
                    key
                ))),
            }
        };

        let get_string_from_datetime = |row: &Row, key: &str| -> Result<String, IngestionError> {
            match row.get(key) {
                Some(Value::Number(n)) => {
                    if let Some(timestamp_micros) = n.as_i64() {
                        let seconds = timestamp_micros / 1_000_000;
                        let nanos = ((timestamp_micros % 1_000_000) * 1_000) as u32;
                        let dt = DateTime::from_timestamp(seconds, nanos).ok_or_else(|| {
                            IngestionError::Processing(format!(
                                "Invalid timestamp for '{}': {}",
                                key, timestamp_micros
                            ))
                        })?;
                        Ok(dt.to_rfc3339())
                    } else {
                        Err(IngestionError::Processing(format!(
                            "Invalid number type for '{}': expected i64 timestamp",
                            key
                        )))
                    }
                }
                Some(other_value) => Err(IngestionError::Processing(format!(
                    "Invalid field type for '{}': expected Number (timestamp), got {:?}",
                    key, other_value
                ))),
                None => Err(IngestionError::Processing(format!(
                    "Missing required field: '{}'",
                    key
                ))),
            }
        };

        let get_optional_string = |row: &Row, key: &str| -> Option<String> {
            match row.get(key) {
                Some(Value::String(s)) => Some(s.clone()),
                _ => None,
            }
        };

        let get_optional_i64 = |row: &Row, key: &str| -> Option<i64> {
            match row.get(key) {
                Some(Value::Number(n)) => n.as_i64(),
                _ => None,
            }
        };

        let parse_datetime = |s: &str| -> Result<DateTime<Utc>, IngestionError> {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                        .map(|ndt| ndt.and_utc())
                })
                .map_err(|e| IngestionError::Processing(format!("Invalid datetime: {}", e)))
        };

        let id = get_string(&row, "id")?;
        let tenant = get_string(&row, "tenant")?;
        let file_name = get_string(&row, "file_name")?;
        let s3_path = get_string(&row, "s3_path")?;
        let status_str = get_string(&row, "status")?;
        let error = get_optional_string(&row, "error");
        let row_count = get_optional_i64(&row, "row_count");
        let created_at_str = get_string_from_datetime(&row, "created_at")?;
        let updated_at_str = get_string_from_datetime(&row, "updated_at")?;

        let status = JobStatus::from_str(&status_str)
            .ok_or_else(|| IngestionError::Processing(format!("Invalid status: {}", status_str)))?;

        let created_at = parse_datetime(&created_at_str)?;
        let updated_at = parse_datetime(&updated_at_str)?;

        Ok(IngestionJob {
            id,
            tenant,
            file_name,
            s3_path,
            status,
            error,
            row_count,
            created_at,
            updated_at,
        })
    }
}
