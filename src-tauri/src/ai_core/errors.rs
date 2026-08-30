use std::fmt;

#[derive(Debug)]
pub enum AiCoreError {
    Unavailable,
    TaskNotFound(String),
    TaskFailed(String),
    Disabled,
    HttpError(String),
    Timeout,
    ParseError(String),
}

impl fmt::Display for AiCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiCoreError::Unavailable => write!(f, "AI backend is unavailable"),
            AiCoreError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            AiCoreError::TaskFailed(msg) => write!(f, "Task failed: {}", msg),
            AiCoreError::Disabled => write!(f, "AI_DISABLED"),
            AiCoreError::HttpError(msg) => write!(f, "AI_HTTP_ERROR: {}", msg),
            AiCoreError::Timeout => write!(f, "AI_TIMEOUT"),
            AiCoreError::ParseError(msg) => write!(f, "AI_PARSE_ERROR: {}", msg),
        }
    }
}

impl std::error::Error for AiCoreError {}

impl From<AiCoreError> for String {
    fn from(e: AiCoreError) -> Self {
        e.to_string()
    }
}
