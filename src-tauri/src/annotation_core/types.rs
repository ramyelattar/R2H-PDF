use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AnnotationType {
    Highlight,
    Underline,
    Strikeout,
    Squiggly,
    FreeText,
    Stamp,
    Ink,
    Link,
    Note,
    FileAttachment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub session_id: String,
    pub page_index: usize,
    pub annot_type: AnnotationType,
    pub color: AnnotationColor,
    pub contents: String,
    pub author: String,
    /// Bounding rectangle [x0, y0, x1, y1]
    pub rect: [f32; 4],
    pub created_at: String,
    pub modified_at: String,
    /// Callout line points for FreeText callout annotations
    pub callout_points: Option<Vec<[f32; 2]>>,
    /// Stamp name for Stamp annotations (e.g. "Approved", "Confidential", "Draft")
    pub stamp_name: Option<String>,
    /// Ink stroke paths for Ink annotations
    pub ink_paths: Option<Vec<Vec<[f32; 2]>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAnnotationRequest {
    pub session_id: String,
    pub source_path: Option<String>,
    pub page_index: usize,
    pub annot_type: AnnotationType,
    pub color: AnnotationColor,
    pub contents: String,
    pub author: String,
    pub rect: [f32; 4],
    /// Callout line points for FreeText callout annotations
    pub callout_points: Option<Vec<[f32; 2]>>,
    /// Stamp name for Stamp annotations (e.g. "Approved", "Confidential", "Draft")
    pub stamp_name: Option<String>,
    /// Ink stroke paths for Ink annotations
    pub ink_paths: Option<Vec<Vec<[f32; 2]>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAnnotationRequest {
    pub session_id: String,
    pub source_path: Option<String>,
    pub annotation_id: String,
    pub color: Option<AnnotationColor>,
    pub contents: Option<String>,
    pub rect: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationListResponse {
    pub session_id: String,
    pub page_index: Option<usize>,
    pub annotations: Vec<Annotation>,
    pub total: usize,
}
