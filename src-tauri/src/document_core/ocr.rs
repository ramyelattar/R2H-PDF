//! OCR pipeline: renders PDF pages to images, invokes the Python OCR worker
//! sidecar, and caches structured results per session/page.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ai_core::local_ai_paths::resolve_local_ai_root_diagnostics;

use super::errors::DocumentCoreError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBlock {
    pub id: String,
    pub text: String,
    pub bbox: OcrBBox,
    pub confidence: Option<f32>,
    pub block_type: String,
    #[serde(default)]
    pub reading_order: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrAvailabilityStatus {
    Available,
    MissingWorker,
    MissingPythonRuntime,
    MissingModel,
    MissingProcessor,
    InvalidModelLayout,
    UnsupportedPlatform,
    ResourceResolutionFailed,
}

impl OcrAvailabilityStatus {
    fn code(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::MissingWorker => "missing_worker",
            Self::MissingPythonRuntime => "missing_python_runtime",
            Self::MissingModel => "missing_model",
            Self::MissingProcessor => "missing_processor",
            Self::InvalidModelLayout => "invalid_model_layout",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::ResourceResolutionFailed => "resource_resolution_failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrAvailability {
    pub status: OcrAvailabilityStatus,
    pub available: bool,
    pub message: String,
    pub resource_source: String,
    pub model_id: String,
    pub missing_assets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrResultStatus {
    Completed,
    NoTextDetected,
}

// ─── Phase 27D — pure bbox conversion utilities ──────────────────────

/// Result of converting an image-pixel bbox into PDF point coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertedOcrBbox {
    /// PDF point rect [x0, y0, x1, y1] (origin bottom-left).
    pub bbox: [f32; 4],
    /// True when the original bbox extended past the page and was clamped.
    pub clamped: bool,
    /// True when the rect collapsed to zero area after clamping (caller
    /// should drop it and surface a warning).
    pub degenerate: bool,
    /// `"image_px"` when scaling was applied, `"unknown_assumed_pdf_points"`
    /// when image dims were missing and the worker bbox was used as-is.
    pub coordinate_space: &'static str,
    /// `[x_scale, y_scale]` from image pixels → PDF points.
    pub conversion_scale: [f32; 2],
}

/// Convert a worker bbox (image-pixel, top-left origin) into a PDF point
/// rect (bottom-left origin) given the rendered image dimensions and the
/// page's PDF point dimensions. Includes clamping and degenerate detection.
///
/// When `image_w_px == 0 || image_h_px == 0` the function falls back to
/// treating the bbox as if it were already in PDF points (legacy cached
/// results) and reports `coordinate_space = "unknown_assumed_pdf_points"`.
pub fn convert_ocr_bbox_to_pdf_points(
    bbox_px: [f32; 4],
    image_w_px: u32,
    image_h_px: u32,
    page_w_pts: f32,
    page_h_pts: f32,
) -> ConvertedOcrBbox {
    let [bx, by, bxw, byh] = bbox_px;
    // Treat input as [x, y, w, h] (matching OcrBBox); compute x1/y1 first.
    let px_x0 = bx;
    let px_y0 = by;
    let px_x1 = bx + bxw;
    let px_y1 = by + byh;

    let (scale_x, scale_y, space) = if image_w_px > 0 && image_h_px > 0 {
        (
            page_w_pts / image_w_px as f32,
            page_h_pts / image_h_px as f32,
            "image_px",
        )
    } else {
        (1.0_f32, 1.0_f32, "unknown_assumed_pdf_points")
    };

    let pdf_x0 = px_x0 * scale_x;
    let pdf_x1 = px_x1 * scale_x;
    let pdf_y_top = px_y0 * scale_y;
    let pdf_y_bot = px_y1 * scale_y;
    // Y flip: PDF y grows up, worker y grows down.
    let pdf_y0 = page_h_pts - pdf_y_bot;
    let pdf_y1 = page_h_pts - pdf_y_top;

    let cx0 = pdf_x0.clamp(0.0, page_w_pts);
    let cx1 = pdf_x1.clamp(0.0, page_w_pts);
    let cy0 = pdf_y0.clamp(0.0, page_h_pts);
    let cy1 = pdf_y1.clamp(0.0, page_h_pts);
    let clamped = (cx0 - pdf_x0).abs() > 0.1
        || (cx1 - pdf_x1).abs() > 0.1
        || (cy0 - pdf_y0).abs() > 0.1
        || (cy1 - pdf_y1).abs() > 0.1;
    let degenerate = cx1 <= cx0 || cy1 <= cy0;

    ConvertedOcrBbox {
        bbox: [cx0, cy0, cx1, cy1],
        clamped,
        degenerate,
        coordinate_space: space,
        conversion_scale: [scale_x, scale_y],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrPageResult {
    pub schema_version: u32,
    pub operation_id: String,
    pub document_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub page_width_points: f32,
    pub page_height_points: f32,
    pub rotation_degrees: i32,
    pub text: String,
    pub blocks: Vec<OcrBlock>,
    pub confidence: Option<f32>,
    pub language: String,
    pub language_source: String,
    pub engine: String,
    pub worker_version: Option<String>,
    pub model_version: Option<String>,
    pub duration_ms: u64,
    pub warnings: Vec<String>,
    pub status: OcrResultStatus,
    pub bbox_coordinate_space: String,
    pub created_at: u128,
    /// Phase 26E: the OCR worker bboxes are in IMAGE PIXEL coordinates
    /// (top-left origin) relative to the rendered image of `image_width_px`
    /// × `image_height_px`. Consumers (e.g. overlay creation) must scale
    /// to PDF user-space using the source page's point dimensions and
    /// flip the Y axis.
    #[serde(default)]
    pub image_width_px: u32,
    #[serde(default)]
    pub image_height_px: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrJobStatus {
    pub job_id: String,
    pub session_id: String,
    pub page_start: usize,
    pub page_end: usize,
    pub status: String, // "queued" | "running" | "completed" | "failed" | "cancelled"
    pub progress_current: usize,
    pub progress_total: usize,
    pub started_at: Option<u128>,
    pub finished_at: Option<u128>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    pub operation_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub force: bool,
    pub dpi: Option<u32>,
    pub language: String,
    pub model_id: String,
    pub output_format_version: u32,
    pub timeout_secs: Option<u64>,
    pub cancellation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRangeRequest {
    pub session_id: String,
    pub page_start: usize,
    pub page_end: usize,
    pub force: bool,
    pub dpi: Option<u32>,
}

// Worker JSON response
#[derive(Debug, Deserialize)]
struct WorkerResponse {
    ok: bool,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    operation_id: Option<String>,
    engine: Option<String>,
    page_index: Option<usize>,
    text: Option<String>,
    blocks: Option<Vec<WorkerBlock>>,
    confidence: Option<f32>,
    language: Option<String>,
    #[serde(default)]
    language_source: Option<String>,
    #[serde(default)]
    worker_version: Option<String>,
    #[serde(default)]
    model_version: Option<String>,
    #[serde(default)]
    warnings: Option<Vec<String>>,
    error_code: Option<String>,
    message: Option<String>,
    #[allow(dead_code)]
    details: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkerBlock {
    id: String,
    text: String,
    bbox: Option<WorkerBBox>,
    confidence: Option<f32>,
    block_type: String,
    #[serde(default)]
    reading_order: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WorkerBBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerValidationError {
    pub code: String,
    pub message: String,
}

impl WorkerValidationError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OcrRunContext {
    pub operation_id: String,
    pub document_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub page_width_points: f32,
    pub page_height_points: f32,
    pub rotation_degrees: i32,
    pub language: String,
    pub model_id: String,
    pub timeout_secs: u64,
    pub cancellation_id: Option<String>,
}

impl OcrRunContext {
    fn legacy(session_id: &str, page_index: usize) -> Self {
        Self {
            operation_id: format!("legacy-ocr-{session_id}-{page_index}"),
            document_id: String::new(),
            session_id: session_id.to_string(),
            page_index,
            page_width_points: 0.0,
            page_height_points: 0.0,
            rotation_degrees: 0,
            language: "auto".to_string(),
            model_id: "PaddleOCR-VL".to_string(),
            timeout_secs: 120,
            cancellation_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// OCR Cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OcrCacheKey {
    session_id: String,
    page_index: usize,
}

pub struct OcrCache {
    results: HashMap<OcrCacheKey, OcrPageResult>,
}

impl OcrCache {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub fn get(&self, session_id: &str, page_index: usize) -> Option<&OcrPageResult> {
        self.results.get(&OcrCacheKey {
            session_id: session_id.to_string(),
            page_index,
        })
    }

    pub fn insert(&mut self, result: OcrPageResult) {
        let key = OcrCacheKey {
            session_id: result.session_id.clone(),
            page_index: result.page_index,
        };
        self.results.insert(key, result);
    }

    pub fn invalidate_session(&mut self, session_id: &str) {
        self.results.retain(|k, _| k.session_id != session_id);
    }

    pub fn invalidate_page(&mut self, session_id: &str, page_index: usize) {
        self.results.remove(&OcrCacheKey {
            session_id: session_id.to_string(),
            page_index,
        });
    }

    pub fn has_result(&self, session_id: &str, page_index: usize) -> bool {
        self.get(session_id, page_index).is_some()
    }

    pub fn get_text(&self, session_id: &str, page_index: usize) -> Option<String> {
        self.get(session_id, page_index).map(|r| r.text.clone())
    }

    pub fn all_texts_for_session(&self, session_id: &str) -> Vec<(usize, String)> {
        let mut results: Vec<_> = self
            .results
            .iter()
            .filter(|(k, _)| k.session_id == session_id)
            .map(|(k, v)| (k.page_index, v.text.clone()))
            .collect();
        results.sort_by_key(|(idx, _)| *idx);
        results
    }
}

// ---------------------------------------------------------------------------
// OCR Engine
// ---------------------------------------------------------------------------

pub struct OcrEngine {
    pub cache: OcrCache,
    python_path: PathBuf,
    worker_path: PathBuf,
    model_path: PathBuf,
    resource_source: String,
    model_id: String,
    smoke_engine: Option<String>,
    default_dpi: u32,
    timeout_secs: u64,
}

impl OcrEngine {
    pub fn new() -> Self {
        let resolution = resolve_local_ai_root_diagnostics(None);
        let local_ai_root = resolution.root_path();
        let worker_path = local_ai_root.join("workers/paddleocr_vl_worker.py");
        let model_path = local_ai_root.join("models/ocr/PaddleOCR-VL");
        let python_path = resolve_python_path(&local_ai_root);

        Self {
            cache: OcrCache::new(),
            python_path,
            worker_path,
            model_path,
            resource_source: resolution.source,
            model_id: "PaddleOCR-VL".to_string(),
            smoke_engine: None,
            default_dpi: 200,
            timeout_secs: 120,
        }
    }

    /// Return a structured availability result for the optional local OCR pack.
    /// This intentionally does not validate unrelated LLM or embedding assets.
    pub fn check_availability(&self) -> OcrAvailability {
        if !cfg!(target_os = "windows") {
            return self.unavailable(
                OcrAvailabilityStatus::UnsupportedPlatform,
                "The packaged OCR runtime is supported on Windows only.",
                vec![],
            );
        }

        if self.resource_source == "unresolved_relative" && !self.worker_path.is_absolute() {
            return self.unavailable(
                OcrAvailabilityStatus::ResourceResolutionFailed,
                "The optional OCR resource pack could not be resolved from a supported application location.",
                vec![],
            );
        }

        if !self.worker_path.is_file() {
            return self.unavailable(
                OcrAvailabilityStatus::MissingWorker,
                "The local OCR worker is not installed in the configured OCR pack.",
                vec!["workers/paddleocr_vl_worker.py".to_string()],
            );
        }
        if !self.model_path.is_dir() {
            return self.unavailable(
                OcrAvailabilityStatus::MissingModel,
                "The local OCR model pack is not installed.",
                vec!["models/ocr/PaddleOCR-VL".to_string()],
            );
        }

        let config_path = self.model_path.join("config.json");
        let weights_path = self.model_path.join("model.safetensors");
        if !config_path.is_file() || !file_is_non_empty(&weights_path) {
            return self.unavailable(
                OcrAvailabilityStatus::MissingModel,
                "The PaddleOCR-VL model weights or configuration are missing.",
                missing_files(&self.model_path, &["config.json", "model.safetensors"]),
            );
        }

        let config = match std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
        {
            Some(value)
                if value.get("model_type").and_then(|v| v.as_str()) == Some("paddleocr_vl") =>
            {
                value
            }
            _ => {
                return self.unavailable(
                    OcrAvailabilityStatus::InvalidModelLayout,
                    "The installed OCR model configuration is not a supported PaddleOCR-VL layout.",
                    vec!["models/ocr/PaddleOCR-VL/config.json".to_string()],
                );
            }
        };
        let _ = config;

        let processor_assets = [
            "preprocessor_config.json",
            "processor_config.json",
            "tokenizer_config.json",
            "tokenizer.json",
            "tokenizer.model",
            "special_tokens_map.json",
            "chat_template.jinja",
            "processing_paddleocr_vl.py",
            "image_processing_paddleocr_vl.py",
            "configuration_paddleocr_vl.py",
            "modeling_paddleocr_vl.py",
        ];
        let missing_processor = missing_files(&self.model_path, &processor_assets);
        if !missing_processor.is_empty() {
            return self.unavailable(
                OcrAvailabilityStatus::MissingProcessor,
                "The OCR processor or tokenizer assets are incomplete.",
                missing_processor,
            );
        }

        if let Err(message) = probe_python_runtime(&self.python_path) {
            return self.unavailable(
                OcrAvailabilityStatus::MissingPythonRuntime,
                &message,
                vec![],
            );
        }

        OcrAvailability {
            status: OcrAvailabilityStatus::Available,
            available: true,
            message: "The local OCR worker, Python runtime, model, and processor assets are ready."
                .to_string(),
            resource_source: self.resource_source.clone(),
            model_id: self.model_id.clone(),
            missing_assets: vec![],
        }
    }

    fn unavailable(
        &self,
        status: OcrAvailabilityStatus,
        message: &str,
        missing_assets: Vec<String>,
    ) -> OcrAvailability {
        OcrAvailability {
            status,
            available: false,
            message: message.to_string(),
            resource_source: self.resource_source.clone(),
            model_id: self.model_id.clone(),
            missing_assets,
        }
    }

    /// Run OCR on a single page image.
    /// `image_path` must point to a PNG file of the rendered page.
    ///
    /// Phase 26E: `image_width_px` / `image_height_px` are the dimensions
    /// of the rendered image fed to the OCR worker. We record them in the
    /// result so consumers (overlay creation, RAG) can translate the
    /// worker's pixel-space bboxes into PDF user-space without guessing
    /// the render DPI.
    pub fn run_ocr_on_image(
        &mut self,
        session_id: &str,
        page_index: usize,
        image_path: &Path,
        image_width_px: u32,
        image_height_px: u32,
        force: bool,
    ) -> Result<OcrPageResult, DocumentCoreError> {
        let context = OcrRunContext::legacy(session_id, page_index);
        self.run_ocr_on_image_with_context(
            image_path,
            image_width_px,
            image_height_px,
            force,
            context,
            None,
        )
    }

    pub fn run_ocr_on_image_with_context(
        &mut self,
        image_path: &Path,
        image_width_px: u32,
        image_height_px: u32,
        force: bool,
        context: OcrRunContext,
        cancellations: Option<&Arc<Mutex<HashSet<String>>>>,
    ) -> Result<OcrPageResult, DocumentCoreError> {
        if image_width_px == 0 || image_height_px == 0 {
            return Err(DocumentCoreError::OcrError(
                "[OCR_INVALID_INPUT] Rendered OCR input has zero dimensions.".to_string(),
            ));
        }
        if !image_path.is_file() {
            return Err(DocumentCoreError::OcrError(
                "[OCR_INPUT_NOT_FOUND] Rendered OCR input is missing.".to_string(),
            ));
        }

        let availability = self.check_availability();
        if !availability.available {
            return Err(DocumentCoreError::OcrError(format!(
                "[OCR_{}] {}",
                availability.status.code(),
                availability.message
            )));
        }

        // Check cache only after the current resource boundary has been
        // validated. A stale in-memory result must never hide an unavailable
        // worker/model installation.
        if !force {
            if let Some(cached) = self.cache.get(&context.session_id, context.page_index) {
                return Ok(cached.clone());
            }
        }

        // Build worker request JSON.
        let mut request = serde_json::json!({
            "image_path": image_path.to_string_lossy(),
            "model_path": self.model_path.to_string_lossy(),
            "schema_version": 1,
            "output_format_version": 1,
            "operation_id": context.operation_id.clone(),
            "page_index": context.page_index,
            "language": context.language.clone(),
            "model_id": context.model_id.clone(),
            "timeout_secs": context.timeout_secs,
            "cancellation_id": context.cancellation_id.clone(),
        });
        if let Some(engine) = &self.smoke_engine {
            request["engine"] = serde_json::Value::String(engine.clone());
        }

        let request_json = serde_json::to_string(&request).map_err(|e| {
            DocumentCoreError::OcrError(format!("Failed to serialize request: {e}"))
        })?;

        let started = Instant::now();
        let output = match run_worker_process_with_cancel(
            &self.python_path,
            &self.worker_path,
            &request_json,
            Duration::from_secs(context.timeout_secs.max(1)),
            || cancellation_requested(cancellations, context.cancellation_id.as_deref()),
        ) {
            Ok(output) => output,
            Err(WorkerProcessFailure::Start(message)) => {
                return Err(DocumentCoreError::OcrError(format!(
                    "[OCR_WORKER_START_FAILED] {message}"
                )))
            }
            Err(WorkerProcessFailure::Input(message, output)) => {
                return Err(DocumentCoreError::OcrError(format!(
                    "[OCR_WORKER_INPUT_FAILED] {message}; {}",
                    output.summary()
                )))
            }
            Err(WorkerProcessFailure::Wait(message)) => {
                return Err(DocumentCoreError::OcrError(format!(
                    "[OCR_WORKER_WAIT_FAILED] {message}"
                )))
            }
            Err(WorkerProcessFailure::Timeout(output)) => {
                return Err(DocumentCoreError::OcrError(format!(
                    "[OCR_TIMEOUT] OCR worker timed out after {} seconds; {}",
                    context.timeout_secs.max(1),
                    output.summary()
                )))
            }
            Err(WorkerProcessFailure::Cancelled(output)) => {
                clear_cancellation(cancellations, context.cancellation_id.as_deref());
                return Err(DocumentCoreError::OcrError(format!(
                    "[OCR_CANCELLED] OCR was cancelled by the user; {}",
                    output.summary()
                )));
            }
        };

        validate_worker_exit(output.status_code, &output.stdout, &output.stderr)
            .map_err(|e| DocumentCoreError::OcrError(format!("[{}] {}", e.code, e.message)))?;

        // Parse worker response.
        let response = parse_worker_response(&output.stdout)
            .map_err(|e| DocumentCoreError::OcrError(format!("[{}] {}", e.code, e.message)))?;
        let response = validate_worker_response(
            response,
            &context.operation_id,
            context.page_index,
            image_width_px,
            image_height_px,
            self.smoke_engine.is_some(),
        )
        .map_err(|e| DocumentCoreError::OcrError(format!("[{}] {}", e.code, e.message)))?;

        // Convert worker blocks to our format.
        let blocks: Vec<OcrBlock> = response
            .blocks
            .unwrap_or_default()
            .into_iter()
            .map(|b| OcrBlock {
                id: b.id,
                text: b.text,
                bbox: b
                    .bbox
                    .map(|bbox| OcrBBox {
                        x: bbox.x,
                        y: bbox.y,
                        width: bbox.width,
                        height: bbox.height,
                    })
                    .unwrap_or(OcrBBox {
                        x: 0.0,
                        y: 0.0,
                        width: 0.0,
                        height: 0.0,
                    }),
                confidence: b.confidence,
                block_type: b.block_type,
                reading_order: b.reading_order,
            })
            .collect();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();

        let result = OcrPageResult {
            schema_version: response.schema_version.unwrap_or(1),
            operation_id: context.operation_id,
            document_id: context.document_id,
            session_id: context.session_id,
            page_index: context.page_index,
            page_width_points: context.page_width_points,
            page_height_points: context.page_height_points,
            rotation_degrees: context.rotation_degrees,
            text: response.text.unwrap_or_default(),
            blocks,
            confidence: response.confidence,
            language: response.language.unwrap_or_else(|| "auto".to_string()),
            language_source: response
                .language_source
                .unwrap_or_else(|| "requested".to_string()),
            engine: response
                .engine
                .unwrap_or_else(|| "PaddleOCR-VL".to_string()),
            worker_version: response.worker_version,
            model_version: response.model_version,
            duration_ms: started.elapsed().as_millis() as u64,
            warnings: response.warnings.unwrap_or_default(),
            status: if response.status.as_deref() == Some("no_text_detected") {
                OcrResultStatus::NoTextDetected
            } else {
                OcrResultStatus::Completed
            },
            bbox_coordinate_space: "image_px".to_string(),
            created_at: now,
            image_width_px,
            image_height_px,
        };

        // Cache the result.
        self.cache.insert(result.clone());

        Ok(result)
    }

    pub fn get_default_dpi(&self) -> u32 {
        self.default_dpi
    }

    pub fn set_python_path(&mut self, path: String) {
        self.python_path = PathBuf::from(path);
    }

    pub fn set_model_path(&mut self, path: PathBuf) {
        self.model_path = path;
    }

    pub fn worker_path(&self) -> &Path {
        &self.worker_path
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Explicitly used by the release smoke harness. Production construction
    /// never reads an environment variable or selects this engine.
    pub fn set_smoke_engine_for_release(&mut self, engine: &str) {
        self.smoke_engine = Some(engine.to_string());
    }

    pub fn python_path(&self) -> &Path {
        &self.python_path
    }
}

const MAX_WORKER_CAPTURE_BYTES: usize = 4 * 1024 * 1024;

struct CapturedWorkerStream {
    bytes: Vec<u8>,
    total_len: usize,
}

#[derive(Debug)]
struct WorkerProcessOutput {
    status_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_len: usize,
    stderr_len: usize,
}

impl WorkerProcessOutput {
    fn summary(&self) -> String {
        format!(
            "stdout_bytes={}, stderr_bytes={}, stdout_truncated={}, stderr_truncated={}",
            self.stdout_len,
            self.stderr_len,
            self.stdout_len > self.stdout.len(),
            self.stderr_len > self.stderr.len()
        )
    }
}

#[derive(Debug)]
enum WorkerProcessFailure {
    Start(String),
    Input(String, WorkerProcessOutput),
    Wait(String),
    Timeout(WorkerProcessOutput),
    Cancelled(WorkerProcessOutput),
}

fn read_worker_stream<R: Read>(mut reader: R) -> CapturedWorkerStream {
    let mut bytes = Vec::new();
    let mut total_len = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                total_len = total_len.saturating_add(read);
                if bytes.len() < MAX_WORKER_CAPTURE_BYTES {
                    let remaining = MAX_WORKER_CAPTURE_BYTES - bytes.len();
                    bytes.extend_from_slice(&buffer[..read.min(remaining)]);
                }
            }
        }
    }
    CapturedWorkerStream { bytes, total_len }
}

fn terminate_worker_tree(child: &mut Child) -> Option<ExitStatus> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let _ = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .output();
    }
    let _ = child.kill();
    child.wait().ok()
}

fn collect_worker_output(
    child: &mut Child,
    terminal_status: Option<ExitStatus>,
    stdout_thread: std::thread::JoinHandle<CapturedWorkerStream>,
    stderr_thread: std::thread::JoinHandle<CapturedWorkerStream>,
) -> Result<WorkerProcessOutput, String> {
    let status = match terminal_status {
        Some(status) => status,
        None => child
            .wait()
            .map_err(|error| format!("failed waiting for OCR worker: {error}"))?,
    };
    let stdout = stdout_thread
        .join()
        .map_err(|_| "OCR worker stdout reader panicked".to_string())?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "OCR worker stderr reader panicked".to_string())?;
    Ok(WorkerProcessOutput {
        status_code: status.code().unwrap_or(-1),
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_len: stdout.total_len,
        stderr_len: stderr.total_len,
    })
}

fn run_worker_process_with_cancel<F>(
    python_path: &Path,
    worker_path: &Path,
    request_json: &str,
    timeout: Duration,
    mut cancellation_requested: F,
) -> Result<WorkerProcessOutput, WorkerProcessFailure>
where
    F: FnMut() -> bool,
{
    let mut child = Command::new(python_path)
        .arg(worker_path.to_string_lossy().as_ref())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| WorkerProcessFailure::Start(error.to_string()))?;

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = terminate_worker_tree(&mut child);
            return Err(WorkerProcessFailure::Start(
                "OCR worker stdout pipe was unavailable".to_string(),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = terminate_worker_tree(&mut child);
            return Err(WorkerProcessFailure::Start(
                "OCR worker stderr pipe was unavailable".to_string(),
            ));
        }
    };
    let stdout_thread = std::thread::spawn(move || read_worker_stream(stdout));
    let stderr_thread = std::thread::spawn(move || read_worker_stream(stderr));

    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            let terminal_status = terminate_worker_tree(&mut child);
            let _ =
                collect_worker_output(&mut child, terminal_status, stdout_thread, stderr_thread);
            return Err(WorkerProcessFailure::Start(
                "OCR worker stdin pipe was unavailable".to_string(),
            ));
        }
    };
    if let Err(error) = stdin.write_all(request_json.as_bytes()) {
        let terminal_status = terminate_worker_tree(&mut child);
        let output =
            collect_worker_output(&mut child, terminal_status, stdout_thread, stderr_thread)
                .map_err(WorkerProcessFailure::Wait)?;
        return Err(WorkerProcessFailure::Input(error.to_string(), output));
    }
    drop(stdin);

    let started = Instant::now();
    let mut failure_kind = None;
    let terminal_status = loop {
        if cancellation_requested() {
            failure_kind = Some("cancelled");
            break terminate_worker_tree(&mut child);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break Some(status);
            }
            Ok(None) if started.elapsed() > timeout => {
                failure_kind = Some("timeout");
                break terminate_worker_tree(&mut child);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let terminal_status = terminate_worker_tree(&mut child);
                let _ = collect_worker_output(
                    &mut child,
                    terminal_status,
                    stdout_thread,
                    stderr_thread,
                );
                return Err(WorkerProcessFailure::Wait(error.to_string()));
            }
        }
    };

    let output = collect_worker_output(&mut child, terminal_status, stdout_thread, stderr_thread)
        .map_err(WorkerProcessFailure::Wait)?;
    match failure_kind {
        Some("timeout") => Err(WorkerProcessFailure::Timeout(output)),
        Some("cancelled") => Err(WorkerProcessFailure::Cancelled(output)),
        _ => Ok(output),
    }
}

#[cfg(test)]
fn run_worker_process_for_test(
    python_path: &Path,
    worker_path: &Path,
    request_json: &[u8],
    timeout: Duration,
) -> Result<WorkerProcessOutput, String> {
    let request_json = std::str::from_utf8(request_json)
        .map_err(|error| format!("test request was not UTF-8: {error}"))?;
    run_worker_process_with_cancel(python_path, worker_path, request_json, timeout, || false)
        .map_err(|failure| format!("worker protocol failure: {failure:?}"))
}

fn resolve_python_path(local_ai_root: &Path) -> PathBuf {
    for relative in [
        "runtimes/python/python.exe",
        "runtime/python.exe",
        "python/python.exe",
    ] {
        let candidate = local_ai_root.join(relative);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("python")
}

fn file_is_non_empty(path: &Path) -> bool {
    path.is_file()
        && std::fs::metadata(path)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

fn missing_files(root: &Path, relative_paths: &[&str]) -> Vec<String> {
    relative_paths
        .iter()
        .filter(|relative| !file_is_non_empty(&root.join(relative)))
        .map(|relative| (*relative).to_string())
        .collect()
}

fn probe_python_runtime(path: &Path) -> Result<(), String> {
    let version = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|_| "A supported Python executable could not be started.".to_string())?;
    if !version.status.success() {
        return Err(
            "The configured Python executable returned a non-zero version status.".to_string(),
        );
    }
    let imports = "import importlib,importlib.util,sys; required=('torch','transformers','PIL'); missing=[name for name in required if importlib.util.find_spec(name) is None];\nif not missing:\n    module=importlib.import_module('transformers.models.paddleocr_vl');\n    missing += [] if hasattr(module,'PaddleOCRVLForConditionalGeneration') else ['transformers.models.paddleocr_vl.PaddleOCRVLForConditionalGeneration'];\nprint(','.join(missing)); sys.exit(1 if missing else 0)";
    let probe = Command::new(path)
        .args(["-c", imports])
        .output()
        .map_err(|_| {
            "The configured Python runtime could not run its OCR dependency probe.".to_string()
        })?;
    if !probe.status.success() {
        let missing = String::from_utf8_lossy(&probe.stdout).trim().to_string();
        return Err(if missing.is_empty() {
            "The configured Python runtime is missing an OCR dependency.".to_string()
        } else {
            format!("The configured Python runtime is missing OCR dependencies: {missing}.")
        });
    }
    Ok(())
}

fn cancellation_requested(
    cancellations: Option<&Arc<Mutex<HashSet<String>>>>,
    id: Option<&str>,
) -> bool {
    let (Some(registry), Some(id)) = (cancellations, id) else {
        return false;
    };
    registry
        .lock()
        .map(|items| items.contains(id))
        .unwrap_or(false)
}

fn clear_cancellation(cancellations: Option<&Arc<Mutex<HashSet<String>>>>, id: Option<&str>) {
    let (Some(registry), Some(id)) = (cancellations, id) else {
        return;
    };
    if let Ok(mut items) = registry.lock() {
        items.remove(id);
    }
}

pub fn operation_temp_path(operation_id: &str) -> PathBuf {
    static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
    let safe: String = operation_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(48)
        .collect();
    let safe = if safe.is_empty() {
        "operation".to_string()
    } else {
        safe
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "r2h-ocr-{safe}-{}-{stamp}-{}.ppm",
        std::process::id(),
        NEXT_OPERATION.fetch_add(1, Ordering::Relaxed)
    ))
}

pub fn validate_worker_exit(
    exit_code: i32,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), WorkerValidationError> {
    if exit_code != 0 {
        if let Ok(response) = serde_json::from_slice::<WorkerResponse>(stdout) {
            if !response.ok {
                let code = response
                    .error_code
                    .unwrap_or_else(|| "OCR_WORKER_FAILED".to_string());
                let message = response
                    .message
                    .unwrap_or_else(|| "OCR worker reported failure.".to_string());
                return Err(WorkerValidationError::new(&code, message));
            }
        }
        let detail = String::from_utf8_lossy(stderr).trim().to_string();
        return Err(WorkerValidationError::new(
            "OCR_WORKER_EXIT_NONZERO",
            if detail.is_empty() {
                format!("OCR worker exited with code {exit_code}.")
            } else {
                format!("OCR worker exited with code {exit_code}: {detail}")
            },
        ));
    }
    if stdout.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(WorkerValidationError::new(
            "OCR_WORKER_EMPTY_OUTPUT",
            "OCR worker returned no structured output.",
        ));
    }
    Ok(())
}

fn parse_worker_response(stdout: &[u8]) -> Result<WorkerResponse, WorkerValidationError> {
    std::str::from_utf8(stdout).map_err(|error| {
        WorkerValidationError::new(
            "OCR_INVALID_UTF8_OUTPUT",
            format!("OCR worker stdout was not valid UTF-8: {error}"),
        )
    })?;
    serde_json::from_slice(stdout).map_err(|error| {
        WorkerValidationError::new(
            "OCR_MALFORMED_OUTPUT",
            format!("Failed to parse OCR worker output: {error}"),
        )
    })
}

fn validate_worker_response(
    response: WorkerResponse,
    operation_id: &str,
    page_index: usize,
    image_width_px: u32,
    image_height_px: u32,
    allow_smoke_engine: bool,
) -> Result<WorkerResponse, WorkerValidationError> {
    if !response.ok {
        let code = response
            .error_code
            .unwrap_or_else(|| "OCR_WORKER_FAILED".to_string());
        let message = response
            .message
            .unwrap_or_else(|| "OCR worker reported failure.".to_string());
        return Err(WorkerValidationError::new(&code, message));
    }
    if response.schema_version != Some(1) {
        return Err(WorkerValidationError::new(
            "OCR_UNSUPPORTED_SCHEMA",
            "OCR worker result schema is unsupported.",
        ));
    }
    if response.operation_id.as_deref() != Some(operation_id) {
        return Err(WorkerValidationError::new(
            "OCR_OPERATION_MISMATCH",
            "OCR worker result belongs to a different operation.",
        ));
    }
    if response.page_index != Some(page_index) {
        return Err(WorkerValidationError::new(
            "OCR_PAGE_MISMATCH",
            "OCR worker result belongs to a different page.",
        ));
    }
    let engine = response.engine.as_deref().unwrap_or("");
    if !allow_smoke_engine && engine.to_ascii_lowercase().contains("template") {
        return Err(WorkerValidationError::new(
            "OCR_TEMPLATE_ENGINE_FORBIDDEN",
            "Template/smoke OCR output is not valid production OCR.",
        ));
    }

    let text = response.text.as_deref().unwrap_or("").trim();
    let blocks = response.blocks.as_ref().map(Vec::as_slice).unwrap_or(&[]);
    let status = response
        .status
        .as_deref()
        .unwrap_or(if text.is_empty() && blocks.is_empty() {
            "no_text_detected"
        } else {
            "completed"
        });
    if status == "no_text_detected" {
        if !text.is_empty() || !blocks.is_empty() {
            return Err(WorkerValidationError::new(
                "OCR_INVALID_NO_TEXT_RESULT",
                "no_text_detected result contained text or blocks.",
            ));
        }
        return Ok(response);
    }
    if status != "completed" {
        return Err(WorkerValidationError::new(
            "OCR_UNSUPPORTED_STATUS",
            format!("OCR worker returned unsupported status: {status}."),
        ));
    }
    if text.is_empty() {
        return Err(WorkerValidationError::new(
            "OCR_EMPTY_TEXT",
            "OCR worker returned completed without text.",
        ));
    }
    if blocks.is_empty() {
        return Err(WorkerValidationError::new(
            "OCR_GEOMETRY_UNAVAILABLE",
            "OCR worker returned text without model-supplied geometry.",
        ));
    }

    validate_confidence(response.confidence, "aggregate confidence")?;
    let mut ids = HashSet::new();
    for block in blocks {
        if block.id.trim().is_empty() || !ids.insert(block.id.clone()) {
            return Err(WorkerValidationError::new(
                "OCR_INVALID_BLOCK",
                "OCR block IDs must be non-empty and unique.",
            ));
        }
        if block.text.trim().is_empty() {
            return Err(WorkerValidationError::new(
                "OCR_INVALID_BLOCK",
                "OCR blocks must contain text.",
            ));
        }
        let bbox = block.bbox.as_ref().ok_or_else(|| {
            WorkerValidationError::new(
                "OCR_GEOMETRY_UNAVAILABLE",
                "OCR block did not include model-supplied geometry.",
            )
        })?;
        validate_bbox(bbox, image_width_px as f32, image_height_px as f32)?;
        validate_confidence(block.confidence, "block confidence")?;
    }
    Ok(response)
}

fn validate_confidence(value: Option<f32>, label: &str) -> Result<(), WorkerValidationError> {
    if let Some(value) = value {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(WorkerValidationError::new(
                "OCR_INVALID_CONFIDENCE",
                format!("{label} was not finite and within 0..1."),
            ));
        }
    }
    Ok(())
}

fn validate_bbox(bbox: &WorkerBBox, width: f32, height: f32) -> Result<(), WorkerValidationError> {
    let values = [bbox.x, bbox.y, bbox.width, bbox.height];
    if values.iter().any(|value| !value.is_finite())
        || bbox.width <= 0.0
        || bbox.height <= 0.0
        || bbox.x < 0.0
        || bbox.y < 0.0
        || bbox.x + bbox.width > width
        || bbox.y + bbox.height > height
    {
        return Err(WorkerValidationError::new(
            "OCR_INVALID_GEOMETRY",
            "OCR block geometry was non-finite, degenerate, or outside the rendered page.",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri state wrapper
// ---------------------------------------------------------------------------

pub struct OcrState {
    pub engine: Mutex<OcrEngine>,
    pub cancellations: Arc<Mutex<HashSet<String>>>,
}

impl OcrState {
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(OcrEngine::new()),
            cancellations: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_result(session_id: &str, page_index: usize, text: &str) -> OcrPageResult {
        OcrPageResult {
            schema_version: 1,
            operation_id: format!("test-op-{page_index}"),
            document_id: "test-document".to_string(),
            session_id: session_id.to_string(),
            page_index,
            page_width_points: 612.0,
            page_height_points: 792.0,
            rotation_degrees: 0,
            text: text.to_string(),
            blocks: vec![],
            confidence: None,
            language: "en".to_string(),
            language_source: "requested".to_string(),
            engine: "test".to_string(),
            worker_version: Some("test".to_string()),
            model_version: None,
            duration_ms: 0,
            warnings: vec![],
            status: OcrResultStatus::Completed,
            bbox_coordinate_space: "image_px".to_string(),
            created_at: 0,
            image_width_px: 1224,
            image_height_px: 1584,
        }
    }

    #[test]
    fn structured_availability_classifies_missing_worker() {
        let mut engine = OcrEngine::new();
        engine.worker_path = PathBuf::from("/nonexistent/ocr-worker.py");
        let result = engine.check_availability();
        assert_eq!(result.status, OcrAvailabilityStatus::MissingWorker);
        assert!(!result.available);
    }

    #[test]
    fn non_zero_worker_exit_is_never_success_even_with_json_stdout() {
        let output = br#"{"ok":true,"status":"completed","text":"real"}"#;
        let error = validate_worker_exit(1, output, b"");
        assert!(error.is_err());
        assert_eq!(error.unwrap_err().code, "OCR_WORKER_EXIT_NONZERO");
    }

    #[test]
    fn large_stderr_worker_can_finish_without_pipe_deadlock() {
        let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../local-ai/workers/test_ocr_protocol_worker.py");
        let output = run_worker_process_for_test(
            Path::new("python"),
            &worker,
            br#"{"mode":"large_stderr","operation_id":"protocol-op","page_index":0}"#,
            Duration::from_secs(10),
        )
        .expect("large-stderr worker should complete");
        assert_eq!(output.status_code, 0);
        assert!(output.stderr_len >= 2 * 1024 * 1024);
        assert!(output
            .stdout
            .windows(b"\"status\": \"no_text_detected\"".len())
            .any(|window| window == b"\"status\": \"no_text_detected\""));
    }

    #[test]
    fn timeout_terminates_worker_process_tree_and_captures_partial_streams() {
        let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../local-ai/workers/test_ocr_protocol_worker.py");
        let error = run_worker_process_with_cancel(
            Path::new("python"),
            &worker,
            r#"{"mode":"spawn_child","operation_id":"timeout-op","page_index":0}"#,
            Duration::from_secs(1),
            || false,
        )
        .expect_err("sleeping worker should hit the bounded timeout");
        let output = match error {
            WorkerProcessFailure::Timeout(output) => output,
            other => panic!("expected timeout, got {other:?}"),
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        let child_pid = stderr
            .lines()
            .find_map(|line| line.strip_prefix("child_pid=")?.parse::<u32>().ok())
            .expect("test worker should report its child pid");
        let task_list = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {child_pid}")])
            .output()
            .expect("tasklist should be available on Windows");
        let task_list_text = String::from_utf8_lossy(&task_list.stdout);
        assert!(!task_list_text.contains(&child_pid.to_string()));
        assert_eq!(output.stdout, b"partial-worker-output");
        assert!(output.stdout_len > 0);
        assert!(output.stderr_len > 0);
    }

    #[test]
    fn non_zero_structured_worker_failure_preserves_error_code() {
        let output =
            br#"{"ok":false,"error_code":"OCR_MODEL_LOAD_FAILED","message":"model unavailable"}"#;
        let error = validate_worker_exit(1, output, b"").unwrap_err();
        assert_eq!(error.code, "OCR_MODEL_LOAD_FAILED");
        assert_eq!(error.message, "model unavailable");
    }

    #[test]
    fn text_only_worker_result_is_rejected_for_overlay_production() {
        let response = WorkerResponse {
            ok: true,
            schema_version: Some(1),
            status: Some("completed".into()),
            operation_id: Some("op-1".into()),
            engine: Some("PaddleOCR-VL".into()),
            page_index: Some(0),
            text: Some("real model text".into()),
            blocks: Some(vec![]),
            confidence: None,
            language: Some("auto".into()),
            language_source: Some("requested".into()),
            warnings: Some(vec![]),
            error_code: None,
            message: None,
            details: None,
            worker_version: Some("1.0.0".into()),
            model_version: None,
        };
        let error = validate_worker_response(response, "op-1", 0, 1000, 1000, false);
        assert!(error.is_err());
        assert_eq!(error.unwrap_err().code, "OCR_GEOMETRY_UNAVAILABLE");
    }

    #[test]
    fn operation_temp_paths_are_unique_and_operation_scoped() {
        let first = operation_temp_path("op/one");
        let second = operation_temp_path("op/two");
        assert_ne!(first, second);
        assert!(first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("r2h-ocr-"));
        assert!(!first.to_string_lossy().contains("op/one"));
    }

    #[test]
    fn empty_stdout_is_a_worker_failure() {
        let error = validate_worker_exit(0, b"\n  \t", b"");
        assert_eq!(error.unwrap_err().code, "OCR_WORKER_EMPTY_OUTPUT");
    }

    #[test]
    fn malformed_worker_json_is_not_a_result() {
        let parsed = serde_json::from_slice::<WorkerResponse>(b"{\"ok\":true");
        assert!(parsed.is_err());
    }

    #[test]
    fn mixed_log_and_json_worker_stdout_is_rejected() {
        let error = parse_worker_response(
            br#"diagnostic-before-json
{"ok":true,"status":"no_text_detected"}"#,
        )
        .unwrap_err();
        assert_eq!(error.code, "OCR_MALFORMED_OUTPUT");
    }

    #[test]
    fn invalid_utf8_worker_stdout_is_reported_explicitly() {
        let error = parse_worker_response(b"{\xff").unwrap_err();
        assert_eq!(error.code, "OCR_INVALID_UTF8_OUTPUT");
    }

    #[test]
    fn invalid_worker_geometry_is_rejected() {
        let response = WorkerResponse {
            ok: true,
            schema_version: Some(1),
            status: Some("completed".into()),
            operation_id: Some("op-geometry".into()),
            engine: Some("PaddleOCR-VL".into()),
            page_index: Some(0),
            text: Some("text".into()),
            blocks: Some(vec![WorkerBlock {
                id: "b1".into(),
                text: "text".into(),
                bbox: Some(WorkerBBox {
                    x: f32::NAN,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                }),
                confidence: None,
                block_type: "text".into(),
                reading_order: Some(0),
            }]),
            confidence: None,
            language: Some("auto".into()),
            language_source: Some("requested".into()),
            warnings: Some(vec![]),
            error_code: None,
            message: None,
            details: None,
            worker_version: Some("1.1.0".into()),
            model_version: None,
        };
        let error =
            validate_worker_response(response, "op-geometry", 0, 1000, 1000, false).unwrap_err();
        assert_eq!(error.code, "OCR_INVALID_GEOMETRY");
    }

    #[test]
    fn no_text_worker_result_is_truthful_and_non_durable_as_overlay() {
        let response = WorkerResponse {
            ok: true,
            schema_version: Some(1),
            status: Some("no_text_detected".into()),
            operation_id: Some("op-empty".into()),
            engine: Some("PaddleOCR-VL".into()),
            page_index: Some(0),
            text: Some(String::new()),
            blocks: Some(vec![]),
            confidence: None,
            language: Some("auto".into()),
            language_source: Some("requested".into()),
            warnings: Some(vec![]),
            error_code: None,
            message: None,
            details: None,
            worker_version: Some("1.1.0".into()),
            model_version: None,
        };
        let validated =
            validate_worker_response(response, "op-empty", 0, 1000, 1000, false).unwrap();
        assert_eq!(validated.status.as_deref(), Some("no_text_detected"));
        assert!(validated.blocks.unwrap_or_default().is_empty());
    }

    #[test]
    fn ocr_cache_stores_and_retrieves() {
        let mut cache = OcrCache::new();
        let result = test_result("s1", 0, "Hello world");
        cache.insert(result.clone());
        assert!(cache.has_result("s1", 0));
        assert_eq!(cache.get_text("s1", 0), Some("Hello world".to_string()));
    }

    #[test]
    fn ocr_cache_invalidate_page() {
        let mut cache = OcrCache::new();
        cache.insert(test_result("s1", 0, "A"));
        cache.insert(test_result("s1", 1, "B"));
        cache.invalidate_page("s1", 0);
        assert!(!cache.has_result("s1", 0));
        assert!(cache.has_result("s1", 1));
    }

    #[test]
    fn ocr_cache_invalidate_session() {
        let mut cache = OcrCache::new();
        cache.insert(test_result("s1", 0, "A"));
        cache.invalidate_session("s1");
        assert!(!cache.has_result("s1", 0));
    }

    #[test]
    fn ocr_engine_check_availability_missing_worker() {
        let mut engine = OcrEngine::new();
        engine.worker_path = PathBuf::from("/nonexistent/worker.py");
        let result = engine.check_availability();
        assert_eq!(result.status, OcrAvailabilityStatus::MissingWorker);
    }

    #[test]
    fn ocr_engine_check_availability_missing_model() {
        let mut engine = OcrEngine::new();
        engine.worker_path = PathBuf::from(file!()); // Use this file as a "valid" file
        engine.model_path = PathBuf::from("/nonexistent/model");
        let result = engine.check_availability();
        assert_eq!(result.status, OcrAvailabilityStatus::MissingModel);
    }

    // ─── Phase 27D: bbox conversion correctness ─────────────────────────

    #[test]
    fn convert_ocr_bbox_center_within_5pt_for_centered_block() {
        // Render: 612×792pt page → 1224×1584px image (2× DPI scale).
        // Block at image px (572, 712, w=80, h=40): pixel center (612, 732)
        // → PDF pre-flip (306, 366) → PDF post-flip (306, 792-366) = (306, 426).
        let conv =
            convert_ocr_bbox_to_pdf_points([572.0, 712.0, 80.0, 40.0], 1224, 1584, 612.0, 792.0);
        assert!(!conv.degenerate);
        assert_eq!(conv.coordinate_space, "image_px");
        let cx = (conv.bbox[0] + conv.bbox[2]) / 2.0;
        let cy = (conv.bbox[1] + conv.bbox[3]) / 2.0;
        assert!((cx - 306.0).abs() < 5.0, "cx={}", cx);
        assert!((cy - 426.0).abs() < 5.0, "cy={}", cy);
    }

    #[test]
    fn convert_ocr_bbox_top_left_of_image_maps_to_top_left_pdf() {
        // Top-left worker bbox should map to the top of the PDF page (max y).
        let conv =
            convert_ocr_bbox_to_pdf_points([0.0, 0.0, 100.0, 50.0], 1224, 1584, 612.0, 792.0);
        // After scaling: pixel y span 0..50 = PDF y span 0..25.
        // PDF flip: pdf_y0 = 792 - 25 = 767; pdf_y1 = 792 - 0 = 792.
        assert!((conv.bbox[1] - 767.0).abs() < 0.5);
        assert!((conv.bbox[3] - 792.0).abs() < 0.5);
        assert!((conv.bbox[0] - 0.0).abs() < 0.5);
        // x1 = 100 px * 0.5 = 50 pt
        assert!((conv.bbox[2] - 50.0).abs() < 0.5);
    }

    #[test]
    fn convert_ocr_bbox_zero_image_dims_falls_back_to_pdf_points() {
        let conv = convert_ocr_bbox_to_pdf_points([10.0, 100.0, 50.0, 20.0], 0, 0, 612.0, 792.0);
        assert_eq!(conv.coordinate_space, "unknown_assumed_pdf_points");
        assert_eq!(conv.conversion_scale, [1.0, 1.0]);
    }

    #[test]
    fn convert_ocr_bbox_off_page_is_clamped() {
        // x extends past page width → clamped + flag set.
        let conv =
            convert_ocr_bbox_to_pdf_points([1200.0, 0.0, 100.0, 20.0], 1224, 1584, 612.0, 792.0);
        assert!(conv.clamped);
        assert!(conv.bbox[0] <= 612.0);
        assert!(conv.bbox[2] <= 612.0);
    }

    #[test]
    fn convert_ocr_bbox_collapsed_after_clamp_marked_degenerate() {
        // Entirely off-page → after clamp all coords sit on the page edge → zero area.
        let conv =
            convert_ocr_bbox_to_pdf_points([2000.0, 0.0, 50.0, 20.0], 1224, 1584, 612.0, 792.0);
        assert!(conv.degenerate);
    }

    #[test]
    fn convert_ocr_bbox_keeps_rect_within_page_bounds() {
        let conv =
            convert_ocr_bbox_to_pdf_points([200.0, 200.0, 400.0, 100.0], 1224, 1584, 612.0, 792.0);
        // After scaling: 100×50 pt rect; should sit comfortably inside the page.
        assert!(conv.bbox[0] >= 0.0);
        assert!(conv.bbox[1] >= 0.0);
        assert!(conv.bbox[2] <= 612.0);
        assert!(conv.bbox[3] <= 792.0);
    }
}
