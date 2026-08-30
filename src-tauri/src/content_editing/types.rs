//! Types for the content editing engine.

use serde::{Deserialize, Serialize};

/// Editability level of a content object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditableLevel {
    /// Can be natively edited in the content stream.
    NativeEditable,
    /// Can be replaced (e.g., image XObject swap) but not inline-edited.
    NativeReplaceable,
    /// Cannot be safely edited natively; visual patch overlay is the only option.
    VisualPatchOnly,
    /// Read-only; no editing possible.
    ReadOnly,
}

/// Type of content object on a PDF page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentObjectType {
    TextSpan,
    TextBlock,
    ImageXobject,
    Path,
    Unknown,
}

/// A content object extracted from a PDF page's content stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentObject {
    pub id: String,
    pub session_id: String,
    pub page_index: usize,
    pub object_type: ContentObjectType,
    pub bbox: [f32; 4], // [x0, y0, x1, y1]
    pub z_index: usize,
    pub editable_level: EditableLevel,
    pub text_info: Option<TextInfo>,
    pub image_info: Option<ImageInfo>,
    pub style_info: Option<StyleInfo>,
    pub diagnostics: Vec<String>,
}

/// Text-specific metadata for a content object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInfo {
    pub raw_text: String,
    pub decoded_text: String,
    pub glyph_count: usize,
    pub font_name: String,
    pub font_size: f32,
    pub fill_color: Option<[f32; 3]>,
    pub stroke_color: Option<[f32; 3]>,
    pub writing_mode: String,
    pub is_subset_font: bool,
    pub encoding_safe: bool,
    pub operator_offset: Option<usize>,
    pub operator_length: Option<usize>,
    /// Which occurrence of this exact text on the page (0-based).
    /// Used to disambiguate repeated text during native editing.
    pub occurrence_index: usize,
    /// Phase 28A — operator type (`tj`, `tj_array`, `single_quote`,
    /// `double_quote`, `hex`, `unknown`). Used by the UI to show what
    /// the engine actually saw and to gate native editing.
    #[serde(default = "default_operator_type")]
    pub operator_type: String,
    /// Phase 28A — which content stream this operator lives in (0 = single
    /// stream or first element of a Contents array). None when MuPDF
    /// extraction was used without raw stream parsing.
    #[serde(default)]
    pub content_stream_index: Option<usize>,
    /// Phase 28A — operator index within that stream (0-based, in the
    /// order found by the parser). Combined with content_stream_index
    /// and occurrence_index, this disambiguates repeated identical text.
    #[serde(default)]
    pub operator_index: Option<usize>,
    /// Phase 28A — granular editability classification used by the UI to
    /// pick a method (`native_in_place`, `native_rebuild_span`,
    /// `safe_visual_replacement`, `read_only`).
    #[serde(default = "default_editable_strategy")]
    pub editable_strategy: String,
    /// Phase 28A — human-readable list of reasons the engine cannot
    /// natively edit this text object. Empty when native editing is safe.
    #[serde(default)]
    pub unsupported_reason: Vec<String>,
    /// Phase 28C — encoding name detected for the font, if known
    /// (e.g. "WinAnsiEncoding", "Identity-H", "MacRomanEncoding").
    #[serde(default)]
    pub font_encoding: Option<String>,
    /// Honest assessment of how trustworthy `decoded_text` is. One of
    /// `"ok"`, `"partial"`, or `"garbled"`. When `"garbled"`, the
    /// frontend must NOT label this object with `decoded_text` and the
    /// edit engine must refuse a native text edit (text identity is
    /// unknown).
    #[serde(default = "default_decoding_quality")]
    pub decoding_quality: String,
}

fn default_decoding_quality() -> String {
    "ok".to_string()
}

fn default_operator_type() -> String {
    "unknown".to_string()
}
fn default_editable_strategy() -> String {
    "safe_visual_replacement".to_string()
}

/// Image-specific metadata for a content object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub xobject_name: String,
    pub width: u32,
    pub height: u32,
    pub color_space: String,
    pub bits_per_component: u32,
    pub transform_matrix: [f32; 6],
}

/// Style metadata for a content object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleInfo {
    pub opacity: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

/// Options for native text editing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTextEditRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub replacement_text: String,
    pub preserve_style: bool,
}

/// Phase 30D — post-edit verification status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Verification was not attempted (visual fallback, rejected edit, etc.).
    NotRun,
    /// Edit was verified: re-parsing the page after the write showed the
    /// new text and no longer showed the old text at the target location.
    Passed,
    /// Edit was attempted but post-write verification failed; the engine
    /// rolled back to the snapshot.
    Failed,
}

impl VerificationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationStatus::NotRun => "not_run",
            VerificationStatus::Passed => "passed",
            VerificationStatus::Failed => "failed",
        }
    }
}

/// Result of a native text edit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTextEditResult {
    pub edit_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub method: EditMethod,
    pub original_text: String,
    pub replacement_text: String,
    pub success: bool,
    pub warnings: Vec<String>,
    /// Phase 30D — post-edit verification status. Defaults to
    /// `NotRun` for older callers via `#[serde(default)]`.
    #[serde(default = "default_verification_status")]
    pub verification: VerificationStatus,
    #[serde(default)]
    pub verification_warnings: Vec<String>,
}

fn default_verification_status() -> VerificationStatus {
    VerificationStatus::NotRun
}

/// Method used for the edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditMethod {
    /// True in-place content stream operator editing (preserves original font).
    NativeInPlaceEdit,
    /// Phase 28D — multi-operator native edit covering an entire block of
    /// nearby text spans, applied in-place where every span uses a
    /// compatible font/encoding.
    NativeMultiOperator,
    /// Redaction + replacement text drawn with standard font (Helvetica).
    /// Visible result is correct but original font is not preserved.
    SafeVisualReplacement,
    /// XObject reference swap in resource dictionary.
    NativeXobjectSwap,
    /// Visual patch overlay only — no PDF content modification.
    VisualPatchFallback,
    /// Edit was rejected (unsafe or unsupported).
    Rejected,
}

/// Request to replace a native image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageReplaceRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub image_bytes_base64: String,
    /// Phase 35F — layout used when drawing the replacement into the target
    /// bbox. `stretch` fills exactly, `fit` preserves aspect with margins,
    /// and `fill` preserves aspect while clipping overflow.
    #[serde(default = "default_image_layout_mode")]
    pub layout_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindReplaceScope {
    CurrentPage,
    WholeDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplacePreviewRequest {
    pub session_id: String,
    pub current_page_index: usize,
    pub find_text: String,
    pub replace_text: String,
    pub scope: FindReplaceScope,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplaceMatchPreview {
    pub id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub text_preview: String,
    pub bbox: [f32; 4],
    pub editable_status: String,
    pub replacement_method: String,
    pub safe: bool,
    pub reason: Option<String>,
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplacePreviewResult {
    pub session_id: String,
    pub matches: Vec<FindReplaceMatchPreview>,
    pub unsafe_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplaceApplyRequest {
    pub session_id: String,
    pub replace_text: String,
    pub matches: Vec<FindReplaceApplyItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplaceApplyItem {
    pub id: String,
    pub page_index: usize,
    pub content_object_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplaceApplyResult {
    pub session_id: String,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub results: Vec<FindReplaceApplyItemResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindReplaceApplyItemResult {
    pub id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub success: bool,
    pub method: EditMethod,
    pub warning: Option<String>,
}

/// Request to delete a native image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageDeleteRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
}

/// Request to move/resize a native image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageMoveRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub new_rect: [f32; 4],
}

/// Request to crop a native image using export-safe visual redraw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageCropRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    /// Crop rectangle in page coordinates, constrained to the image bbox.
    pub crop_rect: [f32; 4],
    /// Destination rectangle for the cropped region. Defaults to the original
    /// image bbox when omitted.
    #[serde(default)]
    pub target_rect: Option<[f32; 4]>,
}

/// Request to rotate a native image using export-safe visual redraw.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageRotateRequest {
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    /// Supported export-safe increments: -90, 90, 180, 270.
    pub degrees: i32,
}

fn default_image_layout_mode() -> String {
    "stretch".to_string()
}

/// Result of a native image edit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeImageEditResult {
    pub edit_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub method: EditMethod,
    pub action: String,
    pub success: bool,
    pub warnings: Vec<String>,
}

// ─── Phase 28D: text block editing ──────────────────────────────────

/// A grouped block of nearby text spans on a page. Built by clustering
/// `ContentObject` text spans that share font + line spacing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub block_id: String,
    pub session_id: String,
    pub page_index: usize,
    /// Block bbox encompassing every member span [x0, y0, x1, y1].
    pub bbox: [f32; 4],
    /// IDs of every member content object.
    pub member_ids: Vec<String>,
    /// Joined block text (lines separated by `\n`).
    pub combined_text: String,
    /// Approximate font size (median of member spans).
    pub font_size: f32,
    /// Whether every member span can be natively edited (i.e. encoding-safe).
    pub all_native_editable: bool,
    /// Honest diagnostics describing block detection.
    pub diagnostics: Vec<String>,
}

/// Strategy hint for `pdf_apply_text_block_edit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockEditStrategy {
    /// Engine picks (preferring native_multi_operator when safe).
    Auto,
    /// Force multi-operator native rewrite. Falls back if any member is unsafe.
    NativeMultiOperator,
    /// Always use safe visual replacement: redact the block bbox and
    /// re-flow the new text inside the rect with Helvetica.
    VisualReflow,
}

/// Phase 35A — what to do when wrapped visual reflow text exceeds the
/// original block height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockOverflowPolicy {
    Reject,
    ShrinkToFit,
    AllowOverflow,
}

fn default_block_overflow_policy() -> BlockOverflowPolicy {
    BlockOverflowPolicy::Reject
}

/// Request to apply a block-level text edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockEditRequest {
    pub session_id: String,
    pub page_index: usize,
    pub block_id: String,
    pub replacement_text: String,
    pub strategy: BlockEditStrategy,
    #[serde(default = "default_block_overflow_policy")]
    pub overflow_policy: BlockOverflowPolicy,
}

/// Result of a block-level text edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlockEditResult {
    pub edit_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub block_id: String,
    pub method: EditMethod,
    pub edited_object_ids: Vec<String>,
    pub before_text: String,
    pub after_text: String,
    pub success: bool,
    pub warnings: Vec<String>,
}

/// A record of a content edit for history/undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentEditRecord {
    pub edit_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub content_object_id: String,
    pub edit_type: String,
    pub method: EditMethod,
    pub before_summary: String,
    pub after_summary: String,
    pub timestamp: u128,
    pub reversible: bool,
    pub warnings: Vec<String>,
    /// Snapshot of page content stream bytes before the edit (for undo).
    pub before_stream_snapshot: Option<Vec<u8>>,
}
