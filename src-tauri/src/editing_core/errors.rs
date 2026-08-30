use std::fmt;

#[derive(Debug)]
pub enum EditingCoreError {
    SessionNotFound(String),
    InvalidOperation(String),
    TransactionFailed(String),
    SnapshotStorageFailed(String),
    SnapshotUnavailable(String),
    UndoStackEmpty,
    RedoStackEmpty,
}

impl fmt::Display for EditingCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditingCoreError::SessionNotFound(id) => {
                write!(f, "session not found: {id}")
            }
            EditingCoreError::InvalidOperation(msg) => {
                write!(f, "invalid operation: {msg}")
            }
            EditingCoreError::TransactionFailed(msg) => {
                write!(f, "transaction failed: {msg}")
            }
            EditingCoreError::SnapshotStorageFailed(msg) => {
                write!(f, "snapshot storage failed: {msg}")
            }
            EditingCoreError::SnapshotUnavailable(msg) => {
                write!(f, "undo snapshot unavailable: {msg}")
            }
            EditingCoreError::UndoStackEmpty => write!(f, "nothing to undo"),
            EditingCoreError::RedoStackEmpty => write!(f, "nothing to redo"),
        }
    }
}

impl std::error::Error for EditingCoreError {}

impl From<EditingCoreError> for String {
    fn from(e: EditingCoreError) -> Self {
        e.to_string()
    }
}
