use super::models::{IngestionError, IngestionJob};
use tracing::info;

pub struct JobProcessor;

impl JobProcessor {
    pub fn new() -> Self {
        Self
    }

    pub async fn process(&self, job: &IngestionJob) -> Result<i64, IngestionError> {
        info!(
            job_id = %job.id,
            file_name = %job.file_name,
            s3_path = %job.s3_path,
            "Processing job"
        );

        // TODO: Implement actual job processing logic here
        // 1. Fetch file from s3_path
        // 2. build and run copy command using the DBActions.
        // 3. Check everything worked and report if not.

        Ok(0)
    }
}

impl Default for JobProcessor {
    fn default() -> Self {
        Self::new()
    }
}
