use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum DocumentCoreError {
    InvalidPdf(String),
    Io(String),
    SessionNotFound(String),
    PageOutOfRange {
        requested: usize,
        total: usize,
    },
    RenderError(String),
    TextExtractionError(String),
    SaveError(String),
    RecoveryFailed(String),
    LockPoisoned,
    MergeError(String),
    SplitError(String),
    WatermarkError(String),
    OcrError(String),
    VectorExtraction {
        code: VectorErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorErrorCode {
    EngineUnavailable,
    ResponseInvalid,
    GeometryInvalid,
}

impl Display for DocumentCoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentCoreError::InvalidPdf(msg) => write!(f, "invalid PDF: {msg}"),
            DocumentCoreError::Io(msg) => write!(f, "io error: {msg}"),
            DocumentCoreError::SessionNotFound(id) => write!(f, "session not found: {id}"),
            DocumentCoreError::PageOutOfRange { requested, total } => {
                write!(f, "page out of range: requested {requested}, total {total}")
            }
            DocumentCoreError::RenderError(msg) => write!(f, "render error: {msg}"),
            DocumentCoreError::TextExtractionError(msg) => {
                write!(f, "text extraction error: {msg}")
            }
            DocumentCoreError::SaveError(msg) => write!(f, "save error: {msg}"),
            DocumentCoreError::RecoveryFailed(msg) => write!(f, "recovery failed: {msg}"),
            DocumentCoreError::LockPoisoned => write!(f, "concurrent lock poisoned"),
            DocumentCoreError::MergeError(msg) => write!(f, "merge error: {msg}"),
            DocumentCoreError::SplitError(msg) => write!(f, "split error: {msg}"),
            DocumentCoreError::WatermarkError(msg) => write!(f, "watermark error: {msg}"),
            DocumentCoreError::OcrError(msg) => write!(f, "ocr error: {msg}"),
            DocumentCoreError::VectorExtraction { message, .. } => {
                write!(f, "native vector extraction error: {message}")
            }
        }
    }
}

impl std::error::Error for DocumentCoreError {}

impl From<std::io::Error> for DocumentCoreError {
    fn from(value: std::io::Error) -> Self {
        DocumentCoreError::Io(value.to_string())
    }
}
