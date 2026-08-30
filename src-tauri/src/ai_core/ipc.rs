use tauri::State;

use crate::ai_core::{
    types::{AiStatusResponse, AiTaskRequest, AiTaskResult},
    AiCoreState,
};

#[tauri::command]
pub async fn ai_run_task(
    state: State<'_, AiCoreState>,
    request: AiTaskRequest,
) -> Result<AiTaskResult, String> {
    let task_id = request.task_id.clone();
    let mut engine = state.lock().await;
    match engine.run_task(request).await {
        Ok(result) => Ok(result),
        Err(e) => Ok(AiTaskResult {
            task_id,
            success: false,
            output_text: None,
            suggestions: None,
            entities: None,
            model_used: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn ai_get_status(state: State<'_, AiCoreState>) -> Result<AiStatusResponse, String> {
    let engine = state.lock().await;
    Ok(engine.get_status().await)
}

#[tauri::command]
pub async fn ai_cancel_task(
    state: State<'_, AiCoreState>,
    task_id: String,
) -> Result<(), String> {
    let mut engine = state.lock().await;
    engine.cancel_task(&task_id).map_err(|e| e.to_string())
}

// ── Local AI Runtime Commands ───────────────────────────────────────────────

use crate::ai_core::{
    LocalAiState,
    local_types::{LocalGenerateRequest, LocalGenerateResult, LocalModelInfo, LocalRuntimeStatus},
};

#[tauri::command]
pub fn ai_list_local_models(
    state: State<'_, LocalAiState>,
) -> Result<Vec<LocalModelInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "local AI lock poisoned".to_string())?;
    Ok(runtime.registry.list_models())
}

#[tauri::command]
pub fn ai_get_local_runtime_status(
    state: State<'_, LocalAiState>,
) -> Result<LocalRuntimeStatus, String> {
    let runtime = state.runtime.lock().map_err(|_| "local AI lock poisoned".to_string())?;
    Ok(runtime.status())
}

#[tauri::command]
pub fn ai_validate_local_model(
    state: State<'_, LocalAiState>,
    model_id: String,
) -> Result<LocalModelInfo, String> {
    let runtime = state.runtime.lock().map_err(|_| "local AI lock poisoned".to_string())?;
    runtime.registry.validate_model(&model_id)
}

#[tauri::command]
pub fn ai_generate_local(
    state: State<'_, LocalAiState>,
    request: LocalGenerateRequest,
) -> Result<LocalGenerateResult, String> {
    let runtime = state.runtime.lock().map_err(|_| "local AI lock poisoned".to_string())?;
    runtime.generate(request)
}


// ── RAG Commands ────────────────────────────────────────────────────────────

use crate::ai_core::rag::{
    RagState, BuildIndexRequest, RagQuestionRequest, RagAnswerResult,
};
use crate::ai_core::vector_index::IndexStatus;

#[tauri::command]
pub fn ai_build_document_index(
    rag_state: State<'_, RagState>,
    request: BuildIndexRequest,
) -> Result<IndexStatus, String> {
    let mut engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    let mut embedding = rag_state.embedding.lock().map_err(|_| "Embedding lock poisoned".to_string())?;
    Ok(engine.build_index(request, Some(&mut *embedding)))
}

#[tauri::command]
pub fn ai_get_document_index_status(
    rag_state: State<'_, RagState>,
    session_id: String,
) -> Result<IndexStatus, String> {
    let engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    Ok(engine.get_index_status(&session_id))
}

#[tauri::command]
pub fn ai_clear_document_index(
    rag_state: State<'_, RagState>,
    session_id: String,
) -> Result<(), String> {
    let mut engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    engine.clear_index(&session_id);
    Ok(())
}

#[tauri::command]
pub fn ai_search_document_semantic(
    rag_state: State<'_, RagState>,
    session_id: String,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<crate::ai_core::vector_index::VectorSearchResult>, String> {
    let engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    // For semantic search without explicit query embedding, use BM25 path.
    Ok(engine.search(&session_id, &query, top_k.unwrap_or(5), None))
}

#[tauri::command]
pub fn ai_ask_document_rag(
    rag_state: State<'_, RagState>,
    local_ai_state: State<'_, LocalAiState>,
    request: RagQuestionRequest,
) -> Result<RagAnswerResult, String> {
    crate::license::assert_feature_allowed("rag")?;

    let engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    let runtime = local_ai_state.runtime.lock().map_err(|_| "local AI lock poisoned".to_string())?;
    let embedding = rag_state.embedding.lock().map_err(|_| "Embedding lock poisoned".to_string())?;
    engine.ask(&request, &runtime, Some(&*embedding))
}


// ── AI Action Planner Commands ──────────────────────────────────────────────

use crate::ai_core::action_planner::ActionPlannerState;
use crate::ai_core::action_types::{AiActionBatch, PlanActionsRequest};

#[tauri::command]
pub fn ai_plan_document_actions(
    planner_state: State<'_, ActionPlannerState>,
    rag_state: State<'_, RagState>,
    local_ai_state: State<'_, LocalAiState>,
    request: PlanActionsRequest,
) -> Result<AiActionBatch, String> {
    let mut planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    let rag_engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    let runtime = local_ai_state.runtime.lock().map_err(|_| "Local AI lock poisoned".to_string())?;
    let embedding = rag_state.embedding.lock().map_err(|_| "Embedding lock poisoned".to_string())?;
    planner.plan(&request, &rag_engine, &runtime, Some(&*embedding))
}

#[tauri::command]
pub fn ai_get_action_batch(
    planner_state: State<'_, ActionPlannerState>,
    batch_id: String,
) -> Result<Option<AiActionBatch>, String> {
    let planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    Ok(planner.get_batch(&batch_id).cloned())
}

#[tauri::command]
pub fn ai_accept_action(
    planner_state: State<'_, ActionPlannerState>,
    batch_id: String,
    action_id: String,
) -> Result<(), String> {
    let mut planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    planner.accept_action(&batch_id, &action_id)
}

#[tauri::command]
pub fn ai_reject_action(
    planner_state: State<'_, ActionPlannerState>,
    batch_id: String,
    action_id: String,
) -> Result<(), String> {
    let mut planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    planner.reject_action(&batch_id, &action_id)
}

#[tauri::command]
pub fn ai_apply_action_batch(
    planner_state: State<'_, ActionPlannerState>,
    batch_id: String,
) -> Result<Vec<String>, String> {
    // Mark accepted actions as applied. Returns list of applied action IDs.
    // Actual editor object creation happens on the frontend.
    let mut planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    let batch = planner.batches.get_mut(&batch_id).ok_or("Batch not found")?;
    let mut applied_ids = Vec::new();
    for action in &mut batch.actions {
        if matches!(action.status, crate::ai_core::action_types::AiActionStatus::Accepted) {
            action.status = crate::ai_core::action_types::AiActionStatus::Applied;
            applied_ids.push(action.action_id.clone());
        }
    }
    Ok(applied_ids)
}

#[tauri::command]
pub fn ai_clear_action_batch(
    planner_state: State<'_, ActionPlannerState>,
    batch_id: String,
) -> Result<(), String> {
    let mut planner = planner_state.planner.lock().map_err(|_| "Planner lock poisoned".to_string())?;
    planner.clear_batch(&batch_id);
    Ok(())
}


// ── Document Review Commands ────────────────────────────────────────────────

use crate::ai_core::review::{DocumentReviewRequest, DocumentReviewResult};
use std::sync::Mutex as StdMutex;

static LAST_REVIEW: once_cell::sync::Lazy<StdMutex<Option<DocumentReviewResult>>> =
    once_cell::sync::Lazy::new(|| StdMutex::new(None));

#[tauri::command]
pub fn ai_review_document(
    rag_state: State<'_, RagState>,
    local_ai_state: State<'_, LocalAiState>,
    eng_state: State<'_, crate::engineering::EngineeringState>,
    request: DocumentReviewRequest,
) -> Result<DocumentReviewResult, String> {
    let rag_engine = rag_state.engine.lock().map_err(|_| "RAG lock poisoned".to_string())?;
    let runtime = local_ai_state.runtime.lock().map_err(|_| "Local AI lock poisoned".to_string())?;
    let embedding = rag_state.embedding.lock().map_err(|_| "Embedding lock poisoned".to_string())?;

    let eng_findings = if request.include_engineering {
        eng_state.findings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    } else {
        Vec::new()
    };

    let result = crate::ai_core::review::run_review(
        &request, &rag_engine, &runtime, Some(&*embedding), eng_findings,
    )?;

    *LAST_REVIEW.lock().unwrap() = Some(result.clone());
    Ok(result)
}

#[tauri::command]
pub fn ai_get_review_result() -> Option<DocumentReviewResult> {
    LAST_REVIEW.lock().unwrap().clone()
}

#[tauri::command]
pub fn ai_clear_review_result() -> Result<(), String> {
    let mut review = LAST_REVIEW
        .lock()
        .map_err(|_| "AI review result lock is poisoned".to_string())?;
    *review = None;
    Ok(())
}


// ── Local AI Path / Model Manager Commands ──────────────────────────────────

use crate::ai_core::local_ai_paths::{
    resolve_local_ai_root, validate_local_ai_root, LocalAiValidationResult,
};

#[tauri::command]
pub fn local_ai_get_status() -> LocalAiValidationResult {
    let root = resolve_local_ai_root(None);
    validate_local_ai_root(&root)
}

#[tauri::command]
pub fn local_ai_validate(path: Option<String>) -> LocalAiValidationResult {
    let root = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => resolve_local_ai_root(None),
    };
    validate_local_ai_root(&root)
}
