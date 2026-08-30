import { invoke } from "@tauri-apps/api/core";
import type { LibrarySnapshot } from "../types/shell";
import type { OcrAvailability, OcrPageResult } from "../features/ocr/types";
export type { OcrAvailability } from "../features/ocr/types";

export interface ErrorEnvelope {
  code: string;
  message: string;
  cause?: string;
}

export type IpcResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: ErrorEnvelope };

const mapError = (error: unknown): ErrorEnvelope => {
  const message = error instanceof Error ? error.message : String(error);
  const lower = message.toLowerCase();
  const code = lower.includes("export_already_running") || lower.includes("export already running")
    ? "EXPORT_ALREADY_RUNNING"
    : lower.includes("output file already exists")
      ? "EXPORT_DESTINATION_EXISTS"
      : lower.includes("project_schema_unsupported")
        ? "PROJECT_SCHEMA_UNSUPPORTED"
        : lower.includes("project_corrupt") || lower.includes("project_integrity_mismatch")
          ? "PROJECT_CORRUPT"
          : lower.includes("project_write_failed")
            ? "PROJECT_WRITE_FAILED"
            : lower.includes("session not found")
    ? "SESSION_NOT_FOUND"
    : lower.includes("page out of range")
      ? "PAGE_OUT_OF_RANGE"
      : message.includes("INVALID_REGEX") || lower.includes("invalid_regex")
        ? "INVALID_REGEX"
        : message.includes("REGEX_TOO_COMPLEX") || lower.includes("regex_too_complex")
          ? "REGEX_TOO_COMPLEX"
      : lower.includes("undo snapshot unavailable")
        ? "SNAPSHOT_UNAVAILABLE"
        : lower.includes("snapshot storage failed")
          ? "SNAPSHOT_STORAGE_FAILED"
          : lower.includes("invalid")
            ? "INVALID_INPUT"
            : "IPC_ERROR";

  return {
    code,
    message,
  };
};

export const invokeSafe = async <T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<IpcResult<T>> => {
  try {
    const data = await invoke<T>(command, args);
    return { ok: true, data };
  } catch (error) {
    return { ok: false, error: mapError(error) };
  }
};

export interface LicenseStatus {
  mode: string;
  allowed: boolean;
  reason: string;
  trial_days: number;
  max_launches: number;
  license_path: string;
  trial_state_path: string;
}

export const getLicenseStatus = async (): Promise<IpcResult<LicenseStatus>> =>
  invokeSafe<LicenseStatus>("license_get_status");

export interface BBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PageInfo {
  index: number;
  width_points: number;
  height_points: number;
  rotation: number;
  has_text: boolean;
}

export interface FontInfo {
  name: string;
  embedded: boolean;
  subset: boolean;
}

export interface PdfObjectSummary {
  object_number: number;
  generation: number;
  byte_offset_hint: number;
  type_hint: string;
}

export interface DocumentSummary {
  page_count: number;
  object_count: number;
  title: string | null;
  author: string | null;
  producer: string | null;
}

export interface OpenDocumentRequest {
  path: string;
  recover_if_damaged: boolean;
}

export interface OpenDocumentResponse {
  session_id: string;
  source_path: string;
  recovered: boolean;
  summary: DocumentSummary;
  pages: PageInfo[];
  fonts: FontInfo[];
  objects: PdfObjectSummary[];
}

export interface RenderRequest {
  session_id: string;
  page_index: number;
  zoom: number;
  viewport: BBox;
  device_pixel_ratio: number;
}

export interface RenderResponse {
  session_id: string;
  page_index: number;
  width_px: number;
  height_px: number;
  width: number;
  height: number;
  zoom: number;
  cache_hit: boolean;
  /** Base64-encoded RGBA pixel data (4 bytes per pixel). */
  pixels_rgba: string;
  render_time_ms: number;
  /** Page width in PDF points (zoom-independent). Used by the canvas
   *  hit-test overlay and inline text editor to position boxes correctly
   *  regardless of zoom level. Optional for backward compatibility with
   *  older payloads. */
  width_pts?: number;
  /** Page height in PDF points (zoom-independent). */
  height_pts?: number;
}

export interface TextSpan {
  content: string;
  bbox: BBox;
  font_name: string | null;
}

export interface TextExtractionRequest {
  session_id: string;
  page_index: number;
  region: BBox | null;
}

export interface TextExtractionResponse {
  session_id: string;
  page_index: number;
  full_text: string;
  spans: TextSpan[];
}

export interface NavigateRequest {
  session_id: string;
  page_index: number;
  zoom: number;
  viewport: BBox;
}

export interface NavigateResponse {
  session_id: string;
  active_page: number;
  prefetch_pages: number[];
}

export interface IncrementalSaveRequest {
  session_id: string;
  target_path: string | null;
  fsync: boolean;
}

export interface IncrementalSaveResponse {
  session_id: string;
  saved_to: string;
  bytes_written: number;
  integrity_hash: string;
}

export interface SessionDiagnostics {
  session_id: string;
  active_sessions: number;
  parsed_document_available: boolean;
  parsed_document_open_count: number;
  document_hash: string;
  document_revision: number;
  page_cache_entries: number;
  page_cache_bytes: number;
  render_cache_hit_count: number;
  render_cache_miss_count: number;
  opened_at_epoch_ms: number;
  total_renders: number;
  total_text_extractions: number;
}

export interface SessionStateResponse {
  session_id: string;
  file_path: string;
  source_path: string;
  title: string;
  current_page: number;
  active_page: number;
  page_count: number;
  total_pages: number;
  zoom: number;
  rotation: number;
  is_dirty: boolean;
  dirty: boolean;
  is_scanned: boolean;
  permissions: SessionPermissions;
  last_saved_at: number | null;
  viewport: ViewportState;
  load_state: string;
}

export interface SessionPermissions {
  can_print: boolean;
  can_copy: boolean;
  can_edit: boolean;
  can_annotate: boolean;
}

export interface ViewportState {
  page_index: number;
  zoom: number;
  scroll_x: number;
  scroll_y: number;
  fit_mode: string;
}

export interface RecoveryReport {
  recovered: boolean;
  method: string;
  warnings: string[];
  repaired_bytes: number;
}

export const docOpen = (request: OpenDocumentRequest) =>
  invokeSafe<OpenDocumentResponse>("doc_open", { request });

export const openPdf = (path: string) =>
  invokeSafe<OpenDocumentResponse>("open_pdf", { path });

export const docRender = (request: RenderRequest) =>
  invokeSafe<RenderResponse>("doc_render", { request });

export const renderPage = (sessionId: string, pageIndex: number, zoom: number) =>
  invokeSafe<RenderResponse>("render_page", {
    sessionId,
    pageIndex,
    zoom,
  });

export const docExtractText = (request: TextExtractionRequest) =>
  invokeSafe<TextExtractionResponse>("doc_extract_text", { request });

/**
 * Extract the full text of every page in one IPC round-trip.
 * Returns an array where index `i` contains the text of page `i`.
 * Use this instead of N individual `docExtractText` calls when building
 * a search index.
 */
export const docExtractAllText = (sessionId: string) =>
  invokeSafe<string[]>("doc_extract_all_text", { sessionId });

export const docNavigate = (request: NavigateRequest) =>
  invokeSafe<NavigateResponse>("doc_navigate", { request });

export const goToPage = (sessionId: string, pageIndex: number) =>
  invokeSafe<SessionStateResponse>("go_to_page", {
    sessionId,
    pageIndex,
  });

export const setZoom = (sessionId: string, zoom: number) =>
  invokeSafe<SessionStateResponse>("set_zoom", {
    sessionId,
    zoom,
  });

export const getSessionState = (sessionId: string) =>
  invokeSafe<SessionStateResponse>("get_session_state", {
    sessionId,
  });

export const docIncrementalSave = (request: IncrementalSaveRequest) =>
  invokeSafe<IncrementalSaveResponse>("doc_incremental_save", { request });

export interface ProjectSaveRequest {
  source_path: string;
  project_name: string;
  workspace_id?: string | null;
  page_count: number;
  overlay_objects: unknown[];
  review_state?: unknown | null;
  view_state?: unknown | null;
}

export interface PersistedProjectDocument {
  document_id: string;
  original_source_path: string;
  current_saved_pdf_path: string;
  source_hash_sha256: string | null;
  saved_hash_sha256: string | null;
  page_count: number;
  overlay_objects: unknown[];
  review_state: unknown | null;
  view_state: unknown | null;
  last_export: unknown | null;
}

export interface PersistedProject {
  schema_version: number;
  migration_version: number;
  project_id: string;
  project_name: string;
  workspace_id: string | null;
  created_at_epoch_ms: number;
  last_modified_at_epoch_ms: number;
  documents: PersistedProjectDocument[];
  integrity_sha256: string;
}

export interface ProjectLoadResponse {
  project: PersistedProject | null;
  project_id: string;
  document_id: string;
  sidecar_path: string;
  migrated: boolean;
}

export interface ProjectSaveResponse {
  status: "project_saved";
  project_id: string;
  document_id: string;
  sidecar_path: string;
  integrity_sha256: string;
  saved_at_epoch_ms: number;
}

export const projectLoad = (sourcePath: string) =>
  invokeSafe<ProjectLoadResponse>("project_load", {
    request: { source_path: sourcePath },
  });

export const projectSave = (request: ProjectSaveRequest) =>
  invokeSafe<ProjectSaveResponse>("project_save", { request });

export interface LibraryRecordOpenRequest {
  source_path: string;
  workspace_id?: string | null;
  page_count: number;
}

export interface LibraryRecordProjectRequest {
  source_path: string;
  project_name: string;
  workspace_id?: string | null;
  page_count: number;
}

export interface LibraryRecordExportRequest {
  source_path: string;
  destination: string;
  export_type: "pdf";
  timestamp_epoch_ms: number;
  size_bytes: number;
  sha256: string;
  page_count: number;
  included_overlays: number;
  warnings: string[];
}

export interface LibraryRecordReviewRequest {
  source_path: string;
  review_id: string;
  status: "in-review";
}

/**
 * The shell library is a bounded, versioned registry stored by the Tauri
 * backend under the existing application-data root.  It is deliberately
 * separate from renderer session convenience state and never contains PDF
 * bytes or OCR/AI content.
 */
export const libraryLoad = () =>
  invokeSafe<LibrarySnapshot>("library_load");

export const libraryRecordOpen = (request: LibraryRecordOpenRequest) =>
  invokeSafe<LibrarySnapshot>("library_record_open", { request });

export const libraryRecordProject = (request: LibraryRecordProjectRequest) =>
  invokeSafe<LibrarySnapshot>("library_record_project", { request });

export const libraryRecordExport = (request: LibraryRecordExportRequest) =>
  invokeSafe<LibrarySnapshot>("library_record_export", { request });

export const libraryRecordReview = (request: LibraryRecordReviewRequest) =>
  invokeSafe<LibrarySnapshot>("library_record_review", { request });

export const docRecover = (path: string) =>
  invokeSafe<RecoveryReport>("doc_recover", { path });

export const docDiagnostics = (sessionId: string) =>
  invokeSafe<SessionDiagnostics>("doc_diagnostics", { sessionId });

export const docClose = (sessionId: string) =>
  invokeSafe<void>("doc_close", { sessionId });

export const closeSession = (sessionId: string) =>
  invokeSafe<void>("close_session", { sessionId });

export interface SpanRecord {
  char_start: number;
  char_end: number;
  bbox: BBox;
}

export interface PageTextWithSpans {
  text: string;
  spans: SpanRecord[];
}

export type SearchScope = "CurrentPage" | "AllPages" | "Selection";

export interface SearchQuery {
  session_id: string;
  query: string;
  scope: SearchScope;
  case_sensitive: boolean;
  whole_words: boolean;
  use_regex: boolean;
  max_results: number | null;
}

export interface SearchBBox {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface SearchMatch {
  page_index: number;
  match_index: number;
  snippet: string;
  bbox: SearchBBox;
}

export interface SearchResponse {
  session_id: string;
  query: string;
  matches: SearchMatch[];
  total: number;
  truncated: boolean;
}

export interface IndexStatus {
  session_id: string;
  indexed_pages: number;
  total_pages: number;
  is_ready: boolean;
  last_updated: string | null;
}

export const searchQuery = (query: SearchQuery) =>
  invokeSafe<SearchResponse>("search_query", { query });

export const searchIndexDocument = (sessionId: string, pages: string[]) =>
  invokeSafe<IndexStatus>("search_index_document", {
    sessionId,
    pages,
  });

export const searchIndexDocumentWithSpans = (
  sessionId: string,
  pages: PageTextWithSpans[],
): Promise<IpcResult<IndexStatus>> =>
  invokeSafe<IndexStatus>("search_index_document_with_spans", {
    sessionId,
    pages,
  });

export const searchSetActivePage = (
  sessionId: string,
  pageIndex: number,
): Promise<IpcResult<void>> =>
  invokeSafe<void>("search_set_active_page", {
    sessionId,
    pageIndex,
  });

export const searchGetIndexStatus = (sessionId: string) =>
  invokeSafe<IndexStatus>("search_get_index_status", {
    sessionId,
  });

export const searchClearIndex = (sessionId: string) =>
  invokeSafe<void>("search_clear_index", { sessionId });

export interface AnnotationColor {
  r: number;
  g: number;
  b: number;
  a: number;
}

export type AnnotationType =
  | "Highlight"
  | "Underline"
  | "Strikeout"
  | "Squiggly"
  | "FreeText"
  | "Stamp"
  | "Ink"
  | "Link"
  | "Note"
  | "FileAttachment";

export interface Annotation {
  id: string;
  session_id: string;
  page_index: number;
  annot_type: AnnotationType;
  color: AnnotationColor;
  contents: string;
  author: string;
  rect: [number, number, number, number];
  created_at: string;
  modified_at: string;
}

export interface CreateAnnotationRequest {
  session_id: string;
  source_path: string | null;
  page_index: number;
  annot_type: AnnotationType;
  color: AnnotationColor;
  contents: string;
  author: string;
  rect: [number, number, number, number];
}

export interface UpdateAnnotationRequest {
  session_id: string;
  source_path: string | null;
  annotation_id: string;
  color: AnnotationColor | null;
  contents: string | null;
  rect: [number, number, number, number] | null;
}

export interface AnnotationListResponse {
  session_id: string;
  page_index: number | null;
  annotations: Annotation[];
  total: number;
}

export const annotCreate = (request: CreateAnnotationRequest) =>
  invokeSafe<Annotation>("annot_create", { request });

export const annotUpdate = (request: UpdateAnnotationRequest) =>
  invokeSafe<Annotation>("annot_update", { request });

export const annotDelete = (sessionId: string, annotationId: string) =>
  invokeSafe<void>("annot_delete", {
    sessionId,
    annotationId,
  });

export const annotList = (sessionId: string, pageIndex: number | null) =>
  invokeSafe<AnnotationListResponse>("annot_list", {
    sessionId,
    pageIndex,
  });

export const annotExportFdf = (sessionId: string) =>
  invokeSafe<string>("annot_export_fdf", { sessionId });

// ── Editing / Undo-Redo ─────────────────────────────────────────────────────

export interface EditOperation {
  id: string;
  op_type: string;
  session_id: string;
  page_index: number;
  object_ref: string | null;
  payload_json: string;
}

export interface EditTransaction {
  transaction_id: string;
  session_id: string;
  operations: EditOperation[];
  description: string;
}

export interface EditTransactionResult {
  transaction_id: string;
  success: boolean;
  applied_count: number;
  error: string | null;
}

export interface UndoRedoState {
  session_id: string;
  undo_depth: number;
  redo_depth: number;
  last_description: string | null;
  undo_entry_count: number;
  redo_entry_count: number;
  in_memory_undo_bytes: number;
  disk_backed_undo_bytes: number;
  pruned_entry_count: number;
  latest_snapshot_tier: "memory" | "disk" | null;
  memory_budget: number;
  disk_budget: number;
  undo_degraded: boolean;
}

export const editApplyTransaction = (transaction: EditTransaction) =>
  invokeSafe<EditTransactionResult>("edit_apply_transaction", { transaction });

export const editUndo = (sessionId: string) =>
  invokeSafe<EditTransactionResult>("edit_undo", { sessionId });

export const editRedo = (sessionId: string) =>
  invokeSafe<EditTransactionResult>("edit_redo", { sessionId });

export const editGetUndoRedoState = (sessionId: string) =>
  invokeSafe<UndoRedoState>("edit_get_undo_redo_state", { sessionId });

// ── AI Core ─────────────────────────────────────────────────────────────────

export interface PageRange {
  start: number;
  end: number;
}

export interface AiAnnotationSuggestion {
  page_index: number;
  text_snippet: string;
  suggested_note: string;
}

export interface AiEntity {
  entity_type: string;
  value: string;
  page_index: number;
  snippet: string;
}

export type AiModelBackend = "unavailable" | "local_ollama";

export interface AiStatusResponse {
  available: boolean;
  backend: AiModelBackend;
  models: string[];
  active_tasks: number;
  error: string | null;
}

export type AiTaskType =
  | "summarize"
  | "extract_key_points"
  | "translate_text"
  | "answer_question"
  | "classify_document"
  | "question_answer"
  | "suggest_annotations"
  | "extract_entities";

export interface AiTaskRequest {
  task_id: string;
  session_id: string;
  task_type: AiTaskType;
  model: string;
  question?: string;
  page_range?: PageRange;
  input_text: string;
  parameters_json?: string;
}

export interface AiTaskResult {
  task_id: string;
  success: boolean;
  output_text: string | null;
  suggestions: AiAnnotationSuggestion[] | null;
  entities: AiEntity[] | null;
  model_used: string | null;
  error: string | null;
}

export const aiGetStatus = () =>
  invokeSafe<AiStatusResponse>("ai_get_status", {});

export const aiRunTask = (request: AiTaskRequest) =>
  invokeSafe<AiTaskResult>("ai_run_task", { request });

export const aiCancelTask = (taskId: string) =>
  invokeSafe<void>("ai_cancel_task", { taskId });


// ── Content Editing ─────────────────────────────────────────────────────────

export interface ContentObject {
  id: string;
  session_id: string;
  page_index: number;
  object_type: "text_span" | "text_block" | "image_xobject" | "path" | "unknown";
  bbox: [number, number, number, number];
  z_index: number;
  editable_level: "native_editable" | "native_replaceable" | "visual_patch_only" | "read_only";
  text_info: TextContentInfo | null;
  image_info: ImageContentInfo | null;
  style_info: StyleContentInfo | null;
  diagnostics: string[];
}

export interface TextContentInfo {
  raw_text: string;
  decoded_text: string;
  glyph_count: number;
  font_name: string;
  font_size: number;
  fill_color: [number, number, number] | null;
  stroke_color: [number, number, number] | null;
  writing_mode: string;
  is_subset_font: boolean;
  encoding_safe: boolean;
  operator_offset: number | null;
  operator_length: number | null;
  occurrence_index: number;
  /** Phase 28A — `tj`, `tj_array`, `single_quote`, `double_quote`, `hex`, `unknown`. */
  operator_type?: string;
  content_stream_index?: number | null;
  operator_index?: number | null;
  /** Phase 28A — `native_in_place`, `native_rebuild_span`, `safe_visual_replacement`, `read_only`. */
  editable_strategy?: string;
  unsupported_reason?: string[];
  font_encoding?: string | null;
  /** Honest assessment of how trustworthy `decoded_text` is. One of
   *  `"ok"`, `"partial"`, `"garbled"`. When `"garbled"`, the UI MUST
   *  NOT label this object with `decoded_text` (it's noise from a
   *  failed ToUnicode mapping) and native edits are refused upstream. */
  decoding_quality?: "ok" | "partial" | "garbled";
}

export interface ImageContentInfo {
  xobject_name: string;
  width: number;
  height: number;
  color_space: string;
  bits_per_component: number;
  transform_matrix: [number, number, number, number, number, number];
}

export interface StyleContentInfo {
  opacity: number;
  rotation: number;
  scale_x: number;
  scale_y: number;
}

export type EditMethodValue =
  | "native_in_place_edit"
  | "native_multi_operator"
  | "safe_visual_replacement"
  | "native_xobject_swap"
  | "visual_patch_fallback"
  | "rejected";

export type VerificationStatusValue = "not_run" | "passed" | "failed";

export interface NativeTextEditResult {
  edit_id: string;
  session_id: string;
  page_index: number;
  content_object_id: string;
  method: EditMethodValue;
  original_text: string;
  replacement_text: string;
  success: boolean;
  warnings: string[];
  /** Phase 30D — post-edit verification status. */
  verification?: VerificationStatusValue;
  verification_warnings?: string[];
}

export type FindReplaceScope = "current_page" | "whole_document";

export interface FindReplacePreviewRequest {
  session_id: string;
  current_page_index: number;
  find_text: string;
  replace_text: string;
  scope: FindReplaceScope;
  case_sensitive: boolean;
  whole_word: boolean;
}

export interface FindReplaceMatchPreview {
  id: string;
  page_index: number;
  content_object_id: string;
  text_preview: string;
  bbox: [number, number, number, number];
  editable_status: string;
  replacement_method: string;
  safe: boolean;
  reason?: string | null;
  checked: boolean;
}

export interface FindReplacePreviewResult {
  session_id: string;
  matches: FindReplaceMatchPreview[];
  unsafe_count: number;
  warnings: string[];
}

export interface FindReplaceApplyResult {
  session_id: string;
  applied_count: number;
  skipped_count: number;
  failed_count: number;
  results: Array<{
    id: string;
    page_index: number;
    content_object_id: string;
    success: boolean;
    method: EditMethodValue;
    warning?: string | null;
  }>;
  warnings: string[];
}

export const pdfPreviewFindReplace = (request: FindReplacePreviewRequest) =>
  invokeSafe<FindReplacePreviewResult>("pdf_preview_find_replace", { request });

export const pdfApplyFindReplace = (request: {
  session_id: string;
  replace_text: string;
  matches: Array<{ id: string; page_index: number; content_object_id: string }>;
}) =>
  invokeSafe<FindReplaceApplyResult>("pdf_apply_find_replace", { request });

// ── Phase 28D: text block editing ─────────────────────────────────────

export interface TextBlock {
  block_id: string;
  session_id: string;
  page_index: number;
  bbox: [number, number, number, number];
  member_ids: string[];
  combined_text: string;
  font_size: number;
  all_native_editable: boolean;
  diagnostics: string[];
}

export type BlockEditStrategy = "auto" | "native_multi_operator" | "visual_reflow";
export type BlockOverflowPolicy = "reject" | "shrink_to_fit" | "allow_overflow";

export interface TextBlockEditRequest {
  session_id: string;
  page_index: number;
  block_id: string;
  replacement_text: string;
  strategy: BlockEditStrategy;
  overflow_policy?: BlockOverflowPolicy;
}

export interface TextBlockEditResult {
  edit_id: string;
  session_id: string;
  page_index: number;
  block_id: string;
  method: EditMethodValue;
  edited_object_ids: string[];
  before_text: string;
  after_text: string;
  success: boolean;
  warnings: string[];
}

export const pdfPrepareTextBlockEdit = (sessionId: string, pageIndex: number) =>
  invokeSafe<TextBlock[]>("pdf_prepare_text_block_edit", { sessionId, pageIndex });

export const pdfApplyTextBlockEdit = (request: TextBlockEditRequest) =>
  invokeSafe<TextBlockEditResult>("pdf_apply_text_block_edit", { request });

// ── Phase 29A: page font registry ─────────────────────────────────────

export type FontEncodingKind =
  | "win_ansi"
  | "mac_roman"
  | "identity_h"
  | "identity_v"
  | "custom_differences"
  | "to_unicode"
  | "unknown";

export interface FontResourceInfo {
  resource_name: string;
  base_font_name: string;
  subtype: string;
  encoding_kind: FontEncodingKind;
  base_encoding: string | null;
  differences_count: number;
  has_to_unicode: boolean;
  is_subset: boolean;
  is_embedded: boolean;
  is_cid_font: boolean;
  is_type0: boolean;
  is_true_type: boolean;
  is_type1: boolean;
  is_type3: boolean;
  can_native_edit_ascii: boolean;
  can_native_edit_latin1: boolean;
  can_native_edit_arabic: boolean;
  can_native_edit_cjk: boolean;
  unsupported_reasons: string[];
}

export interface PageFontRegistry {
  session_id: string;
  page_index: number;
  fonts: FontResourceInfo[];
  warnings: string[];
}

export const pdfGetPageFontRegistry = (sessionId: string, pageIndex: number) =>
  invokeSafe<PageFontRegistry>("pdf_get_page_font_registry", { sessionId, pageIndex });

// ── Phase 31B: page rotation metadata ─────────────────────────────────

export interface PageRotationInfo {
  session_id: string;
  page_index: number;
  rotation_degrees: number;
  width_pts: number;
  height_pts: number;
}

export const pdfGetPageRotation = (sessionId: string, pageIndex: number) =>
  invokeSafe<PageRotationInfo>("pdf_get_page_rotation", { sessionId, pageIndex });

export interface NativeImageEditResult {
  edit_id: string;
  session_id: string;
  page_index: number;
  content_object_id: string;
  method: EditMethodValue;
  action: string;
  success: boolean;
  warnings: string[];
}

export interface ContentEditRecord {
  edit_id: string;
  session_id: string;
  page_index: number;
  content_object_id: string;
  edit_type: string;
  method: string;
  before_summary: string;
  after_summary: string;
  timestamp: number;
  reversible: boolean;
  warnings: string[];
}

export const pdfGetPageContentObjects = (sessionId: string, pageIndex: number) =>
  invokeSafe<ContentObject[]>("pdf_get_page_content_objects", { sessionId, pageIndex });

export const pdfApplyNativeTextEdit = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
  replacement_text: string;
  preserve_style: boolean;
}) =>
  invokeSafe<NativeTextEditResult>("pdf_apply_native_text_edit", { request });

export const pdfReplaceNativeImage = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
  image_bytes_base64: string;
  layout_mode?: "stretch" | "fit" | "fill";
}) =>
  invokeSafe<NativeImageEditResult>("pdf_replace_native_image", { request });

export const pdfDeleteNativeImage = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
}) =>
  invokeSafe<NativeImageEditResult>("pdf_delete_native_image", { request });

export const pdfMoveNativeImage = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
  new_rect: [number, number, number, number];
}) =>
  invokeSafe<NativeImageEditResult>("pdf_move_native_image", { request });

export const pdfCropNativeImage = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
  crop_rect: [number, number, number, number];
  target_rect?: [number, number, number, number] | null;
}) =>
  invokeSafe<NativeImageEditResult>("pdf_crop_native_image", { request });

export const pdfRotateNativeImage = (request: {
  session_id: string;
  page_index: number;
  content_object_id: string;
  degrees: -90 | 90 | 180 | 270;
}) =>
  invokeSafe<NativeImageEditResult>("pdf_rotate_native_image", { request });

export const pdfListContentEdits = (sessionId: string) =>
  invokeSafe<ContentEditRecord[]>("pdf_list_content_edits", { sessionId });


// ── Form Fields ─────────────────────────────────────────────────────────────

export interface FormField {
  name: string;
  field_type: string;
  value: string;
  page_index: number;
  rect: [number, number, number, number];
}

export const docListFormFields = (sessionId: string) =>
  invokeSafe<FormField[]>("doc_list_form_fields", { sessionId });

// ── Content Edit Undo ───────────────────────────────────────────────────────

export const pdfRevertContentEdit = (sessionId: string, editId: string) =>
  invokeSafe<void>("pdf_revert_content_edit", { sessionId, editId });

export const pdfClearContentEditHistory = (sessionId: string) =>
  invokeSafe<void>("pdf_clear_content_edit_history", { sessionId });

// ── Phase 26D: vector/path safe visual removal ─────────────────────────────

export interface CoverPathRequest {
  session_id: string;
  page_index: number;
  content_object_id: string;
}

export interface CoverPathSpec {
  id: string;
  page_index: number;
  bbox: [number, number, number, number];
  method: string;
  diagnostics: string[];
}

export interface CoverPathResult {
  success: boolean;
  spec: CoverPathSpec | null;
  warnings: string[];
  method_label: string;
}

export const pdfCoverPathObject = (request: CoverPathRequest) =>
  invokeSafe<CoverPathResult>("pdf_cover_path_object", { request });

// ── Phase 23A: Form-field creation ──────────────────────────────────────────

export type CreateFieldType =
  | "text"
  | "checkbox"
  | "radio"
  | "combo"
  | "list"
  | "signature";

export interface CreateFormFieldRequest {
  session_id: string;
  page_index: number;
  field_type: CreateFieldType;
  name: string;
  value?: string | null;
  default_value?: string | null;
  options?: string[] | null;
  rect: [number, number, number, number];
  required?: boolean | null;
  read_only?: boolean | null;
  font_size?: number | null;
  border_color?: [number, number, number] | null;
  fill_color?: [number, number, number] | null;
}

export interface DeleteFormFieldRequest {
  session_id: string;
  field_name: string;
}

export interface UpdateFormFieldPropertiesRequest {
  session_id: string;
  field_name: string;
  value?: string | null;
  required?: boolean | null;
  read_only?: boolean | null;
  font_size?: number | null;
}

export interface FormFieldOperationResult {
  success: boolean;
  field_name: string;
  page_index: number;
  action: string;
  warnings: string[];
}

export const pdfCreateFormField = (request: CreateFormFieldRequest) =>
  invokeSafe<FormFieldOperationResult>("pdf_create_form_field", { request });

export const pdfDeleteFormField = (request: DeleteFormFieldRequest) =>
  invokeSafe<FormFieldOperationResult>("pdf_delete_form_field", { request });

export const pdfUpdateFormFieldProperties = (request: UpdateFormFieldPropertiesRequest) =>
  invokeSafe<FormFieldOperationResult>("pdf_update_form_field_properties", { request });

// ── Phase 23E: Compare documents ────────────────────────────────────────────

export type CompareMode = "text_only" | "page_visual" | "combined";
export type CompareChangeType = "added" | "removed" | "modified" | "visual_modified";

export interface CompareDocumentsRequest {
  base_session_id: string;
  revised_session_id?: string | null;
  revised_file_path?: string | null;
  mode?: CompareMode;
  max_pages?: number | null;
  /** Phase 24E — default true on the backend. */
  include_ocr?: boolean;
  /** Phase 24B — cap pages submitted to visual diff. */
  visual_max_pages?: number | null;
  /** Phase 24B — render DPI for visual diff (default 72). */
  visual_dpi?: number | null;
  /** Phase 25A / 26C — run OCR on scanned pages before comparing. */
  auto_ocr_scanned?: boolean;
  /** Phase 25A — cap on auto-OCR pages per compare call. */
  auto_ocr_max_pages?: number | null;
}

export interface CompareDocumentInfo {
  session_id: string | null;
  source_path: string | null;
  page_count: number;
}

export interface CompareSummary {
  pages_compared: number;
  pages_with_changes: number;
  lines_added: number;
  lines_removed: number;
  lines_modified: number;
  identical: boolean;
}

export interface CompareTextChange {
  page_index: number;
  change_type: CompareChangeType;
  old_text: string | null;
  new_text: string | null;
  bbox: [number, number, number, number] | null;
  confidence: number;
  citation: string;
}

export interface CompareVisualChange {
  page_index: number;
  change_type: CompareChangeType;
  bbox: [number, number, number, number];
  confidence: number;
}

export interface ComparePageResult {
  page_index: number;
  base_line_count: number;
  revised_line_count: number;
  added: number;
  removed: number;
  modified: number;
  /** Phase 24E — `native_text` | `ocr_text` | `mixed` | `none`. */
  text_source?: string;
  /** Phase 24B — number of visual change regions on this page. */
  visual_regions?: number;
}

export interface CompareResult {
  compare_id: string;
  base_document: CompareDocumentInfo;
  revised_document: CompareDocumentInfo;
  mode: CompareMode;
  summary: CompareSummary;
  page_results: ComparePageResult[];
  text_changes: CompareTextChange[];
  visual_changes: CompareVisualChange[];
  warnings: string[];
  created_at_ms: number;
}

export const pdfCompareDocuments = (request: CompareDocumentsRequest) =>
  invokeSafe<CompareResult>("pdf_compare_documents", { request });

export const pdfGetCompareResult = (compareId: string) =>
  invokeSafe<CompareResult | null>("pdf_get_compare_result", { compareId });

export const pdfClearCompareResult = (compareId: string) =>
  invokeSafe<boolean>("pdf_clear_compare_result", { compareId });

// ── Phase 24C: Compare HTML redline export ─────────────────────────────────

export interface ExportCompareReportRequest {
  compare_id: string;
  output_path: string;
  overwrite_existing?: boolean | null;
}

export interface ExportCompareReportResult {
  compare_id: string;
  output_path: string;
  bytes_written: number;
  text_changes_count: number;
  visual_changes_count: number;
  pages_compared: number;
  warnings_count: number;
}

export const reportExportCompareReview = (request: ExportCompareReportRequest) =>
  invokeSafe<ExportCompareReportResult>("report_export_compare_review", { request });

// ── Phase 24D: AI compare review ───────────────────────────────────────────

export interface AiReviewCompareRequest {
  compare_id: string;
  session_id?: string | null;
}

export interface ReviewCitation {
  citation_id: string;
  page_index: number;
  chunk_id: string;
  snippet: string;
  score: number;
  source: string;
}

export interface ReviewFinding {
  finding_id: string;
  title: string;
  description: string;
  severity: string;
  category: string;
  page_refs: number[];
  citations: ReviewCitation[];
  recommendation: string;
  confidence: number;
}

export interface ReviewSuggestedAction {
  action_id: string;
  action_type: string;
  page_index: number;
  text: string;
  reason: string;
  citations: ReviewCitation[];
  confidence: number;
}

export interface DocumentReviewResult {
  review_id: string;
  session_id: string;
  summary: string;
  findings: ReviewFinding[];
  risks: ReviewFinding[];
  missing_information: ReviewFinding[];
  engineering_findings: ReviewFinding[];
  suggested_actions: ReviewSuggestedAction[];
  citations: ReviewCitation[];
  warnings: string[];
  elapsed_ms: number;
  created_at: number;
}

export const aiReviewCompareResult = (request: AiReviewCompareRequest) =>
  invokeSafe<DocumentReviewResult>("ai_review_compare_result", { request });

// ── Phase 25D: OCR editable overlay / searchable layer ─────────────────────

export interface OcrPageRequest {
  operation_id: string;
  session_id: string;
  page_index: number;
  force: boolean;
  dpi?: number | null;
  language: string;
  model_id: string;
  output_format_version: number;
  timeout_secs?: number | null;
  cancellation_id?: string | null;
}

export interface OcrCancelResponse {
  operation_id: string;
  cancelled: boolean;
}

export const ocrCheckAvailability = () =>
  invokeSafe<OcrAvailability>("ocr_check_availability");

export const ocrGetStatus = () =>
  invokeSafe<OcrAvailability>("ocr_get_status");

export const ocrRunPage = (request: OcrPageRequest) =>
  invokeSafe<OcrPageResult>("ocr_run_page", { request });

export const ocrCancel = (operation_id: string) =>
  invokeSafe<OcrCancelResponse>("ocr_cancel", { request: { operation_id } });

export const ocrHasResult = (session_id: string, page_index: number) =>
  invokeSafe<boolean>("ocr_has_result", { session_id, page_index });

export const ocrGetPageText = (session_id: string, page_index: number) =>
  invokeSafe<string | null>("ocr_get_page_text", { session_id, page_index });

export const ocrGetAllTexts = (session_id: string) =>
  invokeSafe<Array<[number, string]>>("ocr_get_all_texts", { session_id });

export interface CreateOcrOverlaysRequest {
  session_id: string;
  page_index: number;
  min_confidence?: number | null;
}

export interface OcrOverlaySpec {
  id: string;
  page_index: number;
  text: string;
  bbox: [number, number, number, number];
  confidence: number | null;
  block_type: string;
  block_id?: string;
  /** Phase 26E: untransformed worker bbox in source coord space. */
  original_bbox?: [number, number, number, number];
  /** `"image_px"` | `"pdf_points"` | `"unknown_assumed_pdf_points"` */
  coordinate_space?: string;
  /** Phase 26E: [x_scale, y_scale] used to map image px → PDF points. */
  conversion_scale?: [number, number];
}

export interface OcrOverlayResult {
  document_id: string;
  session_id: string;
  page_index: number;
  status: "completed" | "no_text_detected" | string;
  overlays: OcrOverlaySpec[];
  warnings: string[];
}

export interface OcrTextLayerStatus {
  session_id: string;
  page_index: number;
  status: string;
  overlays_available: boolean;
  note: string;
  warnings: string[];
}

export const pdfCreateOcrEditableOverlays = (request: CreateOcrOverlaysRequest) =>
  invokeSafe<OcrOverlayResult>("pdf_create_ocr_editable_overlays", { request });

export const pdfCreateOcrTextLayer = (request: CreateOcrOverlaysRequest) =>
  invokeSafe<OcrTextLayerStatus>("pdf_create_ocr_text_layer", { request });
