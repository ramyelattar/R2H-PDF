use std::fmt;

#[derive(Debug)]
pub enum AnnotationCoreError {
    SessionNotFound(String),
    AnnotationNotFound(String),
    InvalidType(String),
    ExportFailed(String),
}

impl fmt::Display for AnnotationCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationCoreError::SessionNotFound(id) => {
                write!(f, "session not found: {id}")
            }
            AnnotationCoreError::AnnotationNotFound(id) => {
                write!(f, "annotation not found: {id}")
            }
            AnnotationCoreError::InvalidType(t) => write!(f, "invalid annotation type: {t}"),
            AnnotationCoreError::ExportFailed(msg) => write!(f, "FDF export failed: {msg}"),
        }
    }
}

impl std::error::Error for AnnotationCoreError {}

impl From<AnnotationCoreError> for String {
    fn from(e: AnnotationCoreError) -> Self {
        e.to_string()
    }
}
