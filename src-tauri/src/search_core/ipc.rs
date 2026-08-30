use tauri::State;

use crate::search_core::{
    types::{IndexStatus, PageTextWithSpans, SearchQuery, SearchResponse},
    SearchCoreState,
};

#[tauri::command]
pub fn search_query(
    state: State<'_, SearchCoreState>,
    query: SearchQuery,
) -> Result<SearchResponse, String> {
    let engine = state.lock().map_err(|e| e.to_string())?;
    engine.query(query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_index_document(
    state: State<'_, SearchCoreState>,
    session_id: String,
    pages: Vec<String>,
) -> Result<IndexStatus, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    Ok(engine.index_document(session_id, pages))
}

#[tauri::command]
pub fn search_index_document_with_spans(
    state: State<'_, SearchCoreState>,
    session_id: String,
    pages: Vec<PageTextWithSpans>,
) -> Result<IndexStatus, String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    Ok(engine.index_document_with_spans(session_id, pages))
}

#[tauri::command]
pub fn search_get_index_status(
    state: State<'_, SearchCoreState>,
    session_id: String,
) -> Result<IndexStatus, String> {
    let engine = state.lock().map_err(|e| e.to_string())?;
    Ok(engine.get_index_status(&session_id))
}

#[tauri::command]
pub fn search_clear_index(
    state: State<'_, SearchCoreState>,
    session_id: String,
) -> Result<(), String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine.clear_index(&session_id);
    Ok(())
}

#[tauri::command]
pub fn search_set_active_page(
    state: State<'_, SearchCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<(), String> {
    let mut engine = state.lock().map_err(|e| e.to_string())?;
    engine.set_active_page(session_id, page_index);
    Ok(())
}
