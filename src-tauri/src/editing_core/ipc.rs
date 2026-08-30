use tauri::State;

use crate::document_core::DocumentCoreState;
use crate::editing_core::{
    errors::EditingCoreError,
    types::{EditTransaction, EditTransactionResult, UndoRedoState},
    EditingCoreState,
};

#[tauri::command]
pub fn edit_apply_transaction(
    editing_state: State<'_, EditingCoreState>,
    doc_state: State<'_, DocumentCoreState>,
    transaction: EditTransaction,
) -> Result<EditTransactionResult, String> {
    let session_id = transaction.session_id.clone();

    // Resolve the session Arc before locking the engine so we don't hold
    // both locks simultaneously (avoids potential deadlock ordering issues).
    let session_arc = doc_state
        .store
        .get_session_arc_pub(&session_id)
        .map_err(|e| e.to_string())?;

    let mut engine = editing_state.lock().map_err(|e| e.to_string())?;
    Ok(engine.apply_transaction(transaction, &session_arc))
}

#[tauri::command]
pub fn edit_undo(
    editing_state: State<'_, EditingCoreState>,
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<EditTransactionResult, String> {
    let session_arc = doc_state
        .store
        .get_session_arc_pub(&session_id)
        .map_err(|e| e.to_string())?;

    let mut engine = editing_state.lock().map_err(|e| e.to_string())?;
    engine.undo(&session_id, &session_arc).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn edit_redo(
    editing_state: State<'_, EditingCoreState>,
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<EditTransactionResult, String> {
    let session_arc = doc_state
        .store
        .get_session_arc_pub(&session_id)
        .map_err(|e| e.to_string())?;

    let mut engine = editing_state.lock().map_err(|e| e.to_string())?;
    engine.redo(&session_id, &session_arc).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn edit_get_undo_redo_state(
    editing_state: State<'_, EditingCoreState>,
    session_id: String,
) -> Result<UndoRedoState, String> {
    let engine = editing_state.lock().map_err(|e| e.to_string())?;
    Ok(engine.undo_redo_state(&session_id))
}
