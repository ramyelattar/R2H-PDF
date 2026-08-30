use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfObjectSummary {
    pub object_number: u32,
    pub generation: u16,
    pub byte_offset_hint: usize,
    pub type_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontInfo {
    pub name: String,
    pub embedded: bool,
    pub subset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub index: usize,
    pub width_points: f32,
    pub height_points: f32,
    pub rotation: i32,
    pub has_text: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub page_count: usize,
    pub object_count: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    pub producer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDocumentRequest {
    pub path: String,
    pub recover_if_damaged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenDocumentResponse {
    pub session_id: String,
    pub source_path: String,
    pub recovered: bool,
    pub summary: DocumentSummary,
    pub pages: Vec<PageInfo>,
    pub fonts: Vec<FontInfo>,
    pub objects: Vec<PdfObjectSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderRequest {
    pub session_id: String,
    pub page_index: usize,
    pub zoom: f32,
    pub viewport: BBox,
    pub device_pixel_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResponse {
    pub session_id: String,
    pub page_index: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub width: u32,
    pub height: u32,
    pub zoom: f32,
    pub cache_hit: bool,
    pub pixels_rgba: Vec<u8>,
    pub render_time_ms: u128,
    /// Page width in PDF points (zoom-independent). Required by the canvas
    /// inline editor and hit-test overlay to place editor boxes at the
    /// correct screen coordinates. Defaulted to 0 for old serialized
    /// payloads.
    #[serde(default)]
    pub width_pts: f32,
    /// Page height in PDF points (zoom-independent). Required by the canvas
    /// inline editor and hit-test overlay so the PDF Y-up coordinate flip
    /// produces the correct on-screen top.
    #[serde(default)]
    pub height_pts: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPermissions {
    pub can_print: bool,
    pub can_copy: bool,
    pub can_edit: bool,
    pub can_annotate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportState {
    pub page_index: usize,
    pub zoom: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub fit_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSpan {
    pub content: String,
    pub bbox: BBox,
    pub font_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextExtractionRequest {
    pub session_id: String,
    pub page_index: usize,
    pub region: Option<BBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextExtractionResponse {
    pub session_id: String,
    pub page_index: usize,
    pub full_text: String,
    pub spans: Vec<TextSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalSaveRequest {
    pub session_id: String,
    pub target_path: Option<String>,
    pub fsync: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalSaveResponse {
    pub session_id: String,
    pub saved_to: String,
    pub bytes_written: usize,
    pub integrity_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub recovered: bool,
    pub method: String,
    pub warnings: Vec<String>,
    pub repaired_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateRequest {
    pub session_id: String,
    pub page_index: usize,
    pub zoom: f32,
    pub viewport: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateResponse {
    pub session_id: String,
    pub active_page: usize,
    pub prefetch_pages: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDiagnostics {
    pub session_id: String,
    pub active_sessions: usize,
    pub parsed_document_available: bool,
    pub parsed_document_open_count: usize,
    pub document_hash: String,
    pub document_revision: u64,
    pub page_cache_entries: usize,
    pub page_cache_bytes: usize,
    pub render_cache_hit_count: usize,
    pub render_cache_miss_count: usize,
    pub opened_at_epoch_ms: u128,
    pub total_renders: usize,
    pub total_text_extractions: usize,
}

/// Phase 25A: per-line text + bbox extracted from a single page.
/// Bbox is in PDF points, origin bottom-left, format [x0, y0, x1, y1].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineBbox {
    pub text: String,
    pub bbox: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageLines {
    pub page_index: usize,
    pub lines: Vec<LineBbox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub field_type: String, // "text" | "checkbox" | "radio" | "signature"
    pub value: String,
    pub page_index: usize,
    pub rect: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRange {
    pub start: usize, // inclusive, 0-based
    pub end: usize,   // inclusive, 0-based
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateResponse {
    pub session_id: String,
    pub file_path: String,
    pub source_path: String,
    pub title: String,
    pub current_page: usize,
    pub active_page: usize,
    pub page_count: usize,
    pub total_pages: usize,
    pub zoom: f32,
    pub rotation: i32,
    pub is_dirty: bool,
    pub dirty: bool,
    pub is_scanned: bool,
    pub permissions: SessionPermissions,
    pub last_saved_at: Option<u128>,
    pub viewport: ViewportState,
    pub load_state: String,
}
