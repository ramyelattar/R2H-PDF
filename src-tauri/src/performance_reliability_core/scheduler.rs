use std::collections::VecDeque;

use super::errors::PlatformLayerError;
use super::time::now_epoch_ms;
use super::types::{
    BackgroundJobRequest, BackgroundJobState, BackgroundJobTicket, JobCompletionInput, JobStatus,
};

pub struct BackgroundScheduler {
    queue: VecDeque<BackgroundJobState>,
    next_job_id: u64,
}

impl BackgroundScheduler {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            next_job_id: 1,
        }
    }

    pub fn enqueue(&mut self, request: BackgroundJobRequest) -> Result<BackgroundJobTicket, PlatformLayerError> {
        if request.max_runtime_ms == 0 {
            return Err(PlatformLayerError::InvalidInput(
                "max_runtime_ms must be greater than zero".to_string(),
            ));
        }

        let job_id = format!("bg-job-{}", self.next_job_id);
        self.next_job_id += 1;

        let now = now_epoch_ms();
        let state = BackgroundJobState {
            job_id: job_id.clone(),
            job_type: request.job_type,
            priority: request.priority,
            status: JobStatus::Queued,
            enqueue_epoch_ms: now,
            updated_epoch_ms: now,
            error: None,
        };

        self.queue.push_back(state);

        Ok(BackgroundJobTicket {
            job_id,
            queue_position: self.queue.len().saturating_sub(1),
            accepted: true,
        })
    }

    pub fn mark_running(&mut self, job_id: &str) -> Result<BackgroundJobState, PlatformLayerError> {
        let job = self
            .queue
            .iter_mut()
            .find(|job| job.job_id == job_id)
            .ok_or_else(|| PlatformLayerError::NotFound(format!("job {job_id}")))?;

        job.status = JobStatus::Running;
        job.updated_epoch_ms = now_epoch_ms();

        Ok(job.clone())
    }

    pub fn complete(&mut self, input: JobCompletionInput) -> Result<BackgroundJobState, PlatformLayerError> {
        let job = self
            .queue
            .iter_mut()
            .find(|job| job.job_id == input.job_id)
            .ok_or_else(|| PlatformLayerError::NotFound(format!("job {}", input.job_id)))?;

        job.status = if input.success {
            JobStatus::Completed
        } else {
            JobStatus::Failed
        };
        job.error = input.error;
        job.updated_epoch_ms = now_epoch_ms();

        Ok(job.clone())
    }

    pub fn list(&self) -> Vec<BackgroundJobState> {
        self.queue.iter().cloned().collect()
    }

    pub fn active_count(&self) -> usize {
        self.queue
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Queued | JobStatus::Running))
            .count()
    }
}
