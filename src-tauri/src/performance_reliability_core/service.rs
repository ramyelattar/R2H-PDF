use std::sync::{Arc, Mutex};

use super::autosave::AutosaveController;
use super::benchmark::{assess, default_scenarios};
use super::corpus::strategy as corpus_strategy;
use super::errors::PlatformLayerError;
use super::memory::MemoryController;
use super::recovery::RecoveryController;
use super::scheduler::BackgroundScheduler;
use super::strategy::performance_strategy;
use super::types::*;

pub struct PerformanceReliabilityManager {
    scenarios: Vec<BenchmarkScenario>,
    assessments: Vec<BenchmarkAssessment>,
    memory: MemoryController,
    scheduler: BackgroundScheduler,
    autosave: AutosaveController,
    recovery: RecoveryController,
}

impl PerformanceReliabilityManager {
    pub fn new() -> Self {
        Self {
            scenarios: default_scenarios(),
            assessments: Vec::new(),
            memory: MemoryController::new(),
            scheduler: BackgroundScheduler::new(),
            autosave: AutosaveController::new(),
            recovery: RecoveryController::new(),
        }
    }

    pub fn benchmark_plan(&self) -> BenchmarkPlanResponse {
        BenchmarkPlanResponse {
            scenarios: self.scenarios.clone(),
            baseline_hardware_note:
                "Modern machine baseline: 8+ performance cores, 32GB RAM, NVMe SSD".to_string(),
        }
    }

    pub fn submit_benchmark_result(
        &mut self,
        input: BenchmarkResultInput,
    ) -> Result<BenchmarkAssessment, PlatformLayerError> {
        let assessment = assess(input, &self.scenarios)?;
        self.assessments.push(assessment.clone());
        Ok(assessment)
    }

    pub fn performance_strategy(&self) -> PerformanceEngineeringStrategyResponse {
        performance_strategy()
    }

    pub fn memory_model(&self) -> MemoryControlModelResponse {
        self.memory.model()
    }

    pub fn evaluate_memory(&self, snapshot: MemorySnapshotInput) -> MemoryControlAction {
        self.memory.evaluate(snapshot)
    }

    pub fn queue_job(
        &mut self,
        request: BackgroundJobRequest,
    ) -> Result<BackgroundJobTicket, PlatformLayerError> {
        self.scheduler.enqueue(request)
    }

    pub fn mark_job_running(&mut self, job_id: String) -> Result<BackgroundJobState, PlatformLayerError> {
        self.scheduler.mark_running(&job_id)
    }

    pub fn complete_job(&mut self, input: JobCompletionInput) -> Result<BackgroundJobState, PlatformLayerError> {
        self.scheduler.complete(input)
    }

    pub fn jobs(&self) -> Vec<BackgroundJobState> {
        self.scheduler.list()
    }

    pub fn record_autosave(
        &mut self,
        request: AutosaveCheckpointRequest,
    ) -> Result<AutosaveRecord, PlatformLayerError> {
        self.autosave.record_checkpoint(request)
    }

    pub fn validate_autosave(
        &self,
        checkpoint_id: String,
    ) -> Result<AutosaveValidationReport, PlatformLayerError> {
        self.autosave.validate(&checkpoint_id)
    }

    pub fn create_recovery_checkpoint(
        &mut self,
        request: RecoveryCheckpointRequest,
    ) -> Result<RecoveryCheckpoint, PlatformLayerError> {
        self.recovery.create_checkpoint(request)
    }

    pub fn restore_after_crash(
        &self,
        request: CrashRestoreRequest,
    ) -> Result<CrashRestoreResult, PlatformLayerError> {
        self.recovery.restore_after_crash(request)
    }

    pub fn recovery_architecture(&self) -> RecoveryArchitectureResponse {
        RecoveryController::architecture()
    }

    pub fn test_corpus_strategy(&self) -> TestCorpusStrategyResponse {
        corpus_strategy()
    }

    pub fn diagnostics(&self) -> PlatformLayerDiagnostics {
        PlatformLayerDiagnostics {
            benchmark_results_recorded: self.assessments.len(),
            active_jobs: self.scheduler.active_count(),
            autosave_records: self.autosave.records_count(),
            recovery_checkpoints: self.recovery.checkpoints_count(),
            memory_soft_limit_mb: self.memory.policy.soft_ram_limit_mb,
            memory_hard_limit_mb: self.memory.policy.hard_ram_limit_mb,
        }
    }
}

#[derive(Clone)]
pub struct PerformanceReliabilityState {
    pub manager: Arc<Mutex<PerformanceReliabilityManager>>,
}

impl PerformanceReliabilityState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(PerformanceReliabilityManager::new())),
        }
    }
}
