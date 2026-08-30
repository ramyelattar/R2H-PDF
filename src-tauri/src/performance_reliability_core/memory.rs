use super::types::{MemoryControlAction, MemoryControlModelResponse, MemoryPolicy, MemorySnapshotInput};

pub struct MemoryController {
    pub policy: MemoryPolicy,
}

impl MemoryController {
    pub fn new() -> Self {
        Self {
            policy: MemoryPolicy {
                hard_ram_limit_mb: 4096,
                soft_ram_limit_mb: 3072,
                page_cache_limit_mb: 1024,
                render_tile_limit: 192,
                ocr_worker_limit: 4,
                background_job_memory_budget_mb: 1024,
            },
        }
    }

    pub fn model(&self) -> MemoryControlModelResponse {
        MemoryControlModelResponse {
            policy: self.policy.clone(),
            pressure_tiers: vec![
                "normal (< soft limit)".to_string(),
                "elevated (>= soft limit)".to_string(),
                "critical (>= hard limit)".to_string(),
            ],
            eviction_order: vec![
                "drop far-page render tiles".to_string(),
                "shrink page cache generations".to_string(),
                "pause low-priority background jobs".to_string(),
                "reduce OCR workers".to_string(),
                "force emergency cache compaction".to_string(),
            ],
        }
    }

    pub fn evaluate(&self, snapshot: MemorySnapshotInput) -> MemoryControlAction {
        let mut actions = Vec::new();
        let state = if snapshot.resident_set_mb >= self.policy.hard_ram_limit_mb {
            actions.push("Pause low-priority jobs".to_string());
            actions.push("Trim render/page caches aggressively".to_string());
            actions.push("Throttle OCR workers to 1".to_string());
            "critical"
        } else if snapshot.resident_set_mb >= self.policy.soft_ram_limit_mb {
            actions.push("Trim non-visible page cache".to_string());
            actions.push("Limit pre-render depth".to_string());
            actions.push("Delay compare/reindex jobs".to_string());
            "elevated"
        } else {
            actions.push("No memory intervention required".to_string());
            "normal"
        };

        MemoryControlAction {
            state: state.to_string(),
            actions,
        }
    }
}
