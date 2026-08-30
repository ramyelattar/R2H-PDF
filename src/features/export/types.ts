export type RedactionMode = "export_copy" | "apply_to_current_document";

export interface ExportOptions {
  sessionId: string;
  outputPath: string;
  commitAnnotations: boolean;
  flattenAnnotations: boolean;
  applyRedactions: boolean;
  redactionMode: RedactionMode;
  overwriteExisting: boolean;
}

export interface OverlayObjectPayload {
  id: string;
  object_type: string;
  page_index: number;
  rect: [number, number, number, number];
  text: string | null;
  color: string | null;
  font_size: number | null;
  author: string | null;
  stamp_text: string | null;
  stamp_type: string | null;
  opacity: number | null;
  status: string | null;
  reason: string | null;
  replacement_text: string | null;
  stroke_color: string | null;
  stroke_width: number | null;
  fill_color: string | null;
  /** Phase 24G: provenance tag — "compare" | "compare_visual" | "ai_review" | "signature" | "ocr" | "path_cover". */
  source?: string | null;
  /** Phase 26A: base64 data URL of the visible signature bitmap for `source = "signature"`. */
  image_data_url?: string | null;
  /** Phase 27A: when source = "signature", true → letterbox the image inside the rect; false → stretch. */
  preserve_aspect?: boolean | null;
  /** Phase 27A: original image pixel dimensions (used as a fallback when the backend cannot decode dimensions). */
  image_natural_width?: number | null;
  image_natural_height?: number | null;
}

export interface ExportResult {
  output_path: string;
  output_file_size_bytes: number;
  output_sha256: string;
  page_count: number;
  export_type: "pdf";
  independently_validated: boolean;
  overlay_objects_received_count: number;
  annotations_committed_count: number;
  redactions_applied_count: number;
  redaction_mode: string;
  source_document_mutated: boolean;
  true_flattening_supported: boolean;
  flattened: boolean;
  warnings: string[];
  created_at: number;
  duration_ms: number;
  /** Phase 24G additions — provenance subtotals from `OverlayObjectPayload.source`. */
  compare_annotations_count?: number;
  compare_visual_annotations_count?: number;
  ai_review_annotations_count?: number;
  redline_report_path?: string | null;
  // Phase 25F additions — per-overlay-kind subtotals
  text_boxes_count?: number;
  comments_count?: number;
  highlights_count?: number;
  shapes_count?: number;
  stamps_count?: number;
  signatures_count?: number;
  strikethroughs_count?: number;
  underlines_count?: number;
  images_count?: number;
  ocr_overlays_count?: number;
  skipped_unsupported_count?: number;
  skipped_items?: string[];
  // Phase 26A/F additions
  signatures_embedded_count?: number;
  signatures_failed_count?: number;
  signatures_failed_items?: string[];
  path_covers_count?: number;
  // Phase 27A — how many signature embeds preserved the source aspect ratio.
  signatures_aspect_preserved_count?: number;
  // Phase 27G — warnings about z-order export limits, etc.
  zorder_export_limited?: boolean;
  // Phase 27F — OCR searchable text layer status.
  ocr_text_layers_count?: number;
  // Phase 28G — text-edit counters derived from content-edit history.
  native_text_edits_count?: number;
  native_block_edits_count?: number;
  visual_text_replacements_count?: number;
  read_only_text_count?: number;
  font_preservation_count?: number;
  text_fallback_count?: number;
  image_edits_count?: number;
  image_replacements_count?: number;
  image_deletes_count?: number;
  paragraph_reflows_count?: number;
  find_replace_count?: number;
  image_crops_count?: number;
  image_rotations_count?: number;
  visual_covers_count?: number;
  sampled_background_covers_count?: number;
  white_fallback_covers_count?: number;
}

export type ExportStatus = "idle" | "running" | "completed" | "failed";
