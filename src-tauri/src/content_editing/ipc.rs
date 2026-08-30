//! IPC commands for native PDF content editing.

use tauri::State;
use crate::document_core::DocumentCoreState;
use super::types::*;
use super::analysis::extract_page_content_objects;
use super::text_edit::apply_native_text_edit;
use super::image_edit::{
    replace_native_image,
    delete_native_image,
    move_native_image,
    crop_native_image,
    rotate_native_image,
};
use super::history::ContentEditHistoryState;
use super::find_replace::{preview_find_replace, apply_find_replace_item};

/// Get all content objects on a page (text spans, images, paths).
#[tauri::command]
pub fn pdf_get_page_content_objects(
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<Vec<ContentObject>, String> {
    extract_page_content_objects(&doc_state, &session_id, page_index)
}

/// Apply a native text edit to a content object.
#[tauri::command]
pub fn pdf_apply_native_text_edit(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeTextEditRequest,
) -> Result<NativeTextEditResult, String> {
    // Capture session bytes before edit for undo snapshot.
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = apply_native_text_edit(&doc_state, &request)?;

    // Record in history with snapshot for undo.
    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "text_edit".to_string(),
            method: result.method.clone(),
            before_summary: format!("\"{}\"", result.original_text),
            after_summary: format!("\"{}\"", result.replacement_text),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None, // Will be set by add_record_with_snapshot
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }

    Ok(result)
}

/// Replace a native image XObject.
#[tauri::command]
pub fn pdf_replace_native_image(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeImageReplaceRequest,
) -> Result<NativeImageEditResult, String> {
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = replace_native_image(&doc_state, &request)?;

    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "image_replace".to_string(),
            method: result.method.clone(),
            before_summary: "Original image".to_string(),
            after_summary: "Replaced image".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }

    Ok(result)
}

/// Delete a native image from the page.
#[tauri::command]
pub fn pdf_delete_native_image(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeImageDeleteRequest,
) -> Result<NativeImageEditResult, String> {
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = delete_native_image(&doc_state, &request)?;

    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "image_delete".to_string(),
            method: result.method.clone(),
            before_summary: "Image present".to_string(),
            after_summary: "Image deleted".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }

    Ok(result)
}

/// Move/resize a native image.
#[tauri::command]
pub fn pdf_move_native_image(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeImageMoveRequest,
) -> Result<NativeImageEditResult, String> {
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = move_native_image(&doc_state, &request)?;
    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "image_move_resize".to_string(),
            method: result.method.clone(),
            before_summary: "Image original bbox".to_string(),
            after_summary: "Image moved/resized".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }
    Ok(result)
}

#[tauri::command]
pub fn pdf_preview_find_replace(
    doc_state: State<'_, DocumentCoreState>,
    request: FindReplacePreviewRequest,
) -> Result<FindReplacePreviewResult, String> {
    preview_find_replace(&doc_state, &request)
}

#[tauri::command]
pub fn pdf_apply_find_replace(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: FindReplaceApplyRequest,
) -> Result<FindReplaceApplyResult, String> {
    let mut applied_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut results = Vec::new();
    let mut warnings = Vec::new();

    for item in &request.matches {
        let before_snapshot = {
            let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
            let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
            session.document.bytes.clone()
        };
        let before_len = before_snapshot.len();
        match apply_find_replace_item(&doc_state, &request.session_id, &request.replace_text, item) {
            Ok(result) if result.success => {
                applied_count += 1;
                let record = ContentEditRecord {
                    edit_id: result.edit_id.clone(),
                    session_id: result.session_id.clone(),
                    page_index: result.page_index,
                    content_object_id: result.content_object_id.clone(),
                    edit_type: "find_replace".to_string(),
                    method: result.method.clone(),
                    before_summary: format!("\"{}\"", result.original_text),
                    after_summary: format!("\"{}\"", result.replacement_text),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0),
                    reversible: true,
                    warnings: result.warnings.clone(),
                    before_stream_snapshot: None,
                };
                history_state.add_record_with_snapshot(record, before_snapshot);
                results.push(FindReplaceApplyItemResult {
                    id: item.id.clone(),
                    page_index: item.page_index,
                    content_object_id: item.content_object_id.clone(),
                    success: true,
                    method: result.method,
                    warning: None,
                });
            }
            Ok(result) => {
                skipped_count += 1;
                let warning = result.warnings.join("; ");
                warnings.push(format!("Skipped {}: {}", item.id, warning));
                results.push(FindReplaceApplyItemResult {
                    id: item.id.clone(),
                    page_index: item.page_index,
                    content_object_id: item.content_object_id.clone(),
                    success: false,
                    method: result.method,
                    warning: Some(warning),
                });
                let after_len = {
                    let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
                    let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
                    session.document.bytes.len()
                };
                if after_len != before_len {
                    failed_count += 1;
                    warnings.push(format!("Find/replace item {} reported failure after byte mutation risk.", item.id));
                }
            }
            Err(e) => {
                failed_count += 1;
                warnings.push(format!("Failed {}: {}", item.id, e));
                results.push(FindReplaceApplyItemResult {
                    id: item.id.clone(),
                    page_index: item.page_index,
                    content_object_id: item.content_object_id.clone(),
                    success: false,
                    method: EditMethod::Rejected,
                    warning: Some(e),
                });
            }
        }
    }

    Ok(FindReplaceApplyResult {
        session_id: request.session_id,
        applied_count,
        skipped_count,
        failed_count,
        results,
        warnings,
    })
}

/// Crop a native image using export-safe visual redraw.
#[tauri::command]
pub fn pdf_crop_native_image(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeImageCropRequest,
) -> Result<NativeImageEditResult, String> {
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = crop_native_image(&doc_state, &request)?;
    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "image_crop".to_string(),
            method: result.method.clone(),
            before_summary: "Image uncropped".to_string(),
            after_summary: format!("Image cropped to {:?}", request.crop_rect),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }
    Ok(result)
}

/// Rotate a native image using export-safe visual redraw.
#[tauri::command]
pub fn pdf_rotate_native_image(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: NativeImageRotateRequest,
) -> Result<NativeImageEditResult, String> {
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id).map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = rotate_native_image(&doc_state, &request)?;
    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.content_object_id.clone(),
            edit_type: "image_rotation".to_string(),
            method: result.method.clone(),
            before_summary: "Image unrotated".to_string(),
            after_summary: format!("Image rotated {} degrees", request.degrees),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }
    Ok(result)
}

/// List all content edit records for a session.
#[tauri::command]
pub fn pdf_list_content_edits(
    history_state: State<'_, ContentEditHistoryState>,
    session_id: String,
) -> Vec<ContentEditRecord> {
    history_state.list_records(&session_id)
}

/// Revert a content edit by restoring session bytes from snapshot.
#[tauri::command]
pub fn pdf_revert_content_edit(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    session_id: String,
    edit_id: String,
) -> Result<(), String> {
    history_state.revert_edit(&doc_state, &session_id, &edit_id)
}

/// Clear all content edit history for a session.
#[tauri::command]
pub fn pdf_clear_content_edit_history(
    history_state: State<'_, ContentEditHistoryState>,
    session_id: String,
) -> Result<(), String> {
    history_state.clear_session(&session_id);
    Ok(())
}

// ── Phase 29A: page font registry ─────────────────────────────────────────

/// Inspect `/Resources /Font` on a page and return a structured registry
/// with per-font subtype/encoding/safety information.
#[tauri::command]
pub fn pdf_get_page_font_registry(
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<super::font_registry::PageFontRegistry, String> {
    super::font_registry::build_page_font_registry(&doc_state, &session_id, page_index)
}

// ── Phase 31B: page rotation metadata ──────────────────────────────

/// Phase 31B — read `/Rotate` from the page (inherited if absent) so the
/// frontend can disable the canvas inline editor on rotated pages.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageRotationInfo {
    pub session_id: String,
    pub page_index: usize,
    pub rotation_degrees: i32,
    /// Page size in PDF points (after applying rotation, MuPDF returns
    /// rotated bounds — we expose the raw page values too).
    pub width_pts: f32,
    pub height_pts: f32,
}

#[tauri::command]
pub fn pdf_get_page_rotation(
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<PageRotationInfo, String> {
    let arc = doc_state.store.get_session_arc_pub(&session_id).map_err(|e| e.to_string())?;
    let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
    let pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let count = pdf.page_count().map_err(|e| format!("page_count: {e}"))?;
    let pno = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    if pno >= count {
        return Err(format!("page index {page_index} out of range ({count})"));
    }
    let page = pdf.load_page(pno).map_err(|e| format!("load_page: {e}"))?;
    let pdf_page = mupdf::pdf::PdfPage::try_from(page).map_err(|e| format!("PdfPage: {e}"))?;
    let rotation = pdf_page.rotation().unwrap_or(0);
    // Use the session's cached page info for dimensions when available.
    let dims = session
        .document
        .pages
        .get(page_index)
        .map(|p| (p.width_points, p.height_points))
        .unwrap_or((612.0, 792.0));
    Ok(PageRotationInfo {
        session_id: session_id.clone(),
        page_index,
        rotation_degrees: rotation,
        width_pts: dims.0,
        height_pts: dims.1,
    })
}

// ── Phase 28D: text block editing ──────────────────────────────────────────

/// List text blocks on a page. Blocks are clusters of nearby text spans
/// that share font size and line spacing — the unit of paragraph-style
/// editing.
#[tauri::command]
pub fn pdf_prepare_text_block_edit(
    doc_state: State<'_, DocumentCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<Vec<super::types::TextBlock>, String> {
    let objects = extract_page_content_objects(&doc_state, &session_id, page_index)?;
    Ok(super::text_block::group_text_blocks(&objects))
}

/// Apply a text block edit. Picks `NativeMultiOperator` or `VisualReflow`
/// based on the strategy hint + block safety. Records the operation in
/// the content-edit history with a session-bytes snapshot for revert.
#[tauri::command]
pub fn pdf_apply_text_block_edit(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: super::types::TextBlockEditRequest,
) -> Result<super::types::TextBlockEditResult, String> {
    // Capture session bytes BEFORE the edit so revert can restore them.
    let before_snapshot = {
        let arc = doc_state.store.get_session_arc_pub(&request.session_id)
            .map_err(|e| e.to_string())?;
        let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes.clone()
    };

    let result = super::text_block::apply_text_block_edit(&doc_state, &request)?;

    if result.success {
        let record = ContentEditRecord {
            edit_id: result.edit_id.clone(),
            session_id: result.session_id.clone(),
            page_index: result.page_index,
            content_object_id: result.block_id.clone(),
            edit_type: "text_block_edit".to_string(),
            method: result.method.clone(),
            before_summary: format!("\"{}\"", truncate(&result.before_text, 80)),
            after_summary: format!("\"{}\"", truncate(&result.after_text, 80)),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            reversible: true,
            warnings: result.warnings.clone(),
            before_stream_snapshot: None,
        };
        history_state.add_record_with_snapshot(record, before_snapshot);
    }

    Ok(result)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { return s.to_string(); }
    let mut acc = String::with_capacity(max + 1);
    for (i, c) in s.chars().enumerate() {
        if i >= max - 1 { break; }
        acc.push(c);
    }
    acc.push('…');
    acc
}

// ── Phase 26D: vector/path safe visual removal ─────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CoverPathRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverPathSpec {
    /// Overlay id the frontend should use when creating the rectangle.
    pub id: String,
    pub page_index: usize,
    /// PDF user-space rect [x0, y0, x1, y1] (origin bottom-left).
    pub bbox: [f32; 4],
    pub method: String, // "safe_visual_removal"
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverPathResult {
    pub success: bool,
    pub spec: Option<CoverPathSpec>,
    pub warnings: Vec<String>,
    /// Honest disclaimer surfaced to the UI.
    pub method_label: String,
}

/// Phase 26D: produce a Safe Visual Removal spec for a detected path
/// content object. The frontend converts the returned spec into a
/// white-filled rectangle overlay (tagged `metadata.source = "path_cover"`)
/// which the export pipeline turns into a real PDF Square/FreeText
/// annotation visible externally.
///
/// This deliberately does NOT do true vector point editing; the
/// `method_label` makes that clear ("Safe Visual Removal — not true
/// vector point editing").
#[tauri::command]
pub fn pdf_cover_path_object(
    doc_state: State<'_, crate::document_core::DocumentCoreState>,
    history_state: State<'_, ContentEditHistoryState>,
    request: CoverPathRequest,
) -> Result<CoverPathResult, String> {
    let mut warnings: Vec<String> = Vec::new();

    // Locate the path object from the extracted content objects.
    let objects = extract_page_content_objects(&doc_state, &request.session_id, request.page_index)?;
    let Some(path_obj) = objects.into_iter().find(|o| o.id == request.content_object_id) else {
        return Err(format!(
            "Path object {} not found on page {}",
            request.content_object_id, request.page_index + 1
        ));
    };
    if !matches!(path_obj.object_type, super::types::ContentObjectType::Path) {
        return Err(format!(
            "Object {} is not a path/vector object (got {:?})",
            request.content_object_id, path_obj.object_type
        ));
    }
    let [x0, y0, x1, y1] = path_obj.bbox;
    if x1 <= x0 || y1 <= y0 {
        warnings.push("Path bbox is degenerate; the cover rectangle will be a no-op.".to_string());
        return Ok(CoverPathResult {
            success: false,
            spec: None,
            warnings,
            method_label: "Safe Visual Removal — not true vector point editing".to_string(),
        });
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let edit_id = format!("path-cover-{}-{}", request.page_index, now);
    let spec = CoverPathSpec {
        id: edit_id.clone(),
        page_index: request.page_index,
        bbox: path_obj.bbox,
        method: "safe_visual_removal".to_string(),
        diagnostics: vec![
            "Cover rectangle drawn over path bbox. Original vector geometry is preserved underneath.".to_string(),
            "Vector point editing is not implemented — full path mutation requires a separate phase.".to_string(),
        ],
    };

    // Record in content-edit history so the Objects panel + content-edit
    // panel show the action.
    let record = super::types::ContentEditRecord {
        edit_id: edit_id.clone(),
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: path_obj.id.clone(),
        edit_type: "path_cover".to_string(),
        method: super::types::EditMethod::SafeVisualReplacement,
        before_summary: format!("path bbox {:?}", path_obj.bbox),
        after_summary: format!("covered with rectangle {:?}", path_obj.bbox),
        timestamp: now,
        reversible: true,
        warnings: spec.diagnostics.clone(),
        before_stream_snapshot: None,
    };
    history_state.add_record(record);

    Ok(CoverPathResult {
        success: true,
        spec: Some(spec),
        warnings,
        method_label: "Safe Visual Removal — not true vector point editing".to_string(),
    })
}
