use tauri::State;

use crate::annotation_core::{
    types::{
        Annotation, AnnotationListResponse, CreateAnnotationRequest, UpdateAnnotationRequest,
    },
    AnnotationCoreState,
};
use crate::document_core::DocumentCoreState;

#[tauri::command]
pub fn annot_create(
    state: State<'_, AnnotationCoreState>,
    doc_state: State<'_, DocumentCoreState>,
    request: CreateAnnotationRequest,
) -> Result<Annotation, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;

    // Try to get the document session for MuPDF-backed annotation creation.
    let session_arc = doc_state
        .store
        .get_session_arc_pub(&request.session_id)
        .ok();

    engine
        .create_with_session(request, session_arc.as_ref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annot_update(
    state: State<'_, AnnotationCoreState>,
    request: UpdateAnnotationRequest,
) -> Result<Annotation, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine.update(request).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annot_delete(
    state: State<'_, AnnotationCoreState>,
    session_id: String,
    annotation_id: String,
) -> Result<(), String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine
        .delete(&session_id, &annotation_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annot_list(
    state: State<'_, AnnotationCoreState>,
    session_id: String,
    page_index: Option<usize>,
) -> Result<AnnotationListResponse, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    Ok(engine.list(&session_id, page_index))
}

#[tauri::command]
pub fn annot_export_fdf(
    state: State<'_, AnnotationCoreState>,
    session_id: String,
) -> Result<String, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine.export_fdf(&session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annot_export_json(
    state: State<'_, AnnotationCoreState>,
    session_id: String,
) -> Result<String, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine.export_json(&session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn annot_import_fdf(
    state: State<'_, AnnotationCoreState>,
    session_id: String,
    fdf_string: String,
) -> Result<AnnotationListResponse, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine
        .import_fdf(&session_id, &fdf_string)
        .map_err(|e| e.to_string())
}
