use std::cell::RefCell;
use std::rc::Rc;

use mupdf::{
    Colorspace, Device, LineCap, LineJoin, Matrix, NativeDevice, Path, PathWalker, Point,
    StrokeState,
};
use serde::{Deserialize, Serialize};

use super::engine::OpenedDocument;
use super::errors::{DocumentCoreError, VectorErrorCode};

pub const VECTOR_PROFILE_ID: &str = "documentengine-native-vector-read-v1";
pub const VECTOR_PROFILE_VERSION: u32 = 1;
const MAX_PATHS: usize = 100_000;
const MAX_COMMANDS: usize = 2_000_000;
const PAGE_EPSILON: f64 = 0.0001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeVectorRequest {
    pub session_id: String,
    pub page_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVectorPageResult {
    pub page_index: usize,
    pub page_geometry: VectorPageGeometry,
    pub coordinate_space: &'static str,
    pub paths: Vec<NativeVectorPath>,
    pub profile: VectorProfile,
    pub engine_identity: VectorEngineIdentity,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPageGeometry {
    pub width_pt: f32,
    pub height_pt: f32,
    pub rotation: i32,
    pub origin: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVectorPath {
    pub source_order: usize,
    pub commands: Vec<NativeVectorCommand>,
    pub bounds: VectorBounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<VectorStrokeMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<VectorFillMetadata>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NativeVectorCommand {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    CurveTo {
        c1x: f64,
        c1y: f64,
        c2x: f64,
        c2y: f64,
        x: f64,
        y: f64,
    },
    ClosePath,
    Rect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorStrokeMetadata {
    pub present: bool,
    pub width: f64,
    pub cap: String,
    pub dash_cap: String,
    pub end_cap: String,
    pub join: String,
    pub miter_limit: f64,
    pub dash_phase: f64,
    pub dash: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<VectorColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorFillMetadata {
    pub present: bool,
    pub even_odd: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<VectorColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorColor {
    pub color_space: String,
    pub components: Vec<f64>,
    pub alpha: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorProfile {
    pub profile_id: &'static str,
    pub version: u32,
    pub coordinate_space: &'static str,
    pub source_coordinate_convention: &'static str,
    pub coordinate_transform: &'static str,
    pub path_command_preservation: &'static str,
    pub ordering: &'static str,
    pub unsupported_primitive_policy: &'static str,
    pub clipping_provenance: &'static str,
    pub object_identity: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorEngineIdentity {
    pub product: &'static str,
    pub protocol_version: &'static str,
    pub engine_version: &'static str,
    pub mupdf_binding_version: &'static str,
    pub vector_capability: &'static str,
}

pub fn extract_native_vectors(
    doc: &OpenedDocument,
    request: &NativeVectorRequest,
) -> Result<NativeVectorPageResult, DocumentCoreError> {
    let page_info = doc
        .pages
        .get(request.page_index)
        .ok_or(DocumentCoreError::PageOutOfRange {
            requested: request.page_index,
            total: doc.pages.len(),
        })?;
    if !page_info.width_points.is_finite()
        || !page_info.height_points.is_finite()
        || page_info.width_points <= 0.0
        || page_info.height_points <= 0.0
    {
        return Err(vector_error(
            VectorErrorCode::GeometryInvalid,
            "page geometry is non-finite or non-positive",
        ));
    }

    let document = mupdf::Document::from_bytes(&doc.bytes, "pdf")
        .map_err(|error| vector_error(VectorErrorCode::EngineUnavailable, error.to_string()))?;
    let page =
        document
            .load_page(i32::try_from(request.page_index).map_err(|error| {
                vector_error(VectorErrorCode::GeometryInvalid, error.to_string())
            })?)
            .map_err(|error| vector_error(VectorErrorCode::EngineUnavailable, error.to_string()))?;

    let collector = Rc::new(RefCell::new(VectorCollector::new(
        page_info.width_points as f64,
        page_info.height_points as f64,
    )));
    let device = Device::from_native(SharedCollector(Rc::clone(&collector)))
        .map_err(|error| vector_error(VectorErrorCode::EngineUnavailable, error.to_string()))?;
    page.run_contents(&device, &Matrix::IDENTITY)
        .map_err(|error| vector_error(VectorErrorCode::EngineUnavailable, error.to_string()))?;
    drop(device);

    let collector = Rc::try_unwrap(collector)
        .map_err(|_| {
            vector_error(
                VectorErrorCode::EngineUnavailable,
                "vector collector remained referenced",
            )
        })?
        .into_inner();
    let (paths, warnings) = collector.finish()?;

    Ok(NativeVectorPageResult {
        page_index: request.page_index,
        page_geometry: VectorPageGeometry {
            width_pt: page_info.width_points,
            height_pt: page_info.height_points,
            rotation: page_info.rotation,
            origin: "bottom_left",
        },
        coordinate_space: "PDF_PAGE",
        paths,
        profile: VectorProfile {
            profile_id: VECTOR_PROFILE_ID,
            version: VECTOR_PROFILE_VERSION,
            coordinate_space: "PDF_PAGE",
            source_coordinate_convention: "mupdf_page_top_left",
            coordinate_transform: "x=source_x; y=page_height-source_y; no_display_rotation",
            path_command_preservation: "native_move_line_curve_close_rect",
            ordering: "native_device_execution_order",
            unsupported_primitive_policy: "omit_text_image_shade_and_non_path_operations",
            clipping_provenance: "not_available; clipping callbacks are warnings only",
            object_identity: "source_order_only; no_stable_pdf_object_id_exposed",
        },
        engine_identity: VectorEngineIdentity {
            product: "R2H-PDF",
            protocol_version: "r2h-documentengine-host-v1",
            engine_version: env!("CARGO_PKG_VERSION"),
            mupdf_binding_version: "0.6.0",
            vector_capability: "vectorRead",
        },
        warnings,
    })
}

fn vector_error(code: VectorErrorCode, message: impl Into<String>) -> DocumentCoreError {
    DocumentCoreError::VectorExtraction {
        code,
        message: message.into(),
    }
}

struct VectorCollector {
    page_width: f64,
    page_height: f64,
    paths: Vec<NativeVectorPath>,
    command_count: usize,
    warnings: Vec<String>,
    clipping_warning_recorded: bool,
    failure: Option<DocumentCoreError>,
}

impl VectorCollector {
    fn new(page_width: f64, page_height: f64) -> Self {
        Self {
            page_width,
            page_height,
            paths: Vec::new(),
            command_count: 0,
            warnings: Vec::new(),
            clipping_warning_recorded: false,
            failure: None,
        }
    }

    fn finish(self) -> Result<(Vec<NativeVectorPath>, Vec<String>), DocumentCoreError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        Ok((self.paths, self.warnings))
    }

    fn add_warning(&mut self, warning: &'static str) {
        if !self.warnings.iter().any(|value| value == warning) {
            self.warnings.push(warning.to_string());
        }
    }

    fn record_clipping_warning(&mut self) {
        if !self.clipping_warning_recorded {
            self.clipping_warning_recorded = true;
            self.add_warning("CLIPPING_PROVENANCE_NOT_AVAILABLE");
        }
    }

    fn push_path(
        &mut self,
        path: &Path,
        cmt: Matrix,
        stroke: Option<VectorStrokeMetadata>,
        fill: Option<VectorFillMetadata>,
        warnings: Vec<String>,
    ) {
        if self.failure.is_some() {
            return;
        }
        if self.paths.len() >= MAX_PATHS {
            self.failure = Some(vector_error(
                VectorErrorCode::ResponseInvalid,
                "native vector path response limit exceeded",
            ));
            return;
        }
        let mut walker = PathBuilder::new(self.page_width, self.page_height, cmt);
        if let Err(error) = path.walk(&mut walker) {
            self.failure = Some(vector_error(
                VectorErrorCode::EngineUnavailable,
                error.to_string(),
            ));
            return;
        }
        self.command_count = self.command_count.saturating_add(walker.command_count);
        if self.command_count > MAX_COMMANDS {
            self.failure = Some(vector_error(
                VectorErrorCode::ResponseInvalid,
                "native vector command response limit exceeded",
            ));
            return;
        }
        match walker.finish(self.paths.len(), stroke, fill, warnings) {
            Ok(path) => self.paths.push(path),
            Err(error) => self.failure = Some(error),
        }
    }
}

struct SharedCollector(Rc<RefCell<VectorCollector>>);

impl NativeDevice for SharedCollector {
    fn fill_path(
        &mut self,
        path: &Path,
        even_odd: bool,
        cmt: Matrix,
        color_space: &Colorspace,
        color: &[f32],
        alpha: f32,
        _cp: mupdf::ColorParams,
    ) {
        let (color, color_warning) = vector_color(color_space, color, alpha);
        let mut warnings = Vec::new();
        if let Some(warning) = color_warning {
            warnings.push(warning.to_string());
        }
        let metadata = VectorFillMetadata {
            present: true,
            even_odd,
            color,
        };
        self.0
            .borrow_mut()
            .push_path(path, cmt, None, Some(metadata), warnings);
    }

    fn stroke_path(
        &mut self,
        path: &Path,
        stroke_state: &StrokeState,
        cmt: Matrix,
        color_space: &Colorspace,
        color: &[f32],
        alpha: f32,
        _cp: mupdf::ColorParams,
    ) {
        let dashes: Vec<f64> = stroke_state
            .dashes()
            .into_iter()
            .map(|value| value as f64)
            .collect();
        let width = stroke_state.line_width() as f64;
        let miter_limit = stroke_state.miter_limit() as f64;
        let dash_phase = stroke_state.dash_phase() as f64;
        let mut warnings = Vec::new();
        let numeric_metadata_valid = [width, miter_limit, dash_phase]
            .iter()
            .all(|value| value.is_finite())
            && dashes.iter().all(|value| value.is_finite());
        if !numeric_metadata_valid {
            warnings.push("STROKE_METADATA_NOT_AVAILABLE".to_string());
        }
        let (color, color_warning) = vector_color(color_space, color, alpha);
        if let Some(warning) = color_warning {
            warnings.push(warning.to_string());
        }
        let metadata = numeric_metadata_valid.then(|| VectorStrokeMetadata {
            present: true,
            width,
            cap: line_cap_name(stroke_state.start_cap()),
            dash_cap: line_cap_name(stroke_state.dash_cap()),
            end_cap: line_cap_name(stroke_state.end_cap()),
            join: line_join_name(stroke_state.line_join()),
            miter_limit,
            dash_phase,
            dash: dashes,
            color,
        });
        self.0
            .borrow_mut()
            .push_path(path, cmt, metadata, None, warnings);
    }

    fn clip_path(&mut self, _path: &Path, _even_odd: bool, _cmt: Matrix, _scissor: mupdf::Rect) {
        self.0.borrow_mut().record_clipping_warning();
    }

    fn clip_stroke_path(
        &mut self,
        _path: &Path,
        _stroke_state: &StrokeState,
        _cmt: Matrix,
        _scissor: mupdf::Rect,
    ) {
        self.0.borrow_mut().record_clipping_warning();
    }
}

fn vector_color(
    colorspace: &Colorspace,
    color: &[f32],
    alpha: f32,
) -> (Option<VectorColor>, Option<&'static str>) {
    if !alpha.is_finite() || color.iter().any(|value| !value.is_finite()) {
        return (None, Some("COLOR_METADATA_NOT_AVAILABLE"));
    }
    if color.len() != colorspace.n() as usize {
        return (None, Some("COLOR_METADATA_NOT_AVAILABLE"));
    }
    (
        Some(VectorColor {
            color_space: colorspace.name().to_string(),
            components: color.iter().map(|value| *value as f64).collect(),
            alpha: alpha as f64,
        }),
        None,
    )
}

fn line_cap_name(cap: LineCap) -> String {
    match cap {
        LineCap::Butt => "BUTT",
        LineCap::Round => "ROUND",
        LineCap::Square => "SQUARE",
        LineCap::Triangle => "TRIANGLE",
    }
    .to_string()
}

fn line_join_name(join: LineJoin) -> String {
    match join {
        LineJoin::Miter => "MITER",
        LineJoin::Round => "ROUND",
        LineJoin::Bevel => "BEVEL",
        LineJoin::MiterXps => "MITER_XPS",
    }
    .to_string()
}

struct PathBuilder {
    page_width: f64,
    page_height: f64,
    cmt: Matrix,
    commands: Vec<NativeVectorCommand>,
    points: Vec<(f64, f64)>,
    command_count: usize,
    invalid: Option<String>,
}

impl PathBuilder {
    fn new(page_width: f64, page_height: f64, cmt: Matrix) -> Self {
        Self {
            page_width,
            page_height,
            cmt,
            commands: Vec::new(),
            points: Vec::new(),
            command_count: 0,
            invalid: None,
        }
    }

    fn add_command(&mut self, command: NativeVectorCommand) {
        if self.invalid.is_some() {
            return;
        }
        self.command_count = self.command_count.saturating_add(1);
        self.commands.push(command);
    }

    fn page_point(&mut self, x: f32, y: f32) -> Option<(f64, f64)> {
        if !x.is_finite() || !y.is_finite() {
            self.invalid = Some("native path coordinate is non-finite".to_string());
            return None;
        }
        if [
            self.cmt.a, self.cmt.b, self.cmt.c, self.cmt.d, self.cmt.e, self.cmt.f,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            self.invalid = Some("native path transform is non-finite".to_string());
            return None;
        }
        let transformed = Point::new(x, y).transform(&self.cmt);
        let point = (
            transformed.x as f64,
            self.page_height - transformed.y as f64,
        );
        if !point.0.is_finite() || !point.1.is_finite() {
            self.invalid = Some("native path transformed coordinate is non-finite".to_string());
            return None;
        }
        self.points.push(point);
        Some(point)
    }

    fn finish(
        self,
        source_order: usize,
        stroke: Option<VectorStrokeMetadata>,
        fill: Option<VectorFillMetadata>,
        warnings: Vec<String>,
    ) -> Result<NativeVectorPath, DocumentCoreError> {
        if let Some(message) = self.invalid {
            return Err(vector_error(VectorErrorCode::GeometryInvalid, message));
        }
        if self.commands.is_empty() || self.points.is_empty() {
            return Err(vector_error(
                VectorErrorCode::GeometryInvalid,
                "native path contained no geometry",
            ));
        }
        let (min_x, max_x) = self
            .points
            .iter()
            .map(|(x, _)| *x)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            });
        let (min_y, max_y) = self
            .points
            .iter()
            .map(|(_, y)| *y)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            });
        if ![min_x, min_y, max_x, max_y]
            .iter()
            .all(|value| value.is_finite())
            || min_x > max_x
            || min_y > max_y
            || min_x < -PAGE_EPSILON
            || min_y < -PAGE_EPSILON
            || max_x > self.page_width + PAGE_EPSILON
            || max_y > self.page_height + PAGE_EPSILON
        {
            return Err(vector_error(
                VectorErrorCode::GeometryInvalid,
                "native path geometry is outside page bounds",
            ));
        }
        Ok(NativeVectorPath {
            source_order,
            commands: self.commands,
            bounds: VectorBounds {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            },
            stroke,
            fill,
            warnings,
        })
    }
}

impl PathWalker for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        if let Some((x, y)) = self.page_point(x, y) {
            self.add_command(NativeVectorCommand::MoveTo { x, y });
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some((x, y)) = self.page_point(x, y) {
            self.add_command(NativeVectorCommand::LineTo { x, y });
        }
    }

    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, ex: f32, ey: f32) {
        let first = self.page_point(cx1, cy1);
        let second = self.page_point(cx2, cy2);
        let end = self.page_point(ex, ey);
        if let (Some((c1x, c1y)), Some((c2x, c2y)), Some((x, y))) = (first, second, end) {
            self.add_command(NativeVectorCommand::CurveTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            });
        }
    }

    fn close(&mut self) {
        self.add_command(NativeVectorCommand::ClosePath);
    }

    fn rect(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let first = self.page_point(x1, y1);
        let second = self.page_point(x2, y1);
        let third = self.page_point(x2, y2);
        let fourth = self.page_point(x1, y2);
        if let (Some((x1, y1)), Some((x2, _)), Some((_, y2)), Some((_, _))) =
            (first, second, third, fourth)
        {
            self.add_command(NativeVectorCommand::Rect { x1, y1, x2, y2 });
        }
    }
}
