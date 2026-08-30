use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::document_core::errors::DocumentCoreError;
use crate::document_core::session::DocumentCoreState;
use crate::document_core::types::{
    BBox, OpenDocumentRequest, RenderRequest, TextExtractionRequest,
};
use crate::document_core::vector::{NativeVectorRequest, VECTOR_PROFILE_ID};

pub const PROTOCOL_VERSION: &str = "r2h-documentengine-host-v1";
const ARTIFACT_ROOT_ENV: &str = "R2H_DOCUMENTENGINE_ARTIFACT_ROOT";
const COORDINATE_CONVENTION: &str = "pdf_points_origin_bottom_left";
const MAX_RENDER_SCALE: f32 = 4.0;
const MIN_RENDER_SCALE: f32 = 0.25;
const MAX_DEVICE_PIXEL_RATIO: f32 = 2.0;
static HOST_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenDocumentParams {
    path: String,
    #[serde(default)]
    recover_if_damaged: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionParams {
    document_session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageParams {
    document_session_id: String,
    page_index: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VectorParams {
    document_session_id: String,
    page_index: i64,
    profile_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderParams {
    document_session_id: String,
    page_index: i64,
    scale: Option<f32>,
    dpi: Option<f32>,
    device_pixel_ratio: Option<f32>,
    pixel_format: Option<String>,
    viewport: Option<Value>,
    // Deliberately ignored. The host never accepts a caller-controlled output
    // path; artifacts are always generated below the trusted root.
    #[allow(dead_code)]
    destination: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum DocumentOperation {
    Open,
    Metadata,
    Text,
    Render,
    Close,
    Vector,
}

pub struct HostRuntime {
    document_state: DocumentCoreState,
    host_sessions: HashMap<String, String>,
    closed_sessions: HashSet<String>,
    artifact_root: PathBuf,
    remove_artifact_root_on_drop: bool,
    next_artifact_id: u64,
}

impl HostRuntime {
    pub fn new() -> io::Result<Self> {
        let (artifact_root, remove_artifact_root_on_drop) = artifact_root()?;
        Ok(Self {
            document_state: DocumentCoreState::new(),
            host_sessions: HashMap::new(),
            closed_sessions: HashSet::new(),
            artifact_root,
            remove_artifact_root_on_drop,
            next_artifact_id: 1,
        })
    }

    pub fn handle_line(&mut self, line: &str) -> String {
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return error(None, "INVALID_REQUEST"),
        };
        let request = match value.as_object() {
            Some(value) => value,
            None => return error(None, "INVALID_REQUEST"),
        };
        let request_id = match valid_request_id(request.get("requestId")) {
            Some(value) => value,
            None => return error(None, "INVALID_REQUEST"),
        };
        let version = match request.get("protocolVersion").and_then(Value::as_str) {
            Some(value) if value == PROTOCOL_VERSION => value,
            _ => return error(Some(request_id), "UNSUPPORTED_PROTOCOL_VERSION"),
        };
        let method = match request.get("method").and_then(Value::as_str) {
            Some(value) => value,
            None => return error(Some(request_id), "MALFORMED_PARAMS"),
        };
        let params = match request.get("params") {
            Some(value) if value.is_object() => value,
            _ => return error(Some(request_id), "MALFORMED_PARAMS"),
        };

        let result = match method {
            "HELLO" => Ok(json!({
                "protocolVersion": version,
                "hostVersion": env!("CARGO_PKG_VERSION"),
                "engineVersion": env!("CARGO_PKG_VERSION"),
                "capabilities": {
                    "nativeText": true,
                    "pageRender": true,
                    "vectorRead": true
                }
            })),
            "HEALTH" => Ok(json!({
                "status": "READY",
                "protocolVersion": version
            })),
            "OPEN_DOCUMENT" => self.open_document(params),
            "CLOSE_DOCUMENT" => self.close_document(params),
            "GET_DOCUMENT_INFO" => self.get_document_info(params),
            "GET_PAGE_INFO" => self.get_page_info(params),
            "EXTRACT_NATIVE_TEXT" => self.extract_native_text(params),
            "EXTRACT_NATIVE_VECTORS" => self.extract_native_vectors(params),
            "RENDER_PAGE" => self.render_page(params),
            _ => Err("UNSUPPORTED_METHOD"),
        };

        match result {
            Ok(result) => response(request_id, result),
            Err(code) => error(Some(request_id), code),
        }
    }

    fn open_document(&mut self, params: &Value) -> Result<Value, &'static str> {
        let params: OpenDocumentParams = parse_params(params)?;
        let path = PathBuf::from(&params.path);
        if params.path.trim().is_empty() || !path.is_absolute() {
            return Err("DOCUMENT_OPEN_FAILED");
        }
        if !path.exists() {
            return Err("DOCUMENT_NOT_FOUND");
        }
        if !path.is_file() {
            return Err("DOCUMENT_OPEN_FAILED");
        }

        let opened = self
            .document_state
            .store
            .open_document(OpenDocumentRequest {
                path: params.path,
                recover_if_damaged: params.recover_if_damaged,
            })
            .map_err(|error| map_core_error(error, DocumentOperation::Open))?;

        let internal_session_id = opened.session_id;
        let host_session_id = self.new_host_session_id(&internal_session_id);
        self.host_sessions
            .insert(host_session_id.clone(), internal_session_id);

        Ok(json!({
            "documentSessionId": host_session_id,
            "pageCount": opened.summary.page_count,
            "recovered": opened.recovered
        }))
    }

    fn close_document(&mut self, params: &Value) -> Result<Value, &'static str> {
        let params: SessionParams = parse_params(params)?;
        let internal_session_id = self.resolve_session(&params.document_session_id)?;
        self.document_state
            .store
            .close_document(&internal_session_id)
            .map_err(|error| map_core_error(error, DocumentOperation::Close))?;
        self.host_sessions.remove(&params.document_session_id);
        self.closed_sessions
            .insert(params.document_session_id.clone());
        Ok(json!({ "closed": true }))
    }

    fn get_document_info(&self, params: &Value) -> Result<Value, &'static str> {
        let params: SessionParams = parse_params(params)?;
        self.with_session(&params.document_session_id, |session| {
            let summary = &session.document.summary;
            Ok(json!({
                "pageCount": summary.page_count,
                "objectCount": summary.object_count,
                "metadata": {
                    "title": summary.title,
                    "author": summary.author,
                    "producer": summary.producer
                }
            }))
        })
    }

    fn get_page_info(&self, params: &Value) -> Result<Value, &'static str> {
        let params: PageParams = parse_params(params)?;
        let page_index = page_index(params.page_index)?;
        self.with_session(&params.document_session_id, |session| {
            let page = session
                .document
                .pages
                .get(page_index)
                .ok_or("PAGE_NOT_FOUND")?;
            Ok(json!({
                "pageIndex": page.index,
                "widthPt": page.width_points,
                "heightPt": page.height_points,
                "rotation": page.rotation,
                "hasText": page.has_text
            }))
        })
    }

    fn extract_native_text(&self, params: &Value) -> Result<Value, &'static str> {
        let params: PageParams = parse_params(params)?;
        let page_index = page_index(params.page_index)?;
        let internal_session_id = self.resolve_session(&params.document_session_id)?;
        let extracted = self
            .document_state
            .store
            .extract_text(TextExtractionRequest {
                session_id: internal_session_id,
                page_index,
                region: None,
            })
            .map_err(|error| map_core_error(error, DocumentOperation::Text))?;

        let spans: Vec<Value> = extracted
            .spans
            .into_iter()
            .map(|span| {
                json!({
                    "content": span.content,
                    "bbox": {
                        "x": span.bbox.x,
                        "y": span.bbox.y,
                        "width": span.bbox.width,
                        "height": span.bbox.height
                    },
                    "fontName": span.font_name
                })
            })
            .collect();
        let mut warnings = Vec::new();
        if !spans.is_empty() {
            warnings.push(
                "existing core exposes page-level native text geometry rather than per-run geometry"
                    .to_string(),
            );
        }

        Ok(json!({
            "pageIndex": extracted.page_index,
            "fullText": extracted.full_text,
            "spans": spans,
            "coordinateConvention": COORDINATE_CONVENTION,
            "warnings": warnings
        }))
    }

    fn extract_native_vectors(&self, params: &Value) -> Result<Value, &'static str> {
        let params: VectorParams = serde_json::from_value(params.clone())
            .map_err(|_| "VECTOR_INPUT_INVALID")?;
        if params.profile_version != VECTOR_PROFILE_ID {
            return Err("VECTOR_INPUT_INVALID");
        }
        let page_index = page_index(params.page_index)?;
        let internal_session_id = self.resolve_session(&params.document_session_id)?;
        let result = self
            .document_state
            .store
            .extract_native_vectors(NativeVectorRequest {
                session_id: internal_session_id,
                page_index,
            })
            .map_err(|error| map_core_error(error, DocumentOperation::Vector))?;
        serde_json::to_value(result).map_err(|_| "VECTOR_RESPONSE_INVALID")
    }

    fn render_page(&mut self, params: &Value) -> Result<Value, &'static str> {
        let params: RenderParams = parse_params(params)?;
        if params.destination.is_some() {
            return Err("UNSUPPORTED_OPERATION");
        }
        if params.viewport.is_some() {
            return Err("UNSUPPORTED_OPERATION");
        }
        let page_index = page_index(params.page_index)?;
        let scale = render_scale(params.scale, params.dpi)?;
        let device_pixel_ratio = params.device_pixel_ratio.unwrap_or(1.0);
        if !device_pixel_ratio.is_finite()
            || !(1.0..=MAX_DEVICE_PIXEL_RATIO).contains(&device_pixel_ratio)
        {
            return Err("MALFORMED_PARAMS");
        }
        if params.pixel_format.as_deref().unwrap_or("rgba8") != "rgba8" {
            return Err("UNSUPPORTED_OPERATION");
        }

        let internal_session_id = self.resolve_session(&params.document_session_id)?;
        let (page_width, page_height, rotation) =
            self.with_session(&params.document_session_id, |session| {
                let page = session
                    .document
                    .pages
                    .get(page_index)
                    .ok_or("PAGE_NOT_FOUND")?;
                Ok((page.width_points, page.height_points, page.rotation))
            })?;

        let rendered = self
            .document_state
            .store
            .render_page(RenderRequest {
                session_id: internal_session_id,
                page_index,
                zoom: scale,
                viewport: BBox {
                    x: 0.0,
                    y: 0.0,
                    width: page_width,
                    height: page_height,
                },
                device_pixel_ratio,
            })
            .map_err(|error| map_core_error(error, DocumentOperation::Render))?;

        let artifact_token = format!("render-{:016x}.rgba8", self.next_artifact_id);
        self.next_artifact_id = self.next_artifact_id.wrapping_add(1);
        let artifact_path = self.contained_artifact_path(&artifact_token)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&artifact_path)
            .map_err(|_| "RENDER_FAILED")?;
        file.write_all(&rendered.pixels_rgba)
            .map_err(|_| "RENDER_FAILED")?;
        file.flush().map_err(|_| "RENDER_FAILED")?;

        let digest = Sha256::digest(&rendered.pixels_rgba);
        Ok(json!({
            "artifactToken": artifact_token,
            "sha256": format!("{digest:x}"),
            "byteLength": rendered.pixels_rgba.len(),
            "pixelWidth": rendered.width_px,
            "pixelHeight": rendered.height_px,
            "pageWidthPt": rendered.width_pts,
            "pageHeightPt": rendered.height_pts,
            "rotation": rotation,
            "pixelFormat": "rgba8",
            "warnings": Vec::<String>::new()
        }))
    }

    fn resolve_session(&self, host_session_id: &str) -> Result<String, &'static str> {
        if self.closed_sessions.contains(host_session_id) {
            return Err("SESSION_CLOSED");
        }
        self.host_sessions
            .get(host_session_id)
            .cloned()
            .ok_or("SESSION_NOT_FOUND")
    }

    fn with_session<T, F>(&self, host_session_id: &str, operation: F) -> Result<T, &'static str>
    where
        F: FnOnce(&crate::document_core::session::DocumentSession) -> Result<T, &'static str>,
    {
        let internal_session_id = self.resolve_session(host_session_id)?;
        let session = self
            .document_state
            .store
            .get_session_arc_pub(&internal_session_id)
            .map_err(|error| map_core_error(error, DocumentOperation::Metadata))?;
        let session = session
            .lock()
            .map_err(|_| "INTERNAL_DOCUMENTENGINE_ERROR")?;
        operation(&session)
    }

    fn new_host_session_id(&self, internal_session_id: &str) -> String {
        let nonce = HOST_NONCE.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let seed = format!(
            "{}:{}:{}:{}",
            std::process::id(),
            now,
            nonce,
            internal_session_id
        );
        let digest = Sha256::digest(seed.as_bytes());
        format!("r2h-session-{}", &format!("{digest:x}")[..32])
    }

    fn contained_artifact_path(&self, artifact_token: &str) -> Result<PathBuf, &'static str> {
        let token_path = Path::new(artifact_token);
        if token_path.is_absolute()
            || token_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("RENDER_FAILED");
        }
        let root = fs::canonicalize(&self.artifact_root).map_err(|_| "RENDER_FAILED")?;
        let candidate = root.join(token_path);
        let parent = candidate.parent().ok_or("RENDER_FAILED")?;
        let canonical_parent = fs::canonicalize(parent).map_err(|_| "RENDER_FAILED")?;
        if canonical_parent != root {
            return Err("RENDER_FAILED");
        }
        Ok(candidate)
    }
}

impl Drop for HostRuntime {
    fn drop(&mut self) {
        if self.remove_artifact_root_on_drop {
            let _ = fs::remove_dir_all(&self.artifact_root);
        }
    }
}

pub fn handle_line(line: &str) -> String {
    match HostRuntime::new() {
        Ok(mut runtime) => runtime.handle_line(line),
        Err(_) => error(None, "INTERNAL_DOCUMENTENGINE_ERROR"),
    }
}

pub fn run_stdio() -> io::Result<()> {
    let mut runtime = HostRuntime::new()?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        writeln!(output, "{}", runtime.handle_line(&line))?;
        output.flush()?;
    }
    Ok(())
}

fn artifact_root() -> io::Result<(PathBuf, bool)> {
    if let Ok(configured) = std::env::var(ARTIFACT_ROOT_ENV) {
        let configured = PathBuf::from(configured);
        if !configured.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "artifact root must be absolute",
            ));
        }
        fs::create_dir_all(&configured)?;
        return Ok((fs::canonicalize(configured)?, false));
    }

    let nonce = HOST_NONCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "r2h-documentengine-host-{}-{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&root)?;
    Ok((fs::canonicalize(root)?, true))
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, &'static str> {
    serde_json::from_value(params.clone()).map_err(|_| "MALFORMED_PARAMS")
}

fn page_index(value: i64) -> Result<usize, &'static str> {
    usize::try_from(value).map_err(|_| "PAGE_NOT_FOUND")
}

fn render_scale(scale: Option<f32>, dpi: Option<f32>) -> Result<f32, &'static str> {
    if scale.is_some() && dpi.is_some() {
        return Err("MALFORMED_PARAMS");
    }
    let scale = scale
        .or_else(|| dpi.map(|value| value / 72.0))
        .unwrap_or(1.0);
    if !scale.is_finite() || !(MIN_RENDER_SCALE..=MAX_RENDER_SCALE).contains(&scale) {
        return Err("MALFORMED_PARAMS");
    }
    Ok(scale)
}

fn map_core_error(error: DocumentCoreError, operation: DocumentOperation) -> &'static str {
    match error {
        DocumentCoreError::SessionNotFound(_) => "SESSION_NOT_FOUND",
        DocumentCoreError::PageOutOfRange { .. } => "PAGE_NOT_FOUND",
        DocumentCoreError::InvalidPdf(_) => match operation {
            DocumentOperation::Open => "INVALID_DOCUMENT",
            _ => "INTERNAL_DOCUMENTENGINE_ERROR",
        },
        DocumentCoreError::Io(_) => match operation {
            DocumentOperation::Open => "DOCUMENT_OPEN_FAILED",
            _ => "INTERNAL_DOCUMENTENGINE_ERROR",
        },
        DocumentCoreError::TextExtractionError(_) => "TEXT_EXTRACTION_FAILED",
        DocumentCoreError::VectorExtraction { code, .. } => match code {
            crate::document_core::errors::VectorErrorCode::EngineUnavailable => {
                "VECTOR_ENGINE_UNAVAILABLE"
            }
            crate::document_core::errors::VectorErrorCode::ResponseInvalid => {
                "VECTOR_RESPONSE_INVALID"
            }
            crate::document_core::errors::VectorErrorCode::GeometryInvalid => {
                "VECTOR_GEOMETRY_INVALID"
            }
        },
        DocumentCoreError::RenderError(_) => match operation {
            DocumentOperation::Open => "INVALID_DOCUMENT",
            DocumentOperation::Render => "RENDER_FAILED",
            _ => "INTERNAL_DOCUMENTENGINE_ERROR",
        },
        DocumentCoreError::LockPoisoned
        | DocumentCoreError::SaveError(_)
        | DocumentCoreError::RecoveryFailed(_)
        | DocumentCoreError::MergeError(_)
        | DocumentCoreError::SplitError(_)
        | DocumentCoreError::WatermarkError(_)
        | DocumentCoreError::OcrError(_) => "INTERNAL_DOCUMENTENGINE_ERROR",
    }
}

fn response(request_id: &str, result: Value) -> String {
    json!({ "requestId": request_id, "result": result }).to_string()
}

fn error(request_id: Option<&str>, code: &str) -> String {
    json!({ "requestId": request_id, "error": { "code": code } }).to_string()
}

fn valid_request_id(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 128)
}
