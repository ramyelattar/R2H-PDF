use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Payload structs for each EditOperationType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertTextPayload {
    pub page_index: usize,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font: String,
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteTextPayload {
    pub page_index: usize,
    pub bbox: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceTextPayload {
    pub page_index: usize,
    pub bbox: [f32; 4],
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactPayload {
    pub page_index: usize,
    pub rect: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePagePayload {
    pub page_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertBlankPagePayload {
    pub at_index: usize,
    pub width_pts: f32,
    pub height_pts: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovePagePayload {
    pub from_index: usize,
    pub to_index: usize,
}

/// `degrees` must be one of 0, 90, 180, or 270.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotatePagePayload {
    pub page_index: usize,
    pub degrees: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillFormFieldPayload {
    pub field_name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddSignatureFieldPayload {
    pub page_index: usize,
    pub rect: [f32; 4],
    pub field_name: String,
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EditOperationType {
    InsertText,
    DeleteText,
    ReplaceText,
    Redact,
    DeletePage,
    InsertBlankPage,
    MovePage,
    RotatePage,
    FillFormField,
    AddSignatureField,
    MoveObject,
    ResizeObject,
    DeleteObject,
    SetProperty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOperation {
    pub id: String,
    pub op_type: EditOperationType,
    pub session_id: String,
    pub page_index: usize,
    pub object_ref: Option<String>,
    /// Serialised payload specific to the operation type
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTransaction {
    pub transaction_id: String,
    pub session_id: String,
    pub operations: Vec<EditOperation>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTransactionResult {
    pub transaction_id: String,
    pub success: bool,
    pub applied_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStorageTier {
    Memory,
    Disk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoMemoryBudget {
    pub max_in_memory_bytes: usize,
    pub max_disk_backed_bytes: usize,
    pub max_undo_entries: usize,
    pub spill_to_disk_threshold: usize,
    pub compression_enabled: bool,
    pub current_in_memory_usage: usize,
    pub current_disk_usage: usize,
}

impl Default for UndoMemoryBudget {
    fn default() -> Self {
        Self {
            max_in_memory_bytes: 512 * 1024 * 1024,
            max_disk_backed_bytes: 2 * 1024 * 1024 * 1024,
            max_undo_entries: 100,
            spill_to_disk_threshold: 16 * 1024 * 1024,
            compression_enabled: false,
            current_in_memory_usage: 0,
            current_disk_usage: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoSnapshotDescriptor {
    pub transaction_id: String,
    pub document_id: String,
    pub revision_before: u64,
    pub revision_after: u64,
    pub snapshot_size: usize,
    pub storage_tier: SnapshotStorageTier,
    pub disk_path: Option<String>,
    pub created_at_epoch_ms: u128,
    pub operation_label: String,
    pub restorable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRedoState {
    pub session_id: String,
    pub undo_depth: usize,
    pub redo_depth: usize,
    pub last_description: Option<String>,
    pub undo_entry_count: usize,
    pub redo_entry_count: usize,
    pub in_memory_undo_bytes: usize,
    pub disk_backed_undo_bytes: usize,
    pub pruned_entry_count: usize,
    pub latest_snapshot_tier: Option<SnapshotStorageTier>,
    pub memory_budget: usize,
    pub disk_budget: usize,
    pub undo_degraded: bool,
}
