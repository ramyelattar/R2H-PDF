use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::errors::DocumentCoreError;
use super::session::DocumentCoreState;
use super::types::{
    FormField, IncrementalSaveRequest, IncrementalSaveResponse, NavigateRequest, NavigateResponse,
    OpenDocumentRequest, OpenDocumentResponse, RecoveryReport, RenderRequest, SessionDiagnostics,
    SessionStateResponse, TextExtractionRequest, TextExtractionResponse,
};
use crate::editing_core::EditingCoreState;
use crate::search_core::SearchCoreState;

fn map_error(err: DocumentCoreError) -> String {
    err.to_string()
}

/// IPC-safe render response: `pixels_rgba` is base64-encoded to avoid
/// the cost of serializing a large JSON integer array across the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRenderResponse {
    pub session_id: String,
    pub page_index: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub width: u32,
    pub height: u32,
    pub zoom: f32,
    pub cache_hit: bool,
    pub pixels_rgba: String,
    pub render_time_ms: u128,
    /// Page width in PDF points (zoom-independent). Required so the
    /// canvas hit-test overlay and inline text editor can map PDF
    /// user-space bboxes back to screen coordinates correctly.
    #[serde(default)]
    pub width_pts: f32,
    /// Page height in PDF points (zoom-independent).
    #[serde(default)]
    pub height_pts: f32,
}

impl From<super::types::RenderResponse> for IpcRenderResponse {
    fn from(r: super::types::RenderResponse) -> Self {
        Self {
            session_id: r.session_id,
            page_index: r.page_index,
            width_px: r.width_px,
            height_px: r.height_px,
            width: r.width,
            height: r.height,
            zoom: r.zoom,
            cache_hit: r.cache_hit,
            pixels_rgba: general_purpose::STANDARD.encode(&r.pixels_rgba),
            render_time_ms: r.render_time_ms,
            width_pts: r.width_pts,
            height_pts: r.height_pts,
        }
    }
}

#[tauri::command]
pub fn doc_open(
    state: State<'_, DocumentCoreState>,
    request: OpenDocumentRequest,
) -> Result<OpenDocumentResponse, String> {
    state.store.open_document(request).map_err(map_error)
}

#[tauri::command]
pub fn doc_close(
    state: State<'_, DocumentCoreState>,
    editing_state: State<'_, EditingCoreState>,
    session_id: String,
) -> Result<(), String> {
    state.store.close_document(&session_id).map_err(map_error)?;
    if let Ok(mut engine) = editing_state.lock() {
        engine.clear_session(&session_id);
    }
    Ok(())
}

#[tauri::command]
pub fn doc_render(
    state: State<'_, DocumentCoreState>,
    request: RenderRequest,
) -> Result<IpcRenderResponse, String> {
    state
        .store
        .render_page(request)
        .map(IpcRenderResponse::from)
        .map_err(map_error)
}

#[tauri::command]
pub fn doc_extract_text(
    state: State<'_, DocumentCoreState>,
    request: TextExtractionRequest,
) -> Result<TextExtractionResponse, String> {
    state.store.extract_text(request).map_err(map_error)
}

#[tauri::command]
pub fn doc_navigate(
    state: State<'_, DocumentCoreState>,
    search_state: State<'_, SearchCoreState>,
    request: NavigateRequest,
) -> Result<NavigateResponse, String> {
    let response = state.store.navigate(request.clone()).map_err(map_error)?;
    if let Ok(mut engine) = search_state.lock() {
        engine.set_active_page(request.session_id, request.page_index);
    }
    Ok(response)
}

#[tauri::command]
pub fn doc_incremental_save(
    state: State<'_, DocumentCoreState>,
    request: IncrementalSaveRequest,
) -> Result<IncrementalSaveResponse, String> {
    state.store.incremental_save(request).map_err(map_error)
}

#[tauri::command]
pub fn doc_recover(
    state: State<'_, DocumentCoreState>,
    path: String,
) -> Result<RecoveryReport, String> {
    state.store.recover_file(path).map_err(map_error)
}

#[tauri::command]
pub fn doc_diagnostics(
    state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<SessionDiagnostics, String> {
    state.store.diagnostics(session_id).map_err(map_error)
}

#[tauri::command]
pub fn open_pdf(
    state: State<'_, DocumentCoreState>,
    path: String,
) -> Result<OpenDocumentResponse, String> {
    state
        .store
        .open_document(OpenDocumentRequest {
            path,
            recover_if_damaged: true,
        })
        .map_err(map_error)
}

#[tauri::command]
pub fn render_page(
    state: State<'_, DocumentCoreState>,
    session_id: String,
    page_index: usize,
    zoom: f32,
) -> Result<IpcRenderResponse, String> {
    state
        .store
        .render_page(RenderRequest {
            session_id,
            page_index,
            zoom,
            viewport: super::types::BBox {
                x: 0.0,
                y: 0.0,
                width: 1024.0,
                height: 1448.0,
            },
            device_pixel_ratio: 1.0,
        })
        .map(IpcRenderResponse::from)
        .map_err(map_error)
}

#[tauri::command]
pub fn go_to_page(
    state: State<'_, DocumentCoreState>,
    search_state: State<'_, SearchCoreState>,
    session_id: String,
    page_index: usize,
) -> Result<SessionStateResponse, String> {
    let response = state
        .store
        .go_to_page(session_id.clone(), page_index)
        .map_err(map_error)?;
    if let Ok(mut engine) = search_state.lock() {
        engine.set_active_page(session_id, page_index);
    }
    Ok(response)
}

#[tauri::command]
pub fn set_zoom(
    state: State<'_, DocumentCoreState>,
    session_id: String,
    zoom: f32,
) -> Result<SessionStateResponse, String> {
    state.store.set_zoom(session_id, zoom).map_err(map_error)
}

#[tauri::command]
pub fn get_session_state(
    state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<SessionStateResponse, String> {
    state.store.session_state(session_id).map_err(map_error)
}

#[tauri::command]
pub fn close_session(
    state: State<'_, DocumentCoreState>,
    editing_state: State<'_, EditingCoreState>,
    session_id: String,
) -> Result<(), String> {
    state.store.close_document(&session_id).map_err(map_error)?;
    if let Ok(mut engine) = editing_state.lock() {
        engine.clear_session(&session_id);
    }
    Ok(())
}

/// Extract the full text of every page in one IPC round-trip.
/// Returns a `Vec<String>` where index `i` contains the text of page `i`.
/// Prefer this over N `doc_extract_text` calls when building a search index.
#[tauri::command]
pub fn doc_extract_all_text(
    state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<Vec<String>, String> {
    state
        .store
        .extract_all_pages_text(session_id)
        .map_err(map_error)
}

/// List all AcroForm fields in the document.
/// Returns a `Vec<FormField>` with name, type, value, page_index, and rect for each field.
/// Returns an empty list for documents that have no AcroForm.
#[tauri::command]
pub fn doc_list_form_fields(
    state: State<'_, DocumentCoreState>,
    session_id: String,
) -> Result<Vec<FormField>, String> {
    state.store.list_form_fields(session_id).map_err(map_error)
}

// ── Phase 23A: Form-field creation, deletion, property updates ──────────────

use super::forms::{
    CreateFormFieldRequest, DeleteFormFieldRequest, FormFieldOperationResult,
    UpdateFormFieldPropertiesRequest,
};

#[tauri::command]
pub fn pdf_create_form_field(
    state: State<'_, DocumentCoreState>,
    request: CreateFormFieldRequest,
) -> Result<FormFieldOperationResult, String> {
    state.store.create_form_field(request).map_err(map_error)
}

#[tauri::command]
pub fn pdf_delete_form_field(
    state: State<'_, DocumentCoreState>,
    request: DeleteFormFieldRequest,
) -> Result<FormFieldOperationResult, String> {
    state.store.delete_form_field(request).map_err(map_error)
}

#[tauri::command]
pub fn pdf_update_form_field_properties(
    state: State<'_, DocumentCoreState>,
    request: UpdateFormFieldPropertiesRequest,
) -> Result<FormFieldOperationResult, String> {
    state
        .store
        .update_form_field_properties(request)
        .map_err(map_error)
}

// ── Phase 23E + 24B/E: Document compare / redline ─────────────────────────

use super::compare::{
    run_text_compare_with_sources, run_visual_page_diff, CompareDocumentInfo,
    CompareDocumentsRequest, CompareMode, CompareResult, CompareState, PageTextSource,
    PageTextWithSource, RenderedPage,
};

/// Builds a `PageTextWithSource` for each page of an open session, using
/// native text when present and falling back to cached OCR when available.
///
/// Phase 25A: also populates `lines` from per-line bbox extraction (when
/// native text is present) so the compare diff can attach pixel-accurate
/// bboxes to each change row. OCR-only pages get one synthetic line per
/// text line (no bbox).
///
/// Phase 25A: when `auto_ocr_scanned` is true the function will *also*
/// run OCR on scanned pages whose cache is empty before consulting it,
/// up to `auto_ocr_max_pages`. Any failures (worker missing, render
/// error, etc.) accumulate into `warnings` rather than aborting the
/// compare; the affected pages then degrade to `PageTextSource::None`.
fn build_pages_with_source_v2(
    doc_state: &State<'_, DocumentCoreState>,
    ocr_state: &State<'_, super::ocr::OcrState>,
    session_id: &str,
    include_ocr: bool,
    auto_ocr_scanned: bool,
    auto_ocr_max_pages: usize,
    warnings: &mut Vec<String>,
) -> Result<Vec<PageTextWithSource>, String> {
    let native = doc_state
        .store
        .extract_all_pages_text(session_id.to_string())
        .map_err(map_error)?;

    let line_bboxes = doc_state
        .store
        .extract_all_pages_with_line_bboxes(session_id.to_string())
        .map_err(map_error)?;

    // Identify pages that are scanned (no native text). If auto-OCR is on,
    // run OCR for those pages now (best-effort).
    if include_ocr && auto_ocr_scanned {
        let mut ocr_runs = 0usize;
        for (idx, txt) in native.iter().enumerate() {
            if !txt.trim().is_empty() {
                continue;
            }
            // Already cached?
            let cached = {
                let engine = ocr_state
                    .engine
                    .lock()
                    .map_err(|_| "OCR lock poisoned".to_string())?;
                engine.cache.has_result(session_id, idx)
            };
            if cached {
                continue;
            }
            if ocr_runs >= auto_ocr_max_pages {
                warnings.push(format!(
                    "Auto-OCR stopped after {auto_ocr_max_pages} page(s); remaining scanned pages were skipped. Increase the limit or run OCR manually."
                ));
                break;
            }
            match run_ocr_for_page_inline(doc_state, ocr_state, session_id, idx) {
                Ok(()) => {
                    ocr_runs += 1;
                }
                Err(e) => {
                    warnings.push(format!("Auto-OCR for page {} failed: {e}", idx + 1));
                    // Stop trying further pages if OCR worker is unavailable.
                    if e.to_lowercase().contains("ocr") && e.to_lowercase().contains("unavailable")
                    {
                        break;
                    }
                }
            }
        }
    }

    let ocr_lookup: std::collections::HashMap<usize, String> = if include_ocr {
        let engine = ocr_state
            .engine
            .lock()
            .map_err(|_| "OCR lock poisoned".to_string())?;
        engine
            .cache
            .all_texts_for_session(session_id)
            .into_iter()
            .collect()
    } else {
        Default::default()
    };

    let mut out = Vec::with_capacity(native.len());
    for (idx, native_text) in native.iter().enumerate() {
        let has_native = !native_text.trim().is_empty();
        let ocr_text = ocr_lookup
            .get(&idx)
            .filter(|s| !s.trim().is_empty())
            .cloned();
        let (text, source) = match (has_native, ocr_text) {
            (true, _) => (native_text.clone(), PageTextSource::NativeText),
            (false, Some(ocr)) => (ocr, PageTextSource::OcrText),
            (false, None) => (String::new(), PageTextSource::None),
        };

        // Lines: prefer per-line bboxes from the engine when native text is
        // present (matches the extraction source). For OCR-only pages we
        // synthesize bbox-less lines so the diff still emits change rows.
        let lines = match source {
            PageTextSource::NativeText => {
                let page_lines = line_bboxes
                    .get(idx)
                    .map(|pl| pl.lines.clone())
                    .unwrap_or_default();
                if page_lines.is_empty() {
                    // Fall back to splitting native text — shouldn't happen
                    // for valid PDFs but keeps the path robust.
                    Some(
                        text.lines()
                            .map(|s| super::types::LineBbox {
                                text: s.to_string(),
                                bbox: [0.0; 4],
                            })
                            .collect(),
                    )
                } else {
                    Some(page_lines)
                }
            }
            PageTextSource::OcrText => Some(
                text.lines()
                    .map(|s| super::types::LineBbox {
                        text: s.to_string(),
                        bbox: [0.0; 4],
                    })
                    .collect(),
            ),
            _ => None,
        };

        out.push(PageTextWithSource {
            text,
            source,
            lines,
        });
    }
    Ok(out)
}

/// Internal helper: render `page_index` of `session_id` to a PPM image and
/// invoke the OCR worker, caching the result.
fn run_ocr_for_page_inline(
    doc_state: &State<'_, DocumentCoreState>,
    ocr_state: &State<'_, super::ocr::OcrState>,
    session_id: &str,
    page_index: usize,
) -> Result<(), String> {
    let dpi = 200;
    let zoom = dpi as f32 / 72.0;
    let render_request = super::types::RenderRequest {
        session_id: session_id.to_string(),
        page_index,
        zoom,
        viewport: super::types::BBox {
            x: 0.0,
            y: 0.0,
            width: 2000.0,
            height: 2800.0,
        },
        device_pixel_ratio: 1.0,
    };
    let render_result = doc_state
        .store
        .render_page(render_request)
        .map_err(|e| e.to_string())?;
    let temp_dir = std::env::temp_dir();
    let image_path = temp_dir.join(format!("r2h_autoocr_{session_id}_{page_index}.png"));
    save_rgba_as_png(
        &render_result.pixels_rgba,
        render_result.width_px,
        render_result.height_px,
        &image_path,
    )
    .map_err(|e| format!("Failed to save page image: {e}"))?;

    let mut engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    let _ = engine
        .run_ocr_on_image(
            session_id,
            page_index,
            &image_path,
            render_result.width_px,
            render_result.height_px,
            false,
        )
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&image_path);
    Ok(())
}

#[tauri::command]
pub fn pdf_compare_documents(
    doc_state: State<'_, DocumentCoreState>,
    ocr_state: State<'_, super::ocr::OcrState>,
    compare_state: State<'_, CompareState>,
    request: CompareDocumentsRequest,
) -> Result<CompareResult, String> {
    crate::license::assert_feature_allowed("compare")?;

    let include_ocr = request.include_ocr;
    let auto_ocr = request.auto_ocr_scanned;
    let auto_ocr_cap = request.auto_ocr_max_pages.unwrap_or(25).min(200);

    // Pre-collect auto-OCR warnings so they end up inside CompareResult.warnings.
    let mut auto_ocr_warnings: Vec<String> = Vec::new();

    let base_pages = build_pages_with_source_v2(
        &doc_state,
        &ocr_state,
        &request.base_session_id,
        include_ocr,
        auto_ocr,
        auto_ocr_cap,
        &mut auto_ocr_warnings,
    )?;
    let base_info = CompareDocumentInfo {
        session_id: Some(request.base_session_id.clone()),
        source_path: None,
        page_count: base_pages.len(),
    };

    // Resolve revised side: prefer an open session, else open by path.
    // We keep the temporary session id alive when we need it for visual
    // rendering, then close it before returning.
    let (revised_pages, revised_info, revised_session_for_render, owns_revised_session) =
        if let Some(sid) = &request.revised_session_id {
            let pages = build_pages_with_source_v2(
                &doc_state,
                &ocr_state,
                sid,
                include_ocr,
                auto_ocr,
                auto_ocr_cap,
                &mut auto_ocr_warnings,
            )?;
            let info = CompareDocumentInfo {
                session_id: Some(sid.clone()),
                source_path: None,
                page_count: pages.len(),
            };
            (pages, info, Some(sid.clone()), false)
        } else if let Some(path) = &request.revised_file_path {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                return Err("revised_file_path cannot be empty".to_string());
            }
            let p = std::path::Path::new(trimmed);
            if !p.exists() {
                return Err(format!("revised PDF not found at path: {trimmed}"));
            }
            let opened = doc_state
                .store
                .open_document(super::types::OpenDocumentRequest {
                    path: trimmed.to_string(),
                    recover_if_damaged: true,
                })
                .map_err(map_error)?;
            let session_id = opened.session_id.clone();
            let pages = build_pages_with_source_v2(
                &doc_state,
                &ocr_state,
                &session_id,
                include_ocr,
                auto_ocr,
                auto_ocr_cap,
                &mut auto_ocr_warnings,
            )?;
            let info = CompareDocumentInfo {
                session_id: None,
                source_path: Some(trimmed.to_string()),
                page_count: pages.len(),
            };
            (pages, info, Some(session_id), true)
        } else {
            return Err(
                "either revised_session_id or revised_file_path must be provided".to_string(),
            );
        };

    let mut result = run_text_compare_with_sources(
        &base_pages,
        &revised_pages,
        request.max_pages,
        base_info,
        revised_info,
        request.mode.clone(),
    );

    // Surface any auto-OCR warnings to the caller.
    result.warnings.extend(auto_ocr_warnings);

    // Phase 24B: visual diff when requested.
    let want_visual = matches!(
        request.mode,
        CompareMode::PageVisual | CompareMode::Combined
    );
    if want_visual {
        if let Some(ref revised_sid) = revised_session_for_render {
            run_visual_diff_into(
                &doc_state,
                &request.base_session_id,
                revised_sid,
                request.visual_max_pages,
                request.visual_dpi,
                &mut result,
            );
        } else {
            result.warnings.push(
                "Visual compare requires the revised PDF to be openable as a session.".to_string(),
            );
        }
    }

    if let (true, Some(sid)) = (owns_revised_session, revised_session_for_render.as_ref()) {
        let _ = doc_state.store.close_document(sid);
    }

    compare_state
        .store
        .insert(result.clone())
        .map_err(|e| format!("failed to store compare result: {e}"))?;

    Ok(result)
}

/// Render up to `max_pages` matching pages from base/revised at `dpi`, run
/// a pixel-grid diff, and append `CompareVisualChange` entries + per-page
/// `visual_regions` counts to `result`.
fn run_visual_diff_into(
    doc_state: &State<'_, DocumentCoreState>,
    base_session: &str,
    revised_session: &str,
    max_pages_req: Option<usize>,
    dpi: Option<u32>,
    result: &mut CompareResult,
) {
    let dpi = dpi.unwrap_or(72).clamp(48, 200);
    let zoom = dpi as f32 / 72.0;
    let max_pages = max_pages_req.unwrap_or(50).min(200);

    let total = result.page_results.len();
    let to_compare = total.min(max_pages);
    if to_compare < total {
        result.warnings.push(format!(
            "Visual compare ran on the first {to_compare} of {total} pages (visual_max_pages={max_pages}). Re-run with a higher limit for full coverage."
        ));
    }

    for page_idx in 0..to_compare {
        let base_rendered = render_page_for_visual(doc_state, base_session, page_idx, zoom);
        let revised_rendered = render_page_for_visual(doc_state, revised_session, page_idx, zoom);
        let (base_r, revised_r) = match (base_rendered, revised_rendered) {
            (Ok(b), Ok(r)) => (b, r),
            (Err(e), _) | (_, Err(e)) => {
                result
                    .warnings
                    .push(format!("Visual diff skipped page {}: {e}", page_idx + 1));
                continue;
            }
        };

        // Page-count mismatch handling: render only what exists on both
        // sides. (Rendering above already returns an error when the page
        // index is out of range.)
        let (changes, count) = run_visual_page_diff(&base_r, &revised_r, 32, 8);
        if let Some(pr) = result.page_results.get_mut(page_idx) {
            pr.visual_regions = count;
        }
        result.visual_changes.extend(changes);
        if count > 0 {
            // Make sure pages_with_changes accounts for visual-only changes.
            if let Some(pr) = result.page_results.get(page_idx) {
                if pr.added + pr.removed + pr.modified == 0 {
                    result.summary.pages_with_changes += 1;
                }
            }
        }
    }

    if !result.visual_changes.is_empty() {
        result.summary.identical = false;
    }
}

fn render_page_for_visual(
    doc_state: &State<'_, DocumentCoreState>,
    session_id: &str,
    page_index: usize,
    zoom: f32,
) -> Result<RenderedPage, String> {
    let req = super::types::RenderRequest {
        session_id: session_id.to_string(),
        page_index,
        zoom,
        viewport: super::types::BBox {
            x: 0.0,
            y: 0.0,
            width: 2000.0,
            height: 2800.0,
        },
        device_pixel_ratio: 1.0,
    };
    let resp = doc_state
        .store
        .render_page(req)
        .map_err(|e| e.to_string())?;
    // Derive page point dimensions from the render zoom + pixel dimensions.
    let page_width_pts = resp.width_px as f32 / zoom;
    let page_height_pts = resp.height_px as f32 / zoom;
    Ok(RenderedPage {
        page_index,
        width_px: resp.width_px,
        height_px: resp.height_px,
        page_width_pts,
        page_height_pts,
        pixels_rgba: resp.pixels_rgba,
    })
}

#[tauri::command]
pub fn pdf_get_compare_result(
    compare_state: State<'_, CompareState>,
    compare_id: String,
) -> Result<Option<CompareResult>, String> {
    compare_state.store.get(&compare_id)
}

#[tauri::command]
pub fn pdf_clear_compare_result(
    compare_state: State<'_, CompareState>,
    compare_id: String,
) -> Result<bool, String> {
    compare_state.store.remove(&compare_id)
}

// ── Phase 24C: Compare HTML redline export ─────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExportCompareReportRequest {
    pub compare_id: String,
    pub output_path: String,
    pub overwrite_existing: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportCompareReportResult {
    pub compare_id: String,
    pub output_path: String,
    pub bytes_written: usize,
    pub text_changes_count: usize,
    pub visual_changes_count: usize,
    pub pages_compared: usize,
    pub warnings_count: usize,
}

#[tauri::command]
pub fn report_export_compare_review(
    compare_state: State<'_, CompareState>,
    request: ExportCompareReportRequest,
) -> Result<ExportCompareReportResult, String> {
    let result = compare_state.store.get(&request.compare_id)?;
    let r = result.ok_or_else(|| format!("compare result not found: {}", request.compare_id))?;

    let trimmed = request.output_path.trim();
    if trimmed.is_empty() {
        return Err("output_path cannot be empty".to_string());
    }
    let target = std::path::Path::new(trimmed);
    let overwrite = request.overwrite_existing.unwrap_or(true);
    if target.exists() && !overwrite {
        return Err(format!("output file already exists: {trimmed}"));
    }
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create parent dir: {e}"))?;
        }
    }

    let html = super::compare::build_redline_html_report(&r);
    let bytes = html.as_bytes();
    std::fs::write(target, bytes).map_err(|e| format!("failed to write redline report: {e}"))?;

    Ok(ExportCompareReportResult {
        compare_id: r.compare_id.clone(),
        output_path: trimmed.to_string(),
        bytes_written: bytes.len(),
        text_changes_count: r.text_changes.len(),
        visual_changes_count: r.visual_changes.len(),
        pages_compared: r.summary.pages_compared,
        warnings_count: r.warnings.len(),
    })
}

// ── Phase 24D: AI review of a compare result ───────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AiReviewCompareRequest {
    pub compare_id: String,
    pub session_id: Option<String>,
}

#[tauri::command]
pub fn ai_review_compare_result(
    compare_state: State<'_, CompareState>,
    local_ai_state: State<'_, crate::ai_core::LocalAiState>,
    request: AiReviewCompareRequest,
) -> Result<crate::ai_core::review::DocumentReviewResult, String> {
    let cmp = compare_state
        .store
        .get(&request.compare_id)?
        .ok_or_else(|| format!("compare result not found: {}", request.compare_id))?;

    let session_id = request
        .session_id
        .or_else(|| cmp.base_document.session_id.clone())
        .unwrap_or_default();

    let base_label = label_for(&cmp.base_document);
    let revised_label = label_for(&cmp.revised_document);
    let summary_line = format!(
        "+{} / -{} / ~{} across {} page(s); {} visual region(s).",
        cmp.summary.lines_added,
        cmp.summary.lines_removed,
        cmp.summary.lines_modified,
        cmp.summary.pages_with_changes,
        cmp.visual_changes.len(),
    );

    let text_changes: Vec<(String, usize, Option<String>, Option<String>)> = cmp
        .text_changes
        .iter()
        .map(|c| {
            let ct = match c.change_type {
                super::compare::ChangeType::Added => "added",
                super::compare::ChangeType::Removed => "removed",
                super::compare::ChangeType::Modified => "modified",
                super::compare::ChangeType::VisualModified => "visual",
            };
            (
                ct.to_string(),
                c.page_index + 1,
                c.old_text.clone(),
                c.new_text.clone(),
            )
        })
        .collect();

    let runtime = local_ai_state
        .runtime
        .lock()
        .map_err(|_| "local AI lock poisoned".to_string())?;

    crate::ai_core::review::run_compare_review(
        &session_id,
        &base_label,
        &revised_label,
        &summary_line,
        &text_changes,
        cmp.visual_changes.len(),
        &cmp.warnings,
        &runtime,
    )
}

fn label_for(d: &super::compare::CompareDocumentInfo) -> String {
    if let Some(p) = &d.source_path {
        return p.clone();
    }
    if let Some(s) = &d.session_id {
        return format!("session:{s}");
    }
    "unknown".to_string()
}

/// Silence the unused-import lint when compare/forms helpers ship without
/// being used elsewhere in this module.
#[allow(dead_code)]
fn _ipc_phase23_marker() -> CompareMode {
    CompareMode::TextOnly
}

// ── OCR Commands ────────────────────────────────────────────────────────────

use super::ocr::{
    operation_temp_path, OcrAvailability, OcrPageResult, OcrRequest, OcrRunContext, OcrState,
};

/// Check if the OCR engine is available, preserving the exact blocked reason.
#[tauri::command]
pub fn ocr_check_availability(ocr_state: State<'_, OcrState>) -> Result<OcrAvailability, String> {
    let engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    Ok(engine.check_availability())
}

/// Get the same structured OCR status used by the availability command.
#[tauri::command]
pub fn ocr_get_status(ocr_state: State<'_, OcrState>) -> Result<OcrAvailability, String> {
    let engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    Ok(engine.check_availability())
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OcrCancelRequest {
    pub operation_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrCancelResponse {
    pub operation_id: String,
    pub cancelled: bool,
}

#[tauri::command]
pub fn ocr_cancel(
    ocr_state: State<'_, OcrState>,
    request: OcrCancelRequest,
) -> Result<OcrCancelResponse, String> {
    if request.operation_id.trim().is_empty() {
        return Err("OCR cancellation requires an operation ID.".to_string());
    }
    let mut cancellations = ocr_state
        .cancellations
        .lock()
        .map_err(|_| "OCR cancellation lock poisoned".to_string())?;
    let inserted = cancellations.insert(request.operation_id.clone());
    Ok(OcrCancelResponse {
        operation_id: request.operation_id,
        cancelled: inserted,
    })
}

/// Check if a page has cached OCR results.
#[tauri::command]
pub fn ocr_has_result(
    ocr_state: State<'_, OcrState>,
    session_id: String,
    page_index: usize,
) -> Result<bool, String> {
    let engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    Ok(engine.cache.has_result(&session_id, page_index))
}

/// Get cached OCR text for a page (returns None if not yet OCR'd).
#[tauri::command]
pub fn ocr_get_page_text(
    ocr_state: State<'_, OcrState>,
    session_id: String,
    page_index: usize,
) -> Result<Option<String>, String> {
    let engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    Ok(engine.cache.get_text(&session_id, page_index))
}

/// Run OCR on a single page. Renders the page to a temp PNG, invokes the
/// Python worker, and caches the result.
#[tauri::command]
pub fn ocr_run_page(
    doc_state: State<'_, DocumentCoreState>,
    ocr_state: State<'_, OcrState>,
    request: OcrRequest,
) -> Result<OcrPageResult, String> {
    crate::license::assert_feature_allowed("ocr")?;
    validate_ocr_request(&request)?;

    let (document_id, page_width_points, page_height_points, rotation_degrees) = {
        let arc = doc_state
            .store
            .get_session_arc_pub(&request.session_id)
            .map_err(|e| e.to_string())?;
        let session = arc
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        validate_ocr_page_index(request.page_index, session.document.pages.len())?;
        let page = &session.document.pages[request.page_index];
        let rotation = page_rotation_from_pdf_bytes(&session.document.bytes, request.page_index)?;
        if !page.width_points.is_finite()
            || !page.height_points.is_finite()
            || page.width_points <= 0.0
            || page.height_points <= 0.0
        {
            return Err(
                "[OCR_INVALID_PAGE] The active PDF page has invalid dimensions.".to_string(),
            );
        }
        (
            session.document.document_hash.clone(),
            page.width_points,
            page.height_points,
            rotation,
        )
    };

    // Render the page to pixels.
    let render_request = build_ocr_render_request(&request);

    let render_result = doc_state
        .store
        .render_page(render_request)
        .map_err(|e| e.to_string())?;

    // Save pixels to an operation-isolated temp image. The guard removes it
    // on every exit path, including render-input and worker failures.
    let image_path = operation_temp_path(&request.operation_id);
    let _temp_image = TempImageGuard::new(image_path.clone());

    save_rgba_as_png(
        &render_result.pixels_rgba,
        render_result.width_px,
        render_result.height_px,
        &image_path,
    )
    .map_err(|e| format!("Failed to save page image: {e}"))?;

    // Run OCR with the live session/page context and cancellation registry.
    let timeout_secs = request.timeout_secs.unwrap_or(120).max(1);
    let context = OcrRunContext {
        operation_id: request.operation_id.clone(),
        document_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        page_width_points,
        page_height_points,
        rotation_degrees,
        language: request.language.clone(),
        model_id: request.model_id.clone(),
        timeout_secs,
        cancellation_id: request.cancellation_id.clone(),
    };
    let cancellations = ocr_state.cancellations.clone();
    let mut engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    let result = engine
        .run_ocr_on_image_with_context(
            &image_path,
            render_result.width_px,
            render_result.height_px,
            request.force,
            context,
            Some(&cancellations),
        )
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Get all cached OCR texts for a session (for search index integration).
#[tauri::command]
pub fn ocr_get_all_texts(
    ocr_state: State<'_, OcrState>,
    session_id: String,
) -> Result<Vec<(usize, String)>, String> {
    let engine = ocr_state
        .engine
        .lock()
        .map_err(|_| "OCR lock poisoned".to_string())?;
    Ok(engine.cache.all_texts_for_session(&session_id))
}

// ── Phase 25D: OCR editable overlay + searchable layer ──────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrOverlaySpec {
    pub id: String,
    pub page_index: usize,
    pub text: String,
    /// Bbox in PDF point space [x0, y0, x1, y1] (origin bottom-left).
    pub bbox: [f32; 4],
    pub confidence: Option<f32>,
    pub block_type: String,
    pub block_id: String,
    /// Phase 26E: the worker's original (image-pixel) bbox so the UI can
    /// audit conversions and report when something looks wrong.
    pub original_bbox: [f32; 4],
    /// Phase 26E: where the bbox originally lived (`"image_px"` here for
    /// the PaddleOCR worker). Future engines may emit `"pdf_points"`.
    pub coordinate_space: String,
    /// Phase 26E: scale factor used to convert image pixels → PDF points
    /// per axis. Both will be ~0.36 at 200 DPI (72/200).
    pub conversion_scale: [f32; 2],
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrOverlayResult {
    pub document_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub status: String,
    pub overlays: Vec<OcrOverlaySpec>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateOcrOverlaysRequest {
    pub session_id: String,
    pub page_index: usize,
    /// When set, drop overlays below this confidence (0.0–1.0). Default 0.0.
    pub min_confidence: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrTextLayerStatus {
    pub session_id: String,
    pub page_index: usize,
    pub status: String, // "not_implemented" | "ok"
    pub overlays_available: bool,
    pub note: String,
    pub warnings: Vec<String>,
}

fn validate_ocr_request(request: &OcrRequest) -> Result<(), String> {
    if request.operation_id.trim().is_empty() {
        return Err("[OCR_INVALID_REQUEST] OCR operation_id is required.".to_string());
    }
    if !request.session_id.starts_with("doc-session-") {
        return Err(
            "[OCR_INVALID_SESSION] OCR requires the live native PDF session ID.".to_string(),
        );
    }
    if request.model_id != "PaddleOCR-VL" || request.output_format_version != 1 {
        return Err(
            "[OCR_UNSUPPORTED_REQUEST] OCR model or result schema is unsupported.".to_string(),
        );
    }
    if request.dpi == Some(0) {
        return Err("[OCR_INVALID_REQUEST] OCR dpi must be greater than zero.".to_string());
    }
    Ok(())
}

fn validate_ocr_page_index(page_index: usize, page_count: usize) -> Result<(), String> {
    if page_index >= page_count {
        return Err(format!(
            "[PAGE_OUT_OF_RANGE] page {} is out of range",
            page_index + 1
        ));
    }
    Ok(())
}

fn build_ocr_render_request(request: &OcrRequest) -> super::types::RenderRequest {
    let dpi = request.dpi.unwrap_or(200);
    super::types::RenderRequest {
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        zoom: dpi as f32 / 72.0,
        viewport: super::types::BBox {
            x: 0.0,
            y: 0.0,
            width: 2000.0,
            height: 2800.0,
        },
        device_pixel_ratio: 1.0,
    }
}

/// Phase 25D: read OCR blocks from the cache for `page_index` and return a
/// list of overlay specs that the frontend can convert into editable text
/// overlays. OCR blocks must already be cached — run `ocr_run_page` (or the
/// auto-OCR compare path) first.
#[tauri::command]
pub fn pdf_create_ocr_editable_overlays(
    doc_state: State<'_, DocumentCoreState>,
    ocr_state: State<'_, super::ocr::OcrState>,
    request: CreateOcrOverlaysRequest,
) -> Result<OcrOverlayResult, String> {
    let min_conf = request.min_confidence.unwrap_or(0.0).clamp(0.0, 1.0);
    let mut warnings: Vec<String> = Vec::new();

    // Page dimensions for axis scaling and Y-axis flip.
    let (page_width, page_height) = {
        let arc = doc_state
            .store
            .get_session_arc_pub(&request.session_id)
            .map_err(|e| e.to_string())?;
        let session = arc
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        let p = session
            .document
            .pages
            .get(request.page_index)
            .ok_or_else(|| format!("page {} out of range", request.page_index))?;
        (p.width_points, p.height_points)
    };

    let result = {
        let engine = ocr_state
            .engine
            .lock()
            .map_err(|_| "OCR lock poisoned".to_string())?;
        engine
            .cache
            .get(&request.session_id, request.page_index)
            .cloned()
    };

    let result = match result {
        Some(r) => r,
        None => {
            return Err(format!(
                "No OCR result cached for page {}. Run OCR first.",
                request.page_index + 1
            ));
        }
    };

    // Phase 26E: PaddleOCR worker emits bbox in IMAGE PIXEL space
    // (top-left origin) relative to the rendered image. We convert with:
    //     scale_x = page_width_pts / image_width_px
    //     scale_y = page_height_pts / image_height_px
    //     pdf_y0 = page_h - (img_y + img_h) * scale_y   (Y flip)
    // Results without render dimensions cannot be converted safely. Keep the
    // legacy pure converter behavior for old unit tests, but fail closed in
    // this production acceptance path.
    let conversion_scale: [f32; 2] = if result.image_width_px > 0 && result.image_height_px > 0 {
        [
            page_width / result.image_width_px as f32,
            page_height / result.image_height_px as f32,
        ]
    } else {
        return Err(format!(
            "[OCR_INVALID_RESULT] OCR result for page {} has no recorded render dimensions; re-run OCR before accepting overlays.",
            request.page_index + 1
        ));
    };
    let coordinate_space = "image_px".to_string();

    if matches!(result.status, super::ocr::OcrResultStatus::NoTextDetected) {
        return Ok(OcrOverlayResult {
            document_id: result.document_id.clone(),
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            status: "no_text_detected".to_string(),
            overlays: vec![],
            warnings: vec![format!(
                "No text was detected on page {}.",
                request.page_index + 1
            )],
        });
    }

    let mut overlays = Vec::with_capacity(result.blocks.len());
    let mut dropped_low_conf = 0usize;
    let mut clamped = 0usize;
    for (i, block) in result.blocks.iter().enumerate() {
        if block
            .confidence
            .map(|confidence| confidence < min_conf)
            .unwrap_or(min_conf > 0.0)
        {
            dropped_low_conf += 1;
            continue;
        }
        let original_bbox = [
            block.bbox.x,
            block.bbox.y,
            block.bbox.x + block.bbox.width,
            block.bbox.y + block.bbox.height,
        ];

        let pdf_x0 = block.bbox.x * conversion_scale[0];
        let pdf_x1 = (block.bbox.x + block.bbox.width) * conversion_scale[0];
        let pdf_y_top = block.bbox.y * conversion_scale[1];
        let pdf_y_bot = (block.bbox.y + block.bbox.height) * conversion_scale[1];
        let pdf_y0 = page_height - pdf_y_bot;
        let pdf_y1 = page_height - pdf_y_top;

        // Clamp + validate; reject degenerate bboxes with a warning so
        // the UI can flag them rather than silently dropping.
        let cx0 = pdf_x0.clamp(0.0, page_width);
        let cx1 = pdf_x1.clamp(0.0, page_width);
        let cy0 = pdf_y0.clamp(0.0, page_height);
        let cy1 = pdf_y1.clamp(0.0, page_height);
        if (cx0 - pdf_x0).abs() > 0.1
            || (cx1 - pdf_x1).abs() > 0.1
            || (cy0 - pdf_y0).abs() > 0.1
            || (cy1 - pdf_y1).abs() > 0.1
        {
            clamped += 1;
        }
        if cx1 <= cx0 || cy1 <= cy0 {
            warnings.push(format!(
                "OCR block {i} on page {} collapsed to zero area after scale/clamp; skipped.",
                request.page_index + 1
            ));
            continue;
        }

        overlays.push(OcrOverlaySpec {
            id: format!(
                "ocr-{}-p{}-{}",
                stable_id_component(&result.document_id),
                request.page_index,
                stable_id_component(&block.id)
            ),
            page_index: request.page_index,
            text: block.text.clone(),
            bbox: [cx0, cy0, cx1, cy1],
            confidence: block.confidence,
            block_type: block.block_type.clone(),
            block_id: block.id.clone(),
            original_bbox,
            coordinate_space: coordinate_space.clone(),
            conversion_scale,
        });
    }

    if clamped > 0 {
        warnings.push(format!(
            "{clamped} OCR block(s) on page {} extended past page bounds and were clamped.",
            request.page_index + 1
        ));
    }

    if overlays.is_empty() {
        warnings.push(format!(
            "OCR result for page {} contains no usable blocks (all blocks below confidence threshold or empty).",
            request.page_index + 1,
        ));
    }
    if dropped_low_conf > 0 {
        warnings.push(format!(
            "Dropped {dropped_low_conf} OCR block(s) below confidence {min_conf:.2}."
        ));
    }
    warnings.push(
        "Editable OCR overlay — not native original text. Bbox was validated in PDF coordinates."
            .to_string(),
    );

    Ok(OcrOverlayResult {
        document_id: result.document_id.clone(),
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        status: "completed".to_string(),
        overlays,
        warnings,
    })
}

/// Phase 25D / 27F: searchable PDF text layer creation. True content-stream
/// injection of invisible text glyphs requires either:
///   1. embedding a font into the document and emitting BT…ET text-show
///      operators with PDF text rendering mode 3 ("invisible"); or
///   2. using mupdf's PDF builder APIs to add a real text object.
///
/// The mupdf-rs 0.6 bindings do not expose either path safely (no
/// `add_font` / no rendering-mode parameter on text show), so we'd have
/// to hand-author content streams with a hardcoded built-in font and
/// risk corrupting downstream xref tables. Faking the layer (e.g. drawing
/// visible text with white fill) would falsely advertise searchability,
/// so this command honestly reports `not_implemented` and the UI routes
/// the user to `pdf_create_ocr_editable_overlays`, which produces visual
/// editable PDF annotations. Those annotations are not an embedded searchable
/// text layer.
#[tauri::command]
pub fn pdf_create_ocr_text_layer(
    ocr_state: State<'_, super::ocr::OcrState>,
    request: CreateOcrOverlaysRequest,
) -> Result<OcrTextLayerStatus, String> {
    let overlays_available = {
        let engine = ocr_state
            .engine
            .lock()
            .map_err(|_| "OCR lock poisoned".to_string())?;
        engine
            .cache
            .has_result(&request.session_id, request.page_index)
    };

    Ok(OcrTextLayerStatus {
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        status: "not_implemented".to_string(),
        overlays_available,
        note: "True invisible PDF text-layer injection is NOT implemented in this build. \
               The mupdf-rs binding does not expose safe font-embedding and \
               text-rendering-mode-3 emission, and faking a searchable layer would be \
               dishonest. Visual editable OCR annotations remain available separately \
               when the worker returns validated geometry; they are not embedded searchable PDF text."
            .to_string(),
        warnings: vec![
            "OCR text-layer command intentionally returns not_implemented; no PDF bytes were modified."
                .to_string(),
            "Visual OCR annotations are exportable overlays, not embedded searchable PDF text.".to_string(),
        ],
    })
}

/// Save raw RGBA pixel data as a PNG file.
/// Uses a minimal PNG encoder (uncompressed) to avoid adding image crate deps.
fn save_rgba_as_png(
    pixels: &[u8],
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> Result<(), String> {
    // Convert RGBA to RGB for simpler PNG encoding, or write as raw BMP.
    // For the OCR worker, we'll write a simple PPM (Netpbm) format which
    // PIL/Pillow can read, avoiding the need for a PNG encoder crate.
    let ppm_path = path.with_extension("ppm");
    let mut file = std::fs::File::create(&ppm_path)
        .map_err(|e| format!("Failed to create image file: {e}"))?;

    // PPM header
    use std::io::Write;
    write!(file, "P6\n{} {}\n255\n", width, height)
        .map_err(|e| format!("Failed to write PPM header: {e}"))?;

    // Write RGB pixels (skip alpha channel).
    for chunk in pixels.chunks(4) {
        if chunk.len() >= 3 {
            file.write_all(&chunk[..3])
                .map_err(|e| format!("Failed to write pixel data: {e}"))?;
        }
    }

    // Rename to the expected path (worker reads .ppm just fine via PIL).
    if path != &ppm_path {
        std::fs::rename(&ppm_path, path).map_err(|e| format!("Failed to rename image: {e}"))?;
    }

    Ok(())
}

fn page_rotation_from_pdf_bytes(bytes: &[u8], page_index: usize) -> Result<i32, String> {
    let pdf = mupdf::pdf::PdfDocument::from_bytes(bytes)
        .map_err(|e| format!("[OCR_PDF_OPEN_FAILED] Failed to inspect the active PDF page: {e}"))?;
    let count = pdf
        .page_count()
        .map_err(|e| format!("[OCR_PDF_OPEN_FAILED] Failed to read PDF page count: {e}"))?;
    let page_number = i32::try_from(page_index)
        .map_err(|_| "[OCR_INVALID_PAGE] Page index is too large.".to_string())?;
    if page_number < 0 || page_number >= count {
        return Err(format!(
            "[PAGE_OUT_OF_RANGE] page {} is out of range",
            page_index + 1
        ));
    }
    let page = pdf
        .load_page(page_number)
        .map_err(|e| format!("[OCR_PDF_PAGE_FAILED] Failed to inspect the active PDF page: {e}"))?;
    let pdf_page = mupdf::pdf::PdfPage::try_from(page)
        .map_err(|e| format!("[OCR_PDF_PAGE_FAILED] Failed to inspect page rotation: {e}"))?;
    let rotation = pdf_page.rotation().unwrap_or(0);
    Ok(((rotation % 360) + 360) % 360)
}

struct TempImageGuard(std::path::PathBuf);

impl TempImageGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }
}

impl Drop for TempImageGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn stable_id_component(value: &str) -> String {
    let component: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(48)
        .collect();
    if component.is_empty() {
        "unknown".to_string()
    } else {
        component
    }
}

#[cfg(test)]
mod ocr_ipc_tests {
    use super::*;

    fn request() -> OcrRequest {
        OcrRequest {
            operation_id: "ocr-op-test".to_string(),
            session_id: "doc-session-test".to_string(),
            page_index: 3,
            force: false,
            dpi: Some(200),
            language: "auto".to_string(),
            model_id: "PaddleOCR-VL".to_string(),
            output_format_version: 1,
            timeout_secs: Some(120),
            cancellation_id: Some("ocr-op-test".to_string()),
        }
    }

    #[test]
    fn stale_or_renderer_session_ids_are_rejected_before_rendering() {
        let mut request = request();
        request.session_id = "renderer-tab-3".to_string();
        let error = validate_ocr_request(&request).unwrap_err();
        assert!(error.contains("OCR_INVALID_SESSION"));
    }

    #[test]
    fn valid_ocr_request_builds_a_native_page_render_request() {
        let request = request();
        validate_ocr_request(&request).unwrap();
        let render = build_ocr_render_request(&request);
        assert_eq!(render.session_id, "doc-session-test");
        assert_eq!(render.page_index, 3);
        assert!((render.zoom - (200.0 / 72.0)).abs() < f32::EPSILON);
        assert_eq!(render.viewport.width, 2000.0);
        assert_eq!(render.viewport.height, 2800.0);
    }

    #[test]
    fn invalid_page_index_is_rejected_before_native_rendering() {
        let error = validate_ocr_page_index(4, 4).unwrap_err();
        assert!(error.contains("PAGE_OUT_OF_RANGE"));
    }
}
