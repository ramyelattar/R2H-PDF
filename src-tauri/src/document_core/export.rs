//! PDF export: commit overlay objects as annotations, apply redactions,
//! flatten annotations, and write the final PDF atomically.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::session::DocumentCoreState;
use crate::annotation_core::engine::AnnotationEngine;
use crate::annotation_core::types::{AnnotationColor, AnnotationType, CreateAnnotationRequest};
use crate::editing_core::types::{EditOperation, EditOperationType, RedactPayload};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};

static ACTIVE_EXPORT_PATHS: Lazy<Mutex<HashSet<PathBuf>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

struct ExportPathGuard(PathBuf);

impl Drop for ExportPathGuard {
    fn drop(&mut self) {
        if let Ok(mut paths) = ACTIVE_EXPORT_PATHS.lock() {
            paths.remove(&self.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub session_id: String,
    pub output_path: String,
    pub commit_annotations: bool,
    pub flatten_annotations: bool,
    pub apply_redactions: bool,
    pub redaction_mode: String, // "export_copy" | "apply_to_current_document"
    pub overwrite_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayObject {
    pub id: String,
    pub object_type: String,
    pub page_index: usize,
    pub rect: [f32; 4],
    pub text: Option<String>,
    pub color: Option<String>,
    pub font_size: Option<f32>,
    pub author: Option<String>,
    pub stamp_text: Option<String>,
    pub stamp_type: Option<String>,
    pub opacity: Option<f32>,
    pub status: Option<String>,
    pub reason: Option<String>,
    pub replacement_text: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<f32>,
    pub fill_color: Option<String>,
    /// Phase 24G: optional provenance tag — set by the frontend when this
    /// overlay was created from a compare result, an AI review action, or
    /// a visual diff bbox. Used purely for export-summary counting.
    /// Known values: "compare", "compare_visual", "ai_review", "signature", "ocr".
    #[serde(default)]
    pub source: Option<String>,
    /// Phase 26A: when an overlay represents a visual signature image
    /// (`source = "signature"`), this carries the base64 data URL of the
    /// signature bitmap. The export pipeline decodes it and embeds the
    /// image as a real PDF image XObject + content-stream draw op so it
    /// appears externally in Adobe / Edge / Foxit.
    #[serde(default)]
    pub image_data_url: Option<String>,
    /// Phase 27A: when true, the signature image is letterboxed inside
    /// `rect` so its aspect ratio is preserved. When false (or null), the
    /// image is stretched to fill the rect (legacy behaviour).
    #[serde(default)]
    pub preserve_aspect: Option<bool>,
    /// Phase 27A: optional caller-supplied image dimensions; used only as
    /// a fallback when the backend cannot decode them. In pixels.
    #[serde(default)]
    pub image_natural_width: Option<u32>,
    #[serde(default)]
    pub image_natural_height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub options: ExportOptions,
    pub overlay_objects: Vec<OverlayObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub output_path: String,
    pub output_file_size_bytes: usize,
    pub output_sha256: String,
    pub export_type: String,
    pub independently_validated: bool,
    pub page_count: usize,
    pub overlay_objects_received_count: usize,
    pub annotations_committed_count: usize,
    pub redactions_applied_count: usize,
    pub redaction_mode: String,
    pub source_document_mutated: bool,
    pub true_flattening_supported: bool,
    pub flattened: bool,
    pub warnings: Vec<String>,
    pub created_at: u128,
    pub duration_ms: u128,
    /// Phase 24G: provenance-tagged subtotals derived from `OverlayObject.source`.
    #[serde(default)]
    pub compare_annotations_count: usize,
    #[serde(default)]
    pub compare_visual_annotations_count: usize,
    #[serde(default)]
    pub ai_review_annotations_count: usize,
    /// Phase 24G: optional path to the HTML redline report when this export
    /// followed `Export Redline Review` in the UI. The backend itself does
    /// not write the report here — see `report_export_compare_review`.
    #[serde(default)]
    pub redline_report_path: Option<String>,

    // ── Phase 25F: per-overlay-kind subtotals ──
    #[serde(default)]
    pub text_boxes_count: usize,
    #[serde(default)]
    pub comments_count: usize,
    #[serde(default)]
    pub highlights_count: usize,
    #[serde(default)]
    pub shapes_count: usize,
    #[serde(default)]
    pub stamps_count: usize,
    #[serde(default)]
    pub signatures_count: usize,
    #[serde(default)]
    pub strikethroughs_count: usize,
    #[serde(default)]
    pub underlines_count: usize,
    #[serde(default)]
    pub images_count: usize,
    #[serde(default)]
    pub ocr_overlays_count: usize,
    /// Phase 25F: how many overlay objects had an unrecognised type and were
    /// therefore skipped (we record but do not abort). Mirrors `warnings`.
    #[serde(default)]
    pub skipped_unsupported_count: usize,
    #[serde(default)]
    pub skipped_items: Vec<String>,

    /// Phase 26A/F: how many signature image overlays were actually embedded
    /// into the exported PDF bytes (visible externally). Mirrors
    /// `signatures_count` only when every signature carried a valid
    /// `image_data_url`.
    #[serde(default)]
    pub signatures_embedded_count: usize,
    /// Phase 26A/F: how many signature image overlays failed to embed
    /// (bad data URL, decode failure, missing image, page out of range...).
    /// Each failure is mirrored in `warnings`.
    #[serde(default)]
    pub signatures_failed_count: usize,
    /// Phase 26F: detail strings for failed signature embeds — overlay id
    /// + reason — so the UI can surface them prominently.
    #[serde(default)]
    pub signatures_failed_items: Vec<String>,
    /// Phase 26D: how many path-cover overlays were committed.
    #[serde(default)]
    pub path_covers_count: usize,
    /// Phase 27A: how many signature embeds preserved the source aspect
    /// ratio (i.e. preserve_aspect = true and dims could be decoded).
    #[serde(default)]
    pub signatures_aspect_preserved_count: usize,
    /// Phase 27G: when true, overlay export order could not preserve the
    /// frontend zIndex ordering (e.g. annotation-creation order is fixed).
    #[serde(default)]
    pub zorder_export_limited: bool,
    /// Phase 27F: how many invisible OCR text layers were injected (0 if
    /// the searchable layer command was never invoked or returned
    /// `not_implemented`). Mirrored in warnings when 0 but expected.
    #[serde(default)]
    pub ocr_text_layers_count: usize,
    /// Phase 28G — text edit counters derived from the content-edit history.
    /// `native_text_edits_count` includes single-span native edits.
    #[serde(default)]
    pub native_text_edits_count: usize,
    /// Multi-operator block edits applied natively.
    #[serde(default)]
    pub native_block_edits_count: usize,
    /// Safe-visual replacements (single span and block) — neither native
    /// edit nor a true font preservation.
    #[serde(default)]
    pub visual_text_replacements_count: usize,
    /// Text objects that were rejected as read-only or unsupported.
    #[serde(default)]
    pub read_only_text_count: usize,
    /// How many text edits preserved the original font.
    #[serde(default)]
    pub font_preservation_count: usize,
    /// How many text edits had to fall back to a different font/method.
    #[serde(default)]
    pub text_fallback_count: usize,
    /// Phase 34I — committed image move/resize operations.
    #[serde(default)]
    pub image_edits_count: usize,
    /// Phase 34I — committed image replacement operations.
    #[serde(default)]
    pub image_replacements_count: usize,
    /// Phase 34I — committed image delete/cover operations.
    #[serde(default)]
    pub image_deletes_count: usize,
    /// Phase 35I — committed visual paragraph reflows.
    #[serde(default)]
    pub paragraph_reflows_count: usize,
    /// Phase 35I — committed find/replace operations.
    #[serde(default)]
    pub find_replace_count: usize,
    /// Phase 35I — committed image crop operations.
    #[serde(default)]
    pub image_crops_count: usize,
    /// Phase 35I — committed image rotation operations.
    #[serde(default)]
    pub image_rotations_count: usize,
    /// Phase 34I — visual covers emitted by text/image/path safe-visual edits.
    #[serde(default)]
    pub visual_covers_count: usize,
    /// Phase 35 — visual text covers that used sampled page background color.
    #[serde(default)]
    pub sampled_background_covers_count: usize,
    /// Phase 35 — visual text covers that fell back to white.
    #[serde(default)]
    pub white_fallback_covers_count: usize,
}

// ---------------------------------------------------------------------------
// Overlay → Annotation Mapping
// ---------------------------------------------------------------------------

/// Map an overlay object to a CreateAnnotationRequest.
/// Returns None for objects that should not be committed (e.g. draft redactions
/// when apply_redactions is false).
pub fn map_overlay_to_annotation(
    obj: &OverlayObject,
    session_id: &str,
    apply_redactions: bool,
) -> Result<Option<CreateAnnotationRequest>, String> {
    // Validate rect bounds.
    let [x0, y0, x1, y1] = obj.rect;
    if x1 <= x0 || y1 <= y0 {
        return Err(format!(
            "Invalid rect for object {}: [{}, {}, {}, {}]",
            obj.id, x0, y0, x1, y1
        ));
    }

    match obj.object_type.as_str() {
        "textBox" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#000000"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::FreeText,
                color,
                contents: obj.text.clone().unwrap_or_default(),
                author: obj.author.clone().unwrap_or_default(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        "comment" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#ffba6b"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::Note,
                color,
                contents: obj.text.clone().unwrap_or_default(),
                author: obj.author.clone().unwrap_or_default(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        "highlight" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#ffff00"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::Highlight,
                color,
                contents: obj.text.clone().unwrap_or_default(),
                author: String::new(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        "stamp" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#cc0000"));
            let stamp_name = obj.stamp_text.clone().or(obj.stamp_type.clone());
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::Stamp,
                color,
                contents: obj.stamp_text.clone().unwrap_or_default(),
                author: String::new(),
                rect: obj.rect,
                callout_points: None,
                stamp_name,
                ink_paths: None,
            }))
        }
        "rectangle" => {
            // Map to FreeText with border styling (Square annotation not in our enum).
            let color = parse_color(obj.stroke_color.as_deref().unwrap_or("#333333"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::FreeText,
                color,
                contents: String::new(),
                author: String::new(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        "redaction" => {
            // Only map redactions when explicitly requested.
            if !apply_redactions {
                return Ok(None);
            }
            // Redactions are handled separately via editing_core, not annotation_core.
            Ok(None)
        }
        // Phase 25A: native redline appearance.
        "strikethrough" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#c33333"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::Strikeout,
                color,
                contents: obj.text.clone().unwrap_or_default(),
                author: obj.author.clone().unwrap_or_default(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        "underline" => {
            let color = parse_color(obj.color.as_deref().unwrap_or("#3aa888"));
            Ok(Some(CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index: obj.page_index,
                annot_type: AnnotationType::Underline,
                color,
                contents: obj.text.clone().unwrap_or_default(),
                author: obj.author.clone().unwrap_or_default(),
                rect: obj.rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            }))
        }
        _ => Err(format!("Unknown overlay object type: {}", obj.object_type)),
    }
}

/// Build redaction edit operations for all redaction overlay objects.
pub fn build_redaction_operations(
    objects: &[OverlayObject],
    session_id: &str,
) -> Vec<EditOperation> {
    objects
        .iter()
        .filter(|obj| obj.object_type == "redaction")
        .filter(|obj| obj.status.as_deref() != Some("applied"))
        .enumerate()
        .map(|(i, obj)| {
            let payload = RedactPayload {
                page_index: obj.page_index,
                rect: obj.rect,
            };
            EditOperation {
                id: format!("redact-export-{}", i),
                op_type: EditOperationType::Redact,
                session_id: session_id.to_string(),
                page_index: obj.page_index,
                object_ref: Some(obj.id.clone()),
                payload_json: serde_json::to_string(&payload).unwrap_or_default(),
            }
        })
        .collect()
}

/// Parse a CSS color string (#RRGGBB or rgba) to AnnotationColor.
fn parse_color(color: &str) -> AnnotationColor {
    if color.starts_with('#') && color.len() >= 7 {
        let r = u8::from_str_radix(&color[1..3], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&color[3..5], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&color[5..7], 16).unwrap_or(0) as f32 / 255.0;
        AnnotationColor { r, g, b, a: 1.0 }
    } else if color.starts_with("rgba(") {
        // Simple rgba parser.
        let inner = color.trim_start_matches("rgba(").trim_end_matches(')');
        let parts: Vec<f32> = inner
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        AnnotationColor {
            r: parts.first().copied().unwrap_or(0.0),
            g: parts.get(1).copied().unwrap_or(0.0),
            b: parts.get(2).copied().unwrap_or(0.0),
            a: parts.get(3).copied().unwrap_or(1.0),
        }
    } else {
        AnnotationColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }
}

fn normalize_export_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err("EXPORT_INVALID_DESTINATION: destination path is invalid".to_string());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("pdf") {
        return Err("EXPORT_UNSUPPORTED_TYPE: destination must have a .pdf extension".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "EXPORT_INVALID_DESTINATION: destination has no parent".to_string())?;
    let normalized_parent = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    Ok(normalized_parent.join(path.file_name().unwrap()))
}

fn validate_export_request(
    request: &ExportRequest,
    page_count: usize,
    page_sizes: &[(f32, f32)],
    source_path: &str,
) -> Result<(), String> {
    let opts = &request.options;
    if opts.session_id.trim().is_empty() {
        return Err("EXPORT_INVALID_REQUEST: session_id is required".to_string());
    }
    if opts.redaction_mode != "export_copy" && opts.redaction_mode != "apply_to_current_document" {
        return Err(format!(
            "EXPORT_UNSUPPORTED_OPTION: invalid redaction mode {}",
            opts.redaction_mode
        ));
    }
    if opts.redaction_mode == "apply_to_current_document" && !opts.apply_redactions {
        return Err(
            "EXPORT_CONFLICTING_OPTIONS: destructive redaction mode requires apply_redactions"
                .to_string(),
        );
    }
    if request.overlay_objects.len() > 20_000 {
        return Err("EXPORT_INVALID_REQUEST: too many overlay objects".to_string());
    }

    let target = normalize_export_path(Path::new(&opts.output_path))?;
    let source = normalize_export_path(Path::new(source_path))?;
    if target == source {
        return Err(
            "EXPORT_INVALID_DESTINATION: export destination must differ from the open source PDF"
                .to_string(),
        );
    }

    for object in &request.overlay_objects {
        if object.id.trim().is_empty() || object.id.len() > 256 {
            return Err("EXPORT_INVALID_OVERLAY: overlay id is invalid".to_string());
        }
        if object.page_index >= page_count {
            return Err(format!(
                "EXPORT_PAGE_OUT_OF_RANGE: page {} of {}",
                object.page_index, page_count
            ));
        }
        let [x0, y0, x1, y1] = object.rect;
        if ![x0, y0, x1, y1].iter().all(|value| value.is_finite()) || x1 <= x0 || y1 <= y0 {
            return Err(format!(
                "EXPORT_INVALID_OVERLAY: invalid rect for {}",
                object.id
            ));
        }
        if let Some((width, height)) = page_sizes.get(object.page_index) {
            if x0 < 0.0 || y0 < 0.0 || x1 > *width || y1 > *height {
                return Err(format!(
                    "EXPORT_INVALID_OVERLAY: rect for {} exceeds page bounds",
                    object.id
                ));
            }
        }
        if !matches!(
            object.object_type.as_str(),
            "textBox"
                | "comment"
                | "highlight"
                | "rectangle"
                | "redaction"
                | "stamp"
                | "image"
                | "strikethrough"
                | "underline"
        ) {
            return Err(format!(
                "EXPORT_UNSUPPORTED_OVERLAY: {}",
                object.object_type
            ));
        }
        if object
            .image_data_url
            .as_ref()
            .is_some_and(|value| value.len() > 4 * 1024 * 1024)
        {
            return Err(format!(
                "EXPORT_INVALID_OVERLAY: image payload for {} is too large",
                object.id
            ));
        }
    }
    Ok(())
}

fn validate_output_file(
    path: &Path,
    expected_page_count: usize,
) -> Result<(usize, String, usize), String> {
    let metadata =
        std::fs::metadata(path).map_err(|err| format!("output file is unavailable: {err}"))?;
    let size =
        usize::try_from(metadata.len()).map_err(|_| "output file is too large".to_string())?;
    if size == 0 {
        return Err("output file is empty".to_string());
    }
    let bytes = std::fs::read(path).map_err(|err| format!("read output: {err}"))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err("output does not have a valid PDF header".to_string());
    }
    let pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes)
        .map_err(|err| format!("fresh PDF reopen failed: {err}"))?;
    let page_count = usize::try_from(
        pdf.page_count()
            .map_err(|err| format!("page count: {err}"))?,
    )
    .map_err(|_| "output page count is invalid".to_string())?;
    if page_count != expected_page_count {
        return Err(format!(
            "output page count {page_count} does not match expected {expected_page_count}"
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((size, format!("{:x}", hasher.finalize()), page_count))
}

// ---------------------------------------------------------------------------
// Export IPC Command
// ---------------------------------------------------------------------------

use tauri::State;

/// Export the current PDF with overlay objects committed as annotations.
/// Supports two redaction modes:
/// - "export_copy" (default): clones bytes, applies redactions to clone, writes output.
///   Does NOT mutate the current session.
/// - "apply_to_current_document": mutates the active session bytes (destructive).
#[tauri::command]
pub fn doc_export(
    doc_state: State<'_, DocumentCoreState>,
    history_state: State<'_, crate::content_editing::history::ContentEditHistoryState>,
    request: ExportRequest,
) -> Result<ExportResult, String> {
    crate::license::assert_feature_allowed("export")?;
    let started = Instant::now();

    let opts = &request.options;
    let session_id = &opts.session_id;
    let redaction_mode = if opts.redaction_mode.is_empty() {
        "export_copy"
    } else {
        &opts.redaction_mode
    };
    let mut warnings: Vec<String> = Vec::new();
    let mut annotations_committed = 0usize;
    let mut redactions_applied = 0usize;
    let mut source_mutated = false;
    let overlay_count = request.overlay_objects.len();

    let (mut export_bytes, page_count, source_path, page_sizes) = {
        let arc = doc_state
            .store
            .get_session_arc_pub(session_id)
            .map_err(|e| e.to_string())?;
        let session = arc
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        (
            session.document.bytes.clone(),
            session.page_count,
            session.document.source_path.clone(),
            session
                .document
                .pages
                .iter()
                .map(|page| (page.width_points, page.height_points))
                .collect::<Vec<_>>(),
        )
    };

    validate_export_request(&request, page_count, &page_sizes, &source_path)?;

    let target = Path::new(&opts.output_path);
    let normalized_target = normalize_export_path(target)?;
    let export_path_key = normalized_target.clone();
    {
        let mut active = ACTIVE_EXPORT_PATHS
            .lock()
            .map_err(|_| "EXPORT_CONCURRENCY_LOCK_POISONED".to_string())?;
        if !active.insert(export_path_key.clone()) {
            return Err(
                "EXPORT_ALREADY_RUNNING: another export is using this destination".to_string(),
            );
        }
    }
    let _export_guard = ExportPathGuard(export_path_key);

    // Warn about flattening limitation.
    if opts.flatten_annotations {
        warnings.push(
            "True content-stream flattening is not supported by the current MuPDF binding. \
             Annotations were committed with appearance streams and will display correctly \
             in standard PDF viewers, but remain editable annotation objects."
                .to_string(),
        );
    }

    // 1. Apply overlay annotations to the independent export snapshot. The
    // live session remains unchanged until a successful explicit native save.
    if opts.commit_annotations {
        for obj in &request.overlay_objects {
            match map_overlay_to_annotation(obj, session_id, opts.apply_redactions) {
                Ok(Some(annot_req)) => {
                    match AnnotationEngine::apply_annotation_to_bytes(&annot_req, &export_bytes) {
                        Ok(bytes) => {
                            export_bytes = bytes;
                            annotations_committed += 1;
                        }
                        Err(e) => {
                            return Err(format!("EXPORT_ANNOTATION_FAILED: {}: {}", obj.id, e));
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => return Err(format!("EXPORT_INVALID_OVERLAY: {e}")),
            }
        }
    }

    // 3. Apply redactions.
    if opts.apply_redactions {
        let redact_ops = build_redaction_operations(&request.overlay_objects, session_id);
        if !redact_ops.is_empty() {
            // Apply redactions to export_bytes (the clone), not the session.
            for op in &redact_ops {
                let payload: RedactPayload = serde_json::from_str(&op.payload_json)
                    .map_err(|e| format!("Redaction payload parse error: {e}"))?;

                let pdf = mupdf::pdf::PdfDocument::from_bytes(&export_bytes)
                    .map_err(|e| format!("Failed to open PDF for redaction: {e}"))?;

                let pc = pdf.page_count().map_err(|e| format!("page_count: {e}"))?;
                let page_no =
                    i32::try_from(payload.page_index).map_err(|e| format!("page index: {e}"))?;
                if page_no >= pc {
                    warnings.push(format!(
                        "Redaction page {} out of range",
                        payload.page_index
                    ));
                    continue;
                }

                let fz_page = pdf
                    .load_page(page_no)
                    .map_err(|e| format!("load_page: {e}"))?;
                let mut pdf_page =
                    mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage: {e}"))?;

                let rect = mupdf::Rect::new(
                    payload.rect[0],
                    payload.rect[1],
                    payload.rect[2],
                    payload.rect[3],
                );
                let mut annot = pdf_page
                    .create_annotation(mupdf::pdf::PdfAnnotationType::Redact)
                    .map_err(|e| format!("create redact: {e}"))?;
                annot.set_rect(rect).map_err(|e| format!("set_rect: {e}"))?;
                annot
                    .set_color(mupdf::color::AnnotationColor::Gray(0.0))
                    .map_err(|e| format!("set_color: {e}"))?;
                drop(annot);
                pdf_page.redact().map_err(|e| format!("redact: {e}"))?;

                let mut new_bytes: Vec<u8> = Vec::new();
                pdf.write_to(&mut new_bytes)
                    .map_err(|e| format!("serialize: {e}"))?;
                export_bytes = new_bytes;
                redactions_applied += 1;
            }

            // In both modes, keep the live session unchanged until the output
            // has been written and independently reopened. The destructive
            // mode is applied to the session at the commit point below.
        }
    }

    // 3.5 Phase 26A: embed visual signature image overlays into export_bytes.
    //     We do this AFTER the annotation commit (so signatures sit on top
    //     of the annotation layer) and BEFORE writing to disk. Failures are
    //     surfaced as warnings + per-signature failure counts; we never
    //     silently drop a signature.
    let mut signatures_embedded_count: usize = 0;
    let mut signatures_failed_count: usize = 0;
    let mut signatures_failed_items: Vec<String> = Vec::new();
    let mut signatures_aspect_preserved_count: usize = 0;
    let mut path_covers_count: usize = 0;
    {
        let sig_specs: Vec<super::signature_embed::SignatureEmbedSpec> = request
            .overlay_objects
            .iter()
            .filter(|o| o.source.as_deref() == Some("signature"))
            .filter_map(|o| {
                let url = o.image_data_url.as_deref()?;
                Some(super::signature_embed::SignatureEmbedSpec {
                    id: o.id.clone(),
                    page_index: o.page_index,
                    rect: o.rect,
                    image_data_url: url.to_string(),
                    preserve_aspect: o.preserve_aspect.unwrap_or(false),
                    image_natural_width: o.image_natural_width,
                    image_natural_height: o.image_natural_height,
                })
            })
            .collect();

        // Honest accounting: signatures present in overlays but missing
        // an image_data_url cannot be embedded; surface them so the UI
        // can warn the user instead of silently exporting as a Stamp.
        for o in &request.overlay_objects {
            if o.source.as_deref() == Some("signature") && o.image_data_url.is_none() {
                signatures_failed_count += 1;
                signatures_failed_items.push(format!(
                    "{} (no image_data_url — signature exported only as Stamp label)",
                    o.id
                ));
                warnings.push(format!(
                    "Signature {} has no image data; only the Stamp label will appear externally.",
                    o.id
                ));
            }
        }

        if !sig_specs.is_empty() {
            let mut pdf = match mupdf::pdf::PdfDocument::from_bytes(&export_bytes) {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!(
                        "Skipped signature embedding: failed to reopen PDF: {e}"
                    ));
                    return Err(format!("signature embed: reopen failed: {e}"));
                }
            };

            // Take a one-time snapshot of page dimensions for validation.
            let page_dims: Vec<(f32, f32)> = {
                let arc = doc_state
                    .store
                    .get_session_arc_pub(session_id)
                    .map_err(|e| e.to_string())?;
                let session = arc
                    .lock()
                    .map_err(|_| "session lock poisoned".to_string())?;
                session
                    .document
                    .pages
                    .iter()
                    .map(|p| (p.width_points, p.height_points))
                    .collect()
            };

            let report = super::signature_embed::embed_signatures(
                &mut pdf,
                |idx| page_dims.get(idx).map(|(w, _)| *w),
                |idx| page_dims.get(idx).map(|(_, h)| *h),
                &sig_specs,
            );
            signatures_embedded_count += report.embedded;
            signatures_failed_count += report.failed;
            signatures_aspect_preserved_count += report.aspect_preserved;
            for id in &report.failed_ids {
                signatures_failed_items.push(id.clone());
            }
            warnings.extend(report.warnings);

            if report.embedded > 0 {
                let mut rewritten: Vec<u8> = Vec::new();
                pdf.write_to(&mut rewritten)
                    .map_err(|e| format!("signature embed: serialize: {e}"))?;
                export_bytes = rewritten;
            }
        }

        // Phase 26D: count path-cover overlays for the summary (they are
        // already serialised as `rectangle` overlay objects with a white
        // fill; the export pipeline embeds them via the standard rect
        // annotation path).
        for o in &request.overlay_objects {
            if o.source.as_deref() == Some("path_cover") {
                path_covers_count += 1;
            }
        }
    }

    // 4. Validate the complete snapshot, write it atomically, then reopen the
    // resulting file independently before returning success.
    let output_path = &opts.output_path;

    if target.exists() && !opts.overwrite_existing {
        return Err(format!("Output file already exists: {}", output_path));
    }

    let parent = target
        .parent()
        .ok_or_else(|| "Invalid output path".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("Create dir: {e}"))?;

    let tmp_path = parent.join(format!(
        ".{}.r2h-export.tmp-{}-{}",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("export.pdf"),
        std::process::id(),
        epoch_ms()
    ));
    let backup_path = parent.join(format!(
        ".{}.r2h-export.bak",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("export.pdf")
    ));

    let temp_write = (|| -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|e| format!("Create temp file: {e}"))?;
        file.write_all(&export_bytes)
            .map_err(|e| format!("Write: {e}"))?;
        file.flush().map_err(|e| format!("Flush: {e}"))?;
        file.sync_all().map_err(|e| format!("Sync: {e}"))?;
        Ok(())
    })();
    if let Err(err) = temp_write {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }

    let (_, _, validated_page_count) = match validate_output_file(&tmp_path, page_count) {
        Ok(result) => result,
        Err(err) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("EXPORT_OUTPUT_INVALID: {err}"));
        }
    };
    if validated_page_count != page_count {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(
            "EXPORT_OUTPUT_INVALID: temporary output failed independent validation".to_string(),
        );
    }

    let target_existed = target.exists();
    if target_existed {
        if backup_path.exists() {
            std::fs::remove_file(&backup_path).map_err(|e| format!("Remove old backup: {e}"))?;
        }
        std::fs::rename(target, &backup_path).map_err(|e| format!("Stage existing output: {e}"))?;
    }
    if let Err(err) = std::fs::rename(&tmp_path, target) {
        if target_existed && backup_path.exists() {
            let _ = std::fs::rename(&backup_path, target);
        }
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("Rename: {err}"));
    }

    let (validated_size, validated_hash, _) = match validate_output_file(target, page_count) {
        Ok(result) => result,
        Err(err) => {
            let _ = std::fs::remove_file(target);
            if target_existed && backup_path.exists() {
                let _ = std::fs::rename(&backup_path, target);
            }
            return Err(format!("EXPORT_OUTPUT_REOPEN_FAILED: {err}"));
        }
    };
    if target_existed && backup_path.exists() {
        let _ = std::fs::remove_file(&backup_path);
    }

    if redaction_mode == "apply_to_current_document" && redactions_applied > 0 {
        let arc = doc_state
            .store
            .get_session_arc_pub(session_id)
            .map_err(|e| e.to_string())?;
        let mut session = arc
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes = export_bytes.clone();
        session.is_dirty = true;
        session.invalidate_cached_document();
        source_mutated = true;
    }

    // Phase 24G + 25F: provenance subtotals + per-overlay-kind subtotals.
    let mut compare_annotations_count = 0;
    let mut compare_visual_annotations_count = 0;
    let mut ai_review_annotations_count = 0;
    let mut text_boxes_count = 0;
    let mut comments_count = 0;
    let mut highlights_count = 0;
    let mut shapes_count = 0;
    let mut stamps_count = 0;
    let mut signatures_count = 0;
    let mut strikethroughs_count = 0;
    let mut underlines_count = 0;
    let mut images_count = 0;
    let mut ocr_overlays_count = 0;
    let mut skipped_unsupported_count = 0;
    let mut skipped_items: Vec<String> = Vec::new();

    for obj in &request.overlay_objects {
        match obj.source.as_deref() {
            Some("compare") => compare_annotations_count += 1,
            Some("compare_visual") => compare_visual_annotations_count += 1,
            Some("ai_review") => ai_review_annotations_count += 1,
            Some("signature") => signatures_count += 1,
            Some("ocr") => ocr_overlays_count += 1,
            _ => {}
        }
        match obj.object_type.as_str() {
            "textBox" => text_boxes_count += 1,
            "comment" => comments_count += 1,
            "highlight" => highlights_count += 1,
            "rectangle" => shapes_count += 1,
            "stamp" => stamps_count += 1,
            "strikethrough" => strikethroughs_count += 1,
            "underline" => underlines_count += 1,
            "image" => images_count += 1,
            "redaction" => { /* tracked separately in redactions_applied_count */ }
            other => {
                // Only count as skipped if no recognised mapping exists.
                if !matches!(
                    other,
                    "textBox"
                        | "comment"
                        | "highlight"
                        | "rectangle"
                        | "stamp"
                        | "strikethrough"
                        | "underline"
                        | "image"
                        | "redaction"
                ) {
                    skipped_unsupported_count += 1;
                    skipped_items.push(format!("{} (id={})", other, obj.id));
                }
            }
        }
    }

    // Phase 28G — derive text edit counters from the history state for
    // the export summary.
    let (
        native_text_edits_count,
        native_block_edits_count,
        visual_text_replacements_count,
        read_only_text_count,
        font_preservation_count,
        text_fallback_count,
    ) = count_text_edits(&history_state, session_id);
    let (
        image_edits_count,
        image_replacements_count,
        image_deletes_count,
        image_crops_count,
        image_rotations_count,
        visual_covers_count,
        sampled_background_covers_count,
        white_fallback_covers_count,
    ) = count_image_edits(&history_state, session_id);
    let (paragraph_reflows_count, find_replace_count) =
        count_phase35_text_edits(&history_state, session_id);

    Ok(ExportResult {
        output_path: output_path.clone(),
        output_file_size_bytes: validated_size,
        output_sha256: validated_hash,
        export_type: "pdf".to_string(),
        independently_validated: true,
        page_count,
        overlay_objects_received_count: overlay_count,
        annotations_committed_count: annotations_committed,
        redactions_applied_count: redactions_applied,
        redaction_mode: redaction_mode.to_string(),
        source_document_mutated: source_mutated,
        true_flattening_supported: false,
        flattened: false,
        warnings,
        created_at: epoch_ms(),
        duration_ms: started.elapsed().as_millis(),
        compare_annotations_count,
        compare_visual_annotations_count,
        ai_review_annotations_count,
        redline_report_path: None,
        text_boxes_count,
        comments_count,
        highlights_count,
        shapes_count,
        stamps_count,
        signatures_count,
        strikethroughs_count,
        underlines_count,
        images_count,
        ocr_overlays_count,
        skipped_unsupported_count,
        skipped_items,
        signatures_embedded_count,
        signatures_failed_count,
        signatures_failed_items,
        path_covers_count,
        signatures_aspect_preserved_count,
        // Phase 27G — overlay export currently maps overlays to
        // annotations in payload order. PDF annotation rendering follows
        // PDF /Annots array order, which mostly matches creation order
        // but does not honour our editor zIndex outside that ordering.
        // We surface this honestly so the UI can warn.
        zorder_export_limited: true,
        ocr_text_layers_count: 0,
        native_text_edits_count,
        native_block_edits_count,
        visual_text_replacements_count,
        read_only_text_count,
        font_preservation_count,
        text_fallback_count,
        image_edits_count,
        image_replacements_count,
        image_deletes_count,
        paragraph_reflows_count,
        find_replace_count,
        image_crops_count,
        image_rotations_count,
        visual_covers_count,
        sampled_background_covers_count,
        white_fallback_covers_count,
    })
}

/// Phase 28G — read the content-edit history and derive text-edit
/// counters that the export summary surfaces to the UI.
fn count_text_edits(
    history_state: &crate::content_editing::history::ContentEditHistoryState,
    session_id: &str,
) -> (usize, usize, usize, usize, usize, usize) {
    use crate::content_editing::types::EditMethod;
    let records = history_state.list_records(session_id);
    let mut native_text = 0usize;
    let mut native_block = 0usize;
    let mut visual_repl = 0usize;
    let mut read_only = 0usize;
    let mut font_preserved = 0usize;
    let mut fallback = 0usize;
    for r in records {
        // Phase 28G — only text-related edits count here.
        let is_text = matches!(r.edit_type.as_str(), "text_edit" | "text_block_edit");
        if !is_text {
            continue;
        }
        if r.warnings.iter().any(|w| w == "REVERTED") {
            // Reverted edits do not contribute to the export counters.
            continue;
        }
        match r.method {
            EditMethod::NativeInPlaceEdit => {
                native_text += 1;
                font_preserved += 1;
            }
            EditMethod::NativeMultiOperator => {
                native_block += 1;
                font_preserved += 1;
            }
            EditMethod::SafeVisualReplacement => {
                visual_repl += 1;
                fallback += 1;
            }
            EditMethod::VisualPatchFallback => {
                visual_repl += 1;
                fallback += 1;
            }
            EditMethod::Rejected => {
                read_only += 1;
            }
            EditMethod::NativeXobjectSwap => { /* image; ignored here */ }
        }
    }
    (
        native_text,
        native_block,
        visual_repl,
        read_only,
        font_preserved,
        fallback,
    )
}

fn count_image_edits(
    history_state: &crate::content_editing::history::ContentEditHistoryState,
    session_id: &str,
) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    use crate::content_editing::types::EditMethod;
    let records = history_state.list_records(session_id);
    let mut image_edits = 0usize;
    let mut image_replacements = 0usize;
    let mut image_deletes = 0usize;
    let mut image_crops = 0usize;
    let mut image_rotations = 0usize;
    let mut visual_covers = 0usize;
    let mut sampled_background_covers = 0usize;
    let mut white_fallback_covers = 0usize;
    for r in records {
        if r.warnings.iter().any(|w| w == "REVERTED") {
            continue;
        }
        match r.edit_type.as_str() {
            "image_move_resize" => image_edits += 1,
            "image_replace" => image_replacements += 1,
            "image_delete" => image_deletes += 1,
            "image_crop" => image_crops += 1,
            "image_rotation" => image_rotations += 1,
            _ => {}
        }
        if matches!(
            r.method,
            EditMethod::SafeVisualReplacement | EditMethod::VisualPatchFallback
        ) && matches!(
            r.edit_type.as_str(),
            "text_edit"
                | "text_block_edit"
                | "find_replace"
                | "image_move_resize"
                | "image_replace"
                | "image_delete"
                | "image_crop"
                | "image_rotation"
                | "path_cover"
        ) {
            visual_covers += 1;
            if r.warnings.iter().any(|w| {
                w.contains("sampled background cover color")
                    || w.contains("Sampled background cover color")
            }) {
                sampled_background_covers += 1;
            }
            if r.warnings
                .iter()
                .any(|w| w.contains("white cover used") || w.contains("white rectangle"))
            {
                white_fallback_covers += 1;
            }
        }
    }
    (
        image_edits,
        image_replacements,
        image_deletes,
        image_crops,
        image_rotations,
        visual_covers,
        sampled_background_covers,
        white_fallback_covers,
    )
}

fn count_phase35_text_edits(
    history_state: &crate::content_editing::history::ContentEditHistoryState,
    session_id: &str,
) -> (usize, usize) {
    let records = history_state.list_records(session_id);
    let mut paragraph_reflows = 0usize;
    let mut find_replace = 0usize;
    for r in records {
        if r.warnings.iter().any(|w| w == "REVERTED") {
            continue;
        }
        match r.edit_type.as_str() {
            "text_block_edit"
                if matches!(
                    r.method,
                    crate::content_editing::types::EditMethod::SafeVisualReplacement
                ) =>
            {
                paragraph_reflows += 1;
            }
            "find_replace" => find_replace += 1,
            _ => {}
        }
    }
    (paragraph_reflows, find_replace)
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_textbox_to_freetext() {
        let obj = OverlayObject {
            id: "t1".to_string(),
            object_type: "textBox".to_string(),
            page_index: 0,
            rect: [10.0, 20.0, 200.0, 60.0],
            text: Some("Hello".to_string()),
            color: Some("#ff0000".to_string()),
            font_size: Some(14.0),
            author: Some("User".to_string()),
            stamp_text: None,
            stamp_type: None,
            opacity: None,
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "s1", false).unwrap();
        assert!(result.is_some());
        let annot = result.unwrap();
        assert!(matches!(annot.annot_type, AnnotationType::FreeText));
        assert_eq!(annot.contents, "Hello");
        assert!((annot.color.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn map_comment_to_note() {
        let obj = OverlayObject {
            id: "c1".to_string(),
            object_type: "comment".to_string(),
            page_index: 1,
            rect: [50.0, 700.0, 74.0, 724.0],
            text: Some("Check this".to_string()),
            color: Some("#ffba6b".to_string()),
            font_size: None,
            author: Some("Reviewer".to_string()),
            stamp_text: None,
            stamp_type: None,
            opacity: None,
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "s1", false)
            .unwrap()
            .unwrap();
        assert!(matches!(result.annot_type, AnnotationType::Note));
        assert_eq!(result.contents, "Check this");
        assert_eq!(result.author, "Reviewer");
    }

    #[test]
    fn map_highlight_to_highlight() {
        let obj = OverlayObject {
            id: "h1".to_string(),
            object_type: "highlight".to_string(),
            page_index: 0,
            rect: [72.0, 500.0, 272.0, 514.0],
            text: None,
            color: Some("#ffff00".to_string()),
            font_size: None,
            author: None,
            stamp_text: None,
            stamp_type: None,
            opacity: Some(0.4),
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "s1", false)
            .unwrap()
            .unwrap();
        assert!(matches!(result.annot_type, AnnotationType::Highlight));
    }

    #[test]
    fn map_stamp_to_stamp() {
        let obj = OverlayObject {
            id: "s1".to_string(),
            object_type: "stamp".to_string(),
            page_index: 0,
            rect: [200.0, 600.0, 350.0, 650.0],
            text: None,
            color: Some("#cc0000".to_string()),
            font_size: None,
            author: None,
            stamp_text: Some("APPROVED".to_string()),
            stamp_type: Some("APPROVED".to_string()),
            opacity: None,
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "sess", false)
            .unwrap()
            .unwrap();
        assert!(matches!(result.annot_type, AnnotationType::Stamp));
        assert_eq!(result.stamp_name, Some("APPROVED".to_string()));
    }

    #[test]
    fn redaction_not_applied_without_flag() {
        let obj = OverlayObject {
            id: "r1".to_string(),
            object_type: "redaction".to_string(),
            page_index: 0,
            rect: [100.0, 300.0, 300.0, 320.0],
            text: None,
            color: None,
            font_size: None,
            author: None,
            stamp_text: None,
            stamp_type: None,
            opacity: None,
            status: Some("draft".to_string()),
            reason: Some("PII".to_string()),
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "s1", false).unwrap();
        assert!(
            result.is_none(),
            "Redaction should not be committed without apply flag"
        );
    }

    #[test]
    fn invalid_rect_produces_error() {
        let obj = OverlayObject {
            id: "bad".to_string(),
            object_type: "textBox".to_string(),
            page_index: 0,
            rect: [100.0, 100.0, 50.0, 50.0], // x1 < x0
            text: None,
            color: None,
            font_size: None,
            author: None,
            stamp_text: None,
            stamp_type: None,
            opacity: None,
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: None,
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        };
        let result = map_overlay_to_annotation(&obj, "s1", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid rect"));
    }

    #[test]
    fn build_redaction_ops_filters_draft_only() {
        let objects = vec![
            OverlayObject {
                id: "r1".to_string(),
                object_type: "redaction".to_string(),
                page_index: 0,
                rect: [10.0, 10.0, 100.0, 30.0],
                text: None,
                color: None,
                font_size: None,
                author: None,
                stamp_text: None,
                stamp_type: None,
                opacity: None,
                status: Some("draft".to_string()),
                reason: None,
                replacement_text: None,
                stroke_color: None,
                stroke_width: None,
                fill_color: None,
                source: None,
                image_data_url: None,
                preserve_aspect: None,
                image_natural_width: None,
                image_natural_height: None,
            },
            OverlayObject {
                id: "r2".to_string(),
                object_type: "redaction".to_string(),
                page_index: 1,
                rect: [10.0, 10.0, 100.0, 30.0],
                text: None,
                color: None,
                font_size: None,
                author: None,
                stamp_text: None,
                stamp_type: None,
                opacity: None,
                status: Some("applied".to_string()),
                reason: None,
                replacement_text: None,
                stroke_color: None,
                stroke_width: None,
                fill_color: None,
                source: None,
                image_data_url: None,
                preserve_aspect: None,
                image_natural_width: None,
                image_natural_height: None,
            },
        ];
        let ops = build_redaction_operations(&objects, "s1");
        assert_eq!(ops.len(), 1, "Only draft redactions should be included");
        assert_eq!(ops[0].page_index, 0);
    }

    #[test]
    fn parse_hex_color() {
        let c = parse_color("#ff8040");
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
        assert!((c.b - 0.251).abs() < 0.01);
    }

    // ─── Phase 25A: strikethrough / underline mapping ───

    fn obj_with(kind: &str, source: Option<&str>) -> OverlayObject {
        OverlayObject {
            id: format!("o-{}", kind),
            object_type: kind.to_string(),
            page_index: 0,
            rect: [10.0, 10.0, 100.0, 30.0],
            text: Some("redline".to_string()),
            color: Some("#c33333".to_string()),
            font_size: None,
            author: Some("R".to_string()),
            stamp_text: None,
            stamp_type: None,
            opacity: None,
            status: None,
            reason: None,
            replacement_text: None,
            stroke_color: None,
            stroke_width: None,
            fill_color: None,
            source: source.map(|s| s.to_string()),
            image_data_url: None,
            preserve_aspect: None,
            image_natural_width: None,
            image_natural_height: None,
        }
    }

    #[test]
    fn map_strikethrough_to_strikeout_annotation() {
        let result = map_overlay_to_annotation(&obj_with("strikethrough", None), "s1", false)
            .unwrap()
            .unwrap();
        assert!(matches!(result.annot_type, AnnotationType::Strikeout));
        assert_eq!(result.contents, "redline");
    }

    #[test]
    fn map_underline_to_underline_annotation() {
        let result = map_overlay_to_annotation(&obj_with("underline", None), "s1", false)
            .unwrap()
            .unwrap();
        assert!(matches!(result.annot_type, AnnotationType::Underline));
    }

    // ─── Phase 25F: ExportResult subtotals are populated ───

    #[test]
    fn export_result_has_per_kind_subtotal_fields() {
        // The Default impl for ExportResult does NOT exist; we just confirm
        // a manually-constructed value can carry the new fields. This
        // covers the schema contract used by the frontend.
        let r = ExportResult {
            output_path: "/tmp/x.pdf".into(),
            output_file_size_bytes: 1,
            output_sha256: "hash".into(),
            export_type: "pdf".into(),
            independently_validated: true,
            page_count: 0,
            overlay_objects_received_count: 0,
            annotations_committed_count: 0,
            redactions_applied_count: 0,
            redaction_mode: "export_copy".into(),
            source_document_mutated: false,
            true_flattening_supported: false,
            flattened: false,
            warnings: vec![],
            created_at: 0,
            duration_ms: 0,
            compare_annotations_count: 1,
            compare_visual_annotations_count: 2,
            ai_review_annotations_count: 3,
            redline_report_path: None,
            text_boxes_count: 4,
            comments_count: 5,
            highlights_count: 6,
            shapes_count: 7,
            stamps_count: 8,
            signatures_count: 9,
            strikethroughs_count: 10,
            underlines_count: 11,
            images_count: 12,
            ocr_overlays_count: 13,
            skipped_unsupported_count: 1,
            skipped_items: vec!["unknown (id=zz)".into()],
            signatures_embedded_count: 9,
            signatures_failed_count: 1,
            signatures_failed_items: vec!["sig-x (bad data url)".into()],
            path_covers_count: 2,
            signatures_aspect_preserved_count: 6,
            zorder_export_limited: true,
            ocr_text_layers_count: 0,
            native_text_edits_count: 3,
            native_block_edits_count: 1,
            visual_text_replacements_count: 2,
            read_only_text_count: 1,
            font_preservation_count: 4,
            text_fallback_count: 2,
            image_edits_count: 1,
            image_replacements_count: 2,
            image_deletes_count: 3,
            paragraph_reflows_count: 5,
            find_replace_count: 6,
            image_crops_count: 7,
            image_rotations_count: 8,
            visual_covers_count: 4,
            sampled_background_covers_count: 2,
            white_fallback_covers_count: 1,
        };
        assert_eq!(r.signatures_count, 9);
        assert_eq!(r.ocr_overlays_count, 13);
        assert_eq!(r.skipped_items.len(), 1);
        assert_eq!(r.signatures_aspect_preserved_count, 6);
        assert!(r.zorder_export_limited);
        assert_eq!(r.ocr_text_layers_count, 0);
        assert_eq!(r.native_text_edits_count, 3);
        assert_eq!(r.native_block_edits_count, 1);
        assert_eq!(r.font_preservation_count, 4);
        assert_eq!(r.image_replacements_count, 2);
        assert_eq!(r.paragraph_reflows_count, 5);
        assert_eq!(r.image_crops_count, 7);
        assert_eq!(r.visual_covers_count, 4);
        assert_eq!(r.sampled_background_covers_count, 2);
        assert_eq!(r.white_fallback_covers_count, 1);
    }

    // ─── Phase 28G: count_text_edits derives counters from history ──

    #[test]
    fn count_text_edits_aggregates_by_method() {
        use crate::content_editing::history::ContentEditHistoryState;
        use crate::content_editing::types::{ContentEditRecord, EditMethod};
        let history = ContentEditHistoryState::new();
        history.add_record(ContentEditRecord {
            edit_id: "1".into(),
            session_id: "s1".into(),
            page_index: 0,
            content_object_id: "co1".into(),
            edit_type: "text_edit".into(),
            method: EditMethod::NativeInPlaceEdit,
            before_summary: "x".into(),
            after_summary: "y".into(),
            timestamp: 0,
            reversible: true,
            warnings: vec![],
            before_stream_snapshot: None,
        });
        history.add_record(ContentEditRecord {
            edit_id: "2".into(),
            session_id: "s1".into(),
            page_index: 0,
            content_object_id: "co2".into(),
            edit_type: "text_edit".into(),
            method: EditMethod::SafeVisualReplacement,
            before_summary: "x".into(),
            after_summary: "y".into(),
            timestamp: 0,
            reversible: true,
            warnings: vec![],
            before_stream_snapshot: None,
        });
        history.add_record(ContentEditRecord {
            edit_id: "3".into(),
            session_id: "s1".into(),
            page_index: 0,
            content_object_id: "blk".into(),
            edit_type: "text_block_edit".into(),
            method: EditMethod::NativeMultiOperator,
            before_summary: "x".into(),
            after_summary: "y".into(),
            timestamp: 0,
            reversible: true,
            warnings: vec![],
            before_stream_snapshot: None,
        });
        history.add_record(ContentEditRecord {
            edit_id: "4".into(),
            session_id: "s1".into(),
            page_index: 0,
            content_object_id: "co4".into(),
            edit_type: "text_edit".into(),
            method: EditMethod::Rejected,
            before_summary: "x".into(),
            after_summary: "y".into(),
            timestamp: 0,
            reversible: false,
            warnings: vec![],
            before_stream_snapshot: None,
        });
        let (nt, nb, vis, ro, fp, fb) = count_text_edits(&history, "s1");
        assert_eq!(nt, 1);
        assert_eq!(nb, 1);
        assert_eq!(vis, 1);
        assert_eq!(ro, 1);
        assert_eq!(fp, 2);
        assert_eq!(fb, 1);
    }

    #[test]
    fn count_text_edits_ignores_reverted() {
        use crate::content_editing::history::ContentEditHistoryState;
        use crate::content_editing::types::{ContentEditRecord, EditMethod};
        let history = ContentEditHistoryState::new();
        history.add_record(ContentEditRecord {
            edit_id: "1".into(),
            session_id: "s1".into(),
            page_index: 0,
            content_object_id: "co1".into(),
            edit_type: "text_edit".into(),
            method: EditMethod::NativeInPlaceEdit,
            before_summary: "x".into(),
            after_summary: "y".into(),
            timestamp: 0,
            reversible: false,
            warnings: vec!["REVERTED".to_string()],
            before_stream_snapshot: None,
        });
        let (nt, _, _, _, fp, _) = count_text_edits(&history, "s1");
        assert_eq!(nt, 0);
        assert_eq!(fp, 0);
    }

    #[test]
    fn count_image_edits_aggregates_committed_image_history() {
        use crate::content_editing::history::ContentEditHistoryState;
        use crate::content_editing::types::{ContentEditRecord, EditMethod};
        let history = ContentEditHistoryState::new();
        for (edit_id, edit_type, method) in [
            ("m1", "image_move_resize", EditMethod::SafeVisualReplacement),
            ("r1", "image_replace", EditMethod::SafeVisualReplacement),
            ("d1", "image_delete", EditMethod::SafeVisualReplacement),
            ("c1", "image_crop", EditMethod::SafeVisualReplacement),
            ("rot1", "image_rotation", EditMethod::SafeVisualReplacement),
            ("r2", "image_replace", EditMethod::NativeXobjectSwap),
            ("old", "image_delete", EditMethod::SafeVisualReplacement),
        ] {
            history.add_record(ContentEditRecord {
                edit_id: edit_id.into(),
                session_id: "s1".into(),
                page_index: 0,
                content_object_id: "img".into(),
                edit_type: edit_type.into(),
                method,
                before_summary: "before".into(),
                after_summary: "after".into(),
                timestamp: 0,
                reversible: true,
                warnings: if edit_id == "old" {
                    vec!["REVERTED".to_string()]
                } else if edit_id == "m1" {
                    vec!["Sampled background cover color rgb(0.9, 0.9, 0.9).".to_string()]
                } else if edit_id == "d1" {
                    vec!["Background could not be sampled reliably; white cover used.".to_string()]
                } else {
                    vec![]
                },
                before_stream_snapshot: None,
            });
        }
        let (moves, replacements, deletes, crops, rotations, covers, sampled, white) =
            count_image_edits(&history, "s1");
        assert_eq!(moves, 1);
        assert_eq!(replacements, 2);
        assert_eq!(deletes, 1);
        assert_eq!(crops, 1);
        assert_eq!(rotations, 1);
        assert_eq!(covers, 5);
        assert_eq!(sampled, 1);
        assert_eq!(white, 1);
    }

    // ─── Phase 27A: OverlayObject accepts preserve_aspect/natural dims ──

    #[test]
    fn overlay_object_round_trips_preserve_aspect_fields() {
        let json = r#"{
            "id": "sig-1",
            "object_type": "stamp",
            "page_index": 0,
            "rect": [10.0, 10.0, 100.0, 50.0],
            "text": null,
            "color": null,
            "font_size": null,
            "author": null,
            "stamp_text": null,
            "stamp_type": null,
            "opacity": null,
            "status": null,
            "reason": null,
            "replacement_text": null,
            "stroke_color": null,
            "stroke_width": null,
            "fill_color": null,
            "source": "signature",
            "image_data_url": "data:image/png;base64,AA==",
            "preserve_aspect": true,
            "image_natural_width": 800,
            "image_natural_height": 200
        }"#;
        let obj: OverlayObject = serde_json::from_str(json).unwrap();
        assert_eq!(obj.preserve_aspect, Some(true));
        assert_eq!(obj.image_natural_width, Some(800));
        assert_eq!(obj.image_natural_height, Some(200));
    }
}
