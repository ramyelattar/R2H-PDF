use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkTarget {
    pub key: String,
    pub description: String,
    pub target_ms: u64,
    pub p95_target_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScenario {
    pub id: String,
    pub name: String,
    pub corpus_ref: String,
    pub targets: Vec<BenchmarkTarget>,
    pub warmup_runs: u32,
    pub measured_runs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkPlanResponse {
    pub scenarios: Vec<BenchmarkScenario>,
    pub baseline_hardware_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResultInput {
    pub scenario_id: String,
    pub metric_key: String,
    pub samples_ms: Vec<u64>,
    pub machine_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkAssessment {
    pub scenario_id: String,
    pub metric_key: String,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub target_ms: u64,
    pub p95_target_ms: u64,
    pub passed: bool,
    pub regression_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionTestCase {
    pub id: String,
    pub name: String,
    pub file_profile: String,
    pub critical_metrics: Vec<String>,
    pub max_regression_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCorpusAsset {
    pub id: String,
    pub class_name: String,
    pub page_count: usize,
    pub characteristics: Vec<String>,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCorpusStrategyResponse {
    pub assets: Vec<TestCorpusAsset>,
    pub governance_rules: Vec<String>,
    pub regression_suite: Vec<RegressionTestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPolicy {
    pub hard_ram_limit_mb: usize,
    pub soft_ram_limit_mb: usize,
    pub page_cache_limit_mb: usize,
    pub render_tile_limit: usize,
    pub ocr_worker_limit: usize,
    pub background_job_memory_budget_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshotInput {
    pub resident_set_mb: usize,
    pub page_cache_mb: usize,
    pub job_queue_mb: usize,
    pub open_documents: usize,
    pub in_flight_renders: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryControlAction {
    pub state: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryControlModelResponse {
    pub policy: MemoryPolicy,
    pub pressure_tiers: Vec<String>,
    pub eviction_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundJobType {
    Ocr,
    Reindex,
    Compare,
    PreRender,
    Diagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobPriority {
    UserBlocking,
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundJobRequest {
    pub job_type: BackgroundJobType,
    pub document_session_id: Option<String>,
    pub payload_ref: String,
    pub priority: JobPriority,
    pub max_runtime_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundJobTicket {
    pub job_id: String,
    pub queue_position: usize,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundJobState {
    pub job_id: String,
    pub job_type: BackgroundJobType,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub enqueue_epoch_ms: u128,
    pub updated_epoch_ms: u128,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCompletionInput {
    pub job_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutosavePolicy {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub max_checkpoint_files: usize,
    pub require_atomic_write: bool,
    pub require_integrity_hash: bool,
    pub fsync_on_commit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutosaveCheckpointRequest {
    pub session_id: String,
    pub source_path: String,
    pub checkpoint_path: String,
    pub bytes_written: usize,
    pub integrity_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutosaveRecord {
    pub checkpoint_id: String,
    pub session_id: String,
    pub source_path: String,
    pub checkpoint_path: String,
    pub bytes_written: usize,
    pub integrity_hash: String,
    pub epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutosaveValidationReport {
    pub checkpoint_id: String,
    pub valid: bool,
    pub checks: Vec<String>,
    pub blocking_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCheckpointRequest {
    pub session_id: String,
    pub document_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCheckpoint {
    pub checkpoint_id: String,
    pub session_id: String,
    pub document_path: String,
    pub reason: String,
    pub epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRestoreRequest {
    pub previous_session_id: String,
    pub checkpoint_id: Option<String>,
    pub crash_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRestoreResult {
    pub recovered: bool,
    pub restore_source: Option<String>,
    pub actions_taken: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEngineeringStrategyResponse {
    pub open_pipeline: Vec<String>,
    pub render_pipeline: Vec<String>,
    pub ocr_pipeline: Vec<String>,
    pub reliability_guards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryArchitectureResponse {
    pub autosave_rules: Vec<String>,
    pub checkpoint_model: Vec<String>,
    pub restore_flow: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformLayerDiagnostics {
    pub benchmark_results_recorded: usize,
    pub active_jobs: usize,
    pub autosave_records: usize,
    pub recovery_checkpoints: usize,
    pub memory_soft_limit_mb: usize,
    pub memory_hard_limit_mb: usize,
}
