use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::InProgress => "in_progress",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(JobStatus::Pending),
            "in_progress" => Some(JobStatus::InProgress),
            "completed" => Some(JobStatus::Completed),
            "failed" => Some(JobStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionJob {
    pub id: String,
    pub tenant: String,
    pub file_name: String,
    pub s3_path: String,
    pub status: JobStatus,
    pub error: Option<String>,
    pub row_count: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IngestionJob {
    pub fn new(tenant: String, file_name: String, s3_path: String) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tenant,
            file_name,
            s3_path,
            status: JobStatus::Pending,
            error: None,
            row_count: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("Database error: {0}")]
    Database(#[from] database::DatabaseError),

    #[error("Processing error: {0}")]
    Processing(String),

    #[error("Job not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResponse {
    pub id: String,
    pub tenant: String,
    pub file_name: String,
    pub s3_path: String,
    pub status: JobStatus,
    pub error: Option<String>,
    pub row_count: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<IngestionJob> for JobResponse {
    fn from(job: IngestionJob) -> Self {
        Self {
            id: job.id,
            tenant: job.tenant,
            file_name: job.file_name,
            s3_path: job.s3_path,
            status: job.status,
            error: job.error,
            row_count: job.row_count,
            created_at: job.created_at,
            updated_at: job.updated_at,
        }
    }
}
