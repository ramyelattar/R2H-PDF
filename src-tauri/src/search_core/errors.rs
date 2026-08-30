use std::fmt;

#[derive(Debug)]
pub enum SearchCoreError {
    SessionNotFound(String),
    InvalidQuery(String),
    IndexNotReady(String),
    InvalidRegex(String),
    RegexTooComplex,
}

impl fmt::Display for SearchCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchCoreError::SessionNotFound(id) => write!(f, "session not found: {id}"),
            SearchCoreError::InvalidQuery(msg) => write!(f, "invalid query: {msg}"),
            SearchCoreError::IndexNotReady(id) => write!(f, "index not ready for session: {id}"),
            SearchCoreError::InvalidRegex(msg) => write!(f, "INVALID_REGEX: {msg}"),
            SearchCoreError::RegexTooComplex => write!(f, "REGEX_TOO_COMPLEX"),
        }
    }
}

impl std::error::Error for SearchCoreError {}

impl From<SearchCoreError> for String {
    fn from(e: SearchCoreError) -> Self {
        e.to_string()
    }
}
