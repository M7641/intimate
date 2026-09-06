use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info};

use super::processor::JobProcessor;
use super::repository::JobRepository;

pub struct JobWorker {
    repository: Arc<JobRepository>,
    processor: Arc<JobProcessor>,
    poll_interval_secs: u64,
}

impl JobWorker {
    pub fn new(repository: Arc<JobRepository>, processor: Arc<JobProcessor>) -> Self {
        Self {
            repository,
            processor,
            poll_interval_secs: 5,
        }
    }

    pub fn with_poll_interval(mut self, secs: u64) -> Self {
        self.poll_interval_secs = secs;
        self
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        info!("Job worker started");

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Job worker received shutdown signal");
                        break;
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(self.poll_interval_secs)) => {
                    self.process_pending_jobs().await;
                }
            }
        }

        info!("Job worker stopped");
    }

    async fn process_pending_jobs(&self) {
        let jobs = match self.repository.get_pending_jobs(10).await {
            Ok(jobs) => jobs,
            Err(e) => {
                error!(error = %e, "Failed to fetch pending jobs");
                return;
            }
        };

        if jobs.is_empty() {
            debug!("No pending jobs");
            return;
        }

        info!(count = jobs.len(), "Processing pending jobs");

        for job in jobs {
            if let Err(e) = self.repository.set_in_progress(&job.id).await {
                error!(job_id = %job.id, error = %e, "Failed to set job in progress");
                continue;
            }

            match self.processor.process(&job).await {
                Ok(row_count) => {
                    if let Err(e) = self.repository.complete_job(&job.id, Some(row_count)).await {
                        error!(job_id = %job.id, error = %e, "Failed to mark job complete");
                    } else {
                        info!(job_id = %job.id, rows = row_count, "Job completed");
                    }
                }
                Err(e) => {
                    error!(job_id = %job.id, error = %e, "Job failed");
                    if let Err(e) = self.repository.fail_job(&job.id, &e.to_string()).await {
                        error!(job_id = %job.id, error = %e, "Failed to mark job as failed");
                    }
                }
            }
        }
    }
}
