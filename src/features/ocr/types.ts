export type OcrAvailabilityStatus =
  | "available"
  | "missing_worker"
  | "missing_python_runtime"
  | "missing_model"
  | "missing_processor"
  | "invalid_model_layout"
  | "unsupported_platform"
  | "resource_resolution_failed";

export interface OcrAvailability {
  status: OcrAvailabilityStatus;
  available: boolean;
  message: string;
  resource_source: string;
  model_id: string;
  missing_assets: string[];
}

export interface OcrBBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface OcrBlock {
  id: string;
  text: string;
  bbox: OcrBBox;
  confidence: number | null;
  block_type: string;
  reading_order: number | null;
}

export interface OcrPageResult {
  schema_version: number;
  operation_id: string;
  document_id: string;
  session_id: string;
  page_index: number;
  page_width_points: number;
  page_height_points: number;
  rotation_degrees: number;
  text: string;
  blocks: OcrBlock[];
  confidence: number | null;
  language: string;
  language_source: "requested" | "detected" | string;
  engine: string;
  worker_version: string | null;
  model_version: string | null;
  duration_ms: number;
  warnings: string[];
  status: "completed" | "no_text_detected";
  created_at: number;
  bbox_coordinate_space: "image_px" | "pdf_points" | string;
  image_width_px: number;
  image_height_px: number;
}

export type OcrJobStatus =
  | "idle"
  | "validating"
  | "blocked"
  | "running"
  | "completed"
  | "no_text_detected"
  | "cancelled"
  | "failed"
  | "partial_failure";

export type OcrAcceptanceStatus = "none" | "proposal" | "persisted" | "failed";

export interface OcrState {
  status: OcrJobStatus;
  currentPage: number | null;
  totalPages: number;
  progressCurrent: number;
  progressTotal: number;
  lastResult: OcrPageResult | null;
  error: string | null;
  available: boolean;
  availability: OcrAvailability | null;
  operationId: string | null;
  acceptance: OcrAcceptanceStatus;
  persistedBlockCount: number;
}
