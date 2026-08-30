use tauri::State;

use super::errors::PlatformLayerError;
use super::service::PerformanceReliabilityState;
use super::types::*;

fn map_error(err: PlatformLayerError) -> String {
    err.to_string()
}

#[tauri::command]
pub fn perf_get_benchmark_plan(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<BenchmarkPlanResponse, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.benchmark_plan())
}

#[tauri::command]
pub fn perf_submit_benchmark_result(
    state: State<'_, PerformanceReliabilityState>,
    input: BenchmarkResultInput,
) -> Result<BenchmarkAssessment, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.submit_benchmark_result(input).map_err(map_error)
}

#[tauri::command]
pub fn perf_get_strategy(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<PerformanceEngineeringStrategyResponse, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.performance_strategy())
}

#[tauri::command]
pub fn perf_get_memory_model(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<MemoryControlModelResponse, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.memory_model())
}

#[tauri::command]
pub fn perf_evaluate_memory(
    state: State<'_, PerformanceReliabilityState>,
    snapshot: MemorySnapshotInput,
) -> Result<MemoryControlAction, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.evaluate_memory(snapshot))
}

#[tauri::command]
pub fn perf_schedule_job(
    state: State<'_, PerformanceReliabilityState>,
    request: BackgroundJobRequest,
) -> Result<BackgroundJobTicket, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.queue_job(request).map_err(map_error)
}

#[tauri::command]
pub fn perf_mark_job_running(
    state: State<'_, PerformanceReliabilityState>,
    job_id: String,
) -> Result<BackgroundJobState, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.mark_job_running(job_id).map_err(map_error)
}

#[tauri::command]
pub fn perf_complete_job(
    state: State<'_, PerformanceReliabilityState>,
    input: JobCompletionInput,
) -> Result<BackgroundJobState, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.complete_job(input).map_err(map_error)
}

#[tauri::command]
pub fn perf_list_jobs(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<Vec<BackgroundJobState>, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.jobs())
}

#[tauri::command]
pub fn perf_record_autosave(
    state: State<'_, PerformanceReliabilityState>,
    request: AutosaveCheckpointRequest,
) -> Result<AutosaveRecord, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.record_autosave(request).map_err(map_error)
}

#[tauri::command]
pub fn perf_validate_autosave(
    state: State<'_, PerformanceReliabilityState>,
    checkpoint_id: String,
) -> Result<AutosaveValidationReport, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.validate_autosave(checkpoint_id).map_err(map_error)
}

#[tauri::command]
pub fn perf_create_recovery_checkpoint(
    state: State<'_, PerformanceReliabilityState>,
    request: RecoveryCheckpointRequest,
) -> Result<RecoveryCheckpoint, String> {
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager
        .create_recovery_checkpoint(request)
        .map_err(map_error)
}

#[tauri::command]
pub fn perf_restore_after_crash(
    state: State<'_, PerformanceReliabilityState>,
    request: CrashRestoreRequest,
) -> Result<CrashRestoreResult, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    manager.restore_after_crash(request).map_err(map_error)
}

#[tauri::command]
pub fn perf_get_recovery_architecture(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<RecoveryArchitectureResponse, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.recovery_architecture())
}

#[tauri::command]
pub fn perf_get_test_corpus_strategy(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<TestCorpusStrategyResponse, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.test_corpus_strategy())
}

#[tauri::command]
pub fn perf_diagnostics(
    state: State<'_, PerformanceReliabilityState>,
) -> Result<PlatformLayerDiagnostics, String> {
    let manager = state
        .manager
        .lock()
        .map_err(|_| PlatformLayerError::LockPoisoned)
        .map_err(map_error)?;
    Ok(manager.diagnostics())
}
