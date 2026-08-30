use std::collections::VecDeque;

use super::errors::PlatformLayerError;
use super::time::now_epoch_ms;
use super::types::{
    AutosaveCheckpointRequest, AutosavePolicy, AutosaveRecord, AutosaveValidationReport,
};

pub struct AutosaveController {
    pub policy: AutosavePolicy,
    records: VecDeque<AutosaveRecord>,
    next_id: u64,
}

impl AutosaveController {
    pub fn new() -> Self {
        Self {
            policy: AutosavePolicy {
                enabled: true,
                interval_seconds: 45,
                max_checkpoint_files: 8,
                require_atomic_write: true,
                require_integrity_hash: true,
                fsync_on_commit: true,
            },
            records: VecDeque::new(),
            next_id: 1,
        }
    }

    pub fn record_checkpoint(
        &mut self,
        request: AutosaveCheckpointRequest,
    ) -> Result<AutosaveRecord, PlatformLayerError> {
        if request.integrity_hash.trim().is_empty() {
            return Err(PlatformLayerError::IntegrityError(
                "integrity hash is required".to_string(),
            ));
        }

        let record = AutosaveRecord {
            checkpoint_id: format!("autosave-{}", self.next_id),
            session_id: request.session_id,
            source_path: request.source_path,
            checkpoint_path: request.checkpoint_path,
            bytes_written: request.bytes_written,
            integrity_hash: request.integrity_hash,
            epoch_ms: now_epoch_ms(),
        };
        self.next_id += 1;

        self.records.push_front(record.clone());

        while self.records.len() > self.policy.max_checkpoint_files {
            self.records.pop_back();
        }

        Ok(record)
    }

    pub fn validate(&self, checkpoint_id: &str) -> Result<AutosaveValidationReport, PlatformLayerError> {
        let record = self
            .records
            .iter()
            .find(|record| record.checkpoint_id == checkpoint_id)
            .ok_or_else(|| PlatformLayerError::NotFound(format!("autosave checkpoint {checkpoint_id}")))?;

        let mut checks = vec![
            "Checkpoint record exists".to_string(),
            "Checkpoint includes integrity hash".to_string(),
        ];
        let mut blocking_issues = Vec::new();

        if record.integrity_hash.trim().is_empty() {
            blocking_issues.push("Missing integrity hash".to_string());
        }
        if record.bytes_written == 0 {
            blocking_issues.push("Zero-byte checkpoint".to_string());
        }

        if blocking_issues.is_empty() {
            checks.push("Byte count is non-zero".to_string());
        }

        Ok(AutosaveValidationReport {
            checkpoint_id: checkpoint_id.to_string(),
            valid: blocking_issues.is_empty(),
            checks,
            blocking_issues,
        })
    }

    pub fn records_count(&self) -> usize {
        self.records.len()
    }
}
