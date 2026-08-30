use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum PlatformLayerError {
    InvalidInput(String),
    NotFound(String),
    PolicyViolation(String),
    SchedulingError(String),
    IntegrityError(String),
    LockPoisoned,
}

impl Display for PlatformLayerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformLayerError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            PlatformLayerError::NotFound(msg) => write!(f, "not found: {msg}"),
            PlatformLayerError::PolicyViolation(msg) => write!(f, "policy violation: {msg}"),
            PlatformLayerError::SchedulingError(msg) => write!(f, "scheduling error: {msg}"),
            PlatformLayerError::IntegrityError(msg) => write!(f, "integrity error: {msg}"),
            PlatformLayerError::LockPoisoned => write!(f, "concurrent lock poisoned"),
        }
    }
}

impl std::error::Error for PlatformLayerError {}
