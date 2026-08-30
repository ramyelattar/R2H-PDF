use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mupdf::{
    color::AnnotationColor,
    pdf::{PdfAnnotationType, PdfDocument, PdfPage},
    Rect, Size,
};

use crate::document_core::session::DocumentSession;
use crate::document_core::types::PageInfo;
use crate::editing_core::{
    errors::EditingCoreError,
    types::{
        AddSignatureFieldPayload, DeletePagePayload, DeleteTextPayload, EditOperation,
        EditOperationType, EditTransaction, EditTransactionResult, FillFormFieldPayload,
        InsertBlankPagePayload, InsertTextPayload, MovePagePayload, RedactPayload,
        ReplaceTextPayload, RotatePagePayload, SnapshotStorageTier, UndoMemoryBudget,
        UndoRedoState, UndoSnapshotDescriptor,
    },
};

// ---------------------------------------------------------------------------
// History entry types
// ---------------------------------------------------------------------------

/// Stored on the undo stack: the transaction that was applied plus a bounded
/// snapshot descriptor for bytes before the transaction was applied.
#[derive(Clone)]
pub struct HistoryEntry {
    pub tx: EditTransaction,
    pub before_snapshot: StoredSnapshot,
}

/// Stored on the redo stack: the transaction that was undone plus a bounded
/// snapshot descriptor for bytes after the transaction had been applied.
#[derive(Clone)]
pub struct RedoEntry {
    pub tx: EditTransaction,
    pub after_snapshot: StoredSnapshot,
}

#[derive(Clone)]
pub struct StoredSnapshot {
    pub descriptor: UndoSnapshotDescriptor,
    storage: SnapshotStorage,
}

#[derive(Clone)]
enum SnapshotStorage {
    Memory(Vec<u8>),
    Disk(PathBuf),
}

// ---------------------------------------------------------------------------
// Internal per-session history (unused struct kept for future use)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct SessionHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<RedoEntry>,
}

#[allow(dead_code)]
impl SessionHistory {
    fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// EditingEngine
// ---------------------------------------------------------------------------

pub struct EditingEngine {
    /// session_id â†’ undo history
    undo_stacks: HashMap<String, Vec<HistoryEntry>>,
    /// session_id â†’ redo history
    redo_stacks: HashMap<String, Vec<RedoEntry>>,
    budget: UndoMemoryBudget,
    snapshot_root: PathBuf,
    next_snapshot_id: u64,
    pruned_entry_count: usize,
    latest_snapshot_tier: Option<SnapshotStorageTier>,
}

impl EditingEngine {
    pub fn new() -> Self {
        let snapshot_root = default_snapshot_root();
        cleanup_stale_snapshot_root(&snapshot_root);
        Self {
            undo_stacks: HashMap::new(),
            redo_stacks: HashMap::new(),
            budget: UndoMemoryBudget::default(),
            snapshot_root,
            next_snapshot_id: 1,
            pruned_entry_count: 0,
            latest_snapshot_tier: None,
        }
    }

    pub fn with_budget_for_tests(budget: UndoMemoryBudget, snapshot_root: PathBuf) -> Self {
        cleanup_stale_snapshot_root(&snapshot_root);
        Self {
            undo_stacks: HashMap::new(),
            redo_stacks: HashMap::new(),
            budget,
            snapshot_root,
            next_snapshot_id: 1,
            pruned_entry_count: 0,
            latest_snapshot_tier: None,
        }
    }

    fn create_snapshot(
        &mut self,
        tx: &EditTransaction,
        bytes: Vec<u8>,
        revision_before: u64,
        revision_after: u64,
    ) -> Result<StoredSnapshot, EditingCoreError> {
        let snapshot_size = bytes.len();
        let created_at_epoch_ms = now_epoch_ms();
        let snapshot_id = self.next_snapshot_id;
        self.next_snapshot_id = self.next_snapshot_id.saturating_add(1);
        let operation_label = tx.description.clone();

        if snapshot_size >= self.budget.spill_to_disk_threshold {
            let session_dir = self.snapshot_root.join(sanitize_path_component(&tx.session_id));
            std::fs::create_dir_all(&session_dir).map_err(|e| {
                EditingCoreError::SnapshotStorageFailed(format!("create snapshot dir: {e}"))
            })?;
            let file_name = format!(
                "{}_{}_{}.snapshot",
                sanitize_path_component(&tx.transaction_id),
                snapshot_id,
                created_at_epoch_ms
            );
            let path = session_dir.join(file_name);
            std::fs::write(&path, &bytes).map_err(|e| {
                EditingCoreError::SnapshotStorageFailed(format!("write disk snapshot: {e}"))
            })?;
            let descriptor = UndoSnapshotDescriptor {
                transaction_id: tx.transaction_id.clone(),
                document_id: tx.session_id.clone(),
                revision_before,
                revision_after,
                snapshot_size,
                storage_tier: SnapshotStorageTier::Disk,
                disk_path: Some(path.to_string_lossy().to_string()),
                created_at_epoch_ms,
                operation_label,
                restorable: true,
            };
            self.latest_snapshot_tier = Some(SnapshotStorageTier::Disk);
            return Ok(StoredSnapshot {
                descriptor,
                storage: SnapshotStorage::Disk(path),
            });
        }

        let descriptor = UndoSnapshotDescriptor {
            transaction_id: tx.transaction_id.clone(),
            document_id: tx.session_id.clone(),
            revision_before,
            revision_after,
            snapshot_size,
            storage_tier: SnapshotStorageTier::Memory,
            disk_path: None,
            created_at_epoch_ms,
            operation_label,
            restorable: true,
        };
        self.latest_snapshot_tier = Some(SnapshotStorageTier::Memory);
        Ok(StoredSnapshot {
            descriptor,
            storage: SnapshotStorage::Memory(bytes),
        })
    }

    fn restore_snapshot(snapshot: &StoredSnapshot) -> Result<Vec<u8>, EditingCoreError> {
        match &snapshot.storage {
            SnapshotStorage::Memory(bytes) => Ok(bytes.clone()),
            SnapshotStorage::Disk(path) => std::fs::read(path).map_err(|e| {
                EditingCoreError::SnapshotUnavailable(format!(
                    "disk-backed undo snapshot is unavailable at {}: {e}",
                    path.display()
                ))
            }),
        }
    }

    fn delete_snapshot(snapshot: &StoredSnapshot) {
        if let SnapshotStorage::Disk(path) = &snapshot.storage {
            let _ = std::fs::remove_file(path);
        }
    }

    fn clear_redo_stack(&mut self, session_id: &str) {
        if let Some(entries) = self.redo_stacks.get_mut(session_id) {
            for entry in entries.drain(..) {
                Self::delete_snapshot(&entry.after_snapshot);
            }
        }
    }

    pub fn clear_session(&mut self, session_id: &str) {
        if let Some(entries) = self.undo_stacks.remove(session_id) {
            for entry in entries {
                Self::delete_snapshot(&entry.before_snapshot);
            }
        }
        if let Some(entries) = self.redo_stacks.remove(session_id) {
            for entry in entries {
                Self::delete_snapshot(&entry.after_snapshot);
            }
        }
        let _ = std::fs::remove_dir_all(self.snapshot_root.join(sanitize_path_component(session_id)));
    }

    fn usage_for_session(&self, session_id: &str) -> (usize, usize) {
        let mut memory = 0usize;
        let mut disk = 0usize;
        if let Some(entries) = self.undo_stacks.get(session_id) {
            for entry in entries {
                add_snapshot_usage(&entry.before_snapshot, &mut memory, &mut disk);
            }
        }
        if let Some(entries) = self.redo_stacks.get(session_id) {
            for entry in entries {
                add_snapshot_usage(&entry.after_snapshot, &mut memory, &mut disk);
            }
        }
        (memory, disk)
    }

    fn prune_to_budget(&mut self, session_id: &str) {
        loop {
            let undo_len = self.undo_stacks.get(session_id).map(|v| v.len()).unwrap_or(0);
            let (memory, disk) = self.usage_for_session(session_id);
            let over_budget = undo_len > self.budget.max_undo_entries
                || memory > self.budget.max_in_memory_bytes
                || disk > self.budget.max_disk_backed_bytes;
            if !over_budget {
                break;
            }

            let Some(stack) = self.undo_stacks.get_mut(session_id) else {
                break;
            };
            if stack.is_empty() {
                break;
            }
            let entry = stack.remove(0);
            Self::delete_snapshot(&entry.before_snapshot);
            self.pruned_entry_count += 1;
        }
    }

    // -----------------------------------------------------------------------
    // apply_transaction
    // -----------------------------------------------------------------------

    /// Apply `tx` to the document held in `session`, managing undo/redo stacks
    /// and the dirty flag.
    ///
    /// * Snapshots `session.document.bytes` before any mutation.
    /// * Dispatches each operation to `apply_mupdf_op` (stub for now).
    /// * On success: pushes a `HistoryEntry` onto the undo stack, clears the
    ///   redo stack, and sets `session.is_dirty = true`.
    /// * On failure: discards the snapshot and returns an error result without
    ///   touching the stacks.
    pub fn apply_transaction(
        &mut self,
        tx: EditTransaction,
        session: &Arc<Mutex<DocumentSession>>,
    ) -> EditTransactionResult {
        let transaction_id = tx.transaction_id.clone();
        let session_id = tx.session_id.clone();
        let applied_count = tx.operations.len();

        // --- snapshot before bytes ---
        let (before_bytes, revision_before) = {
            match session.lock() {
                Ok(s) => (s.document.bytes.clone(), s.document_revision),
                Err(_) => {
                    return EditTransactionResult {
                        transaction_id,
                        success: false,
                        applied_count: 0,
                        error: Some("session lock poisoned".to_string()),
                    };
                }
            }
        };

        let mut snapshot = match self.create_snapshot(&tx, before_bytes, revision_before, revision_before) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                return EditTransactionResult {
                    transaction_id,
                    success: false,
                    applied_count: 0,
                    error: Some(err.to_string()),
                };
            }
        };

        // --- apply each operation ---
        for op in &tx.operations {
            if let Err(e) = apply_mupdf_op(op, session) {
                Self::delete_snapshot(&snapshot);
                // Discard snapshot; do not push to undo stack.
                return EditTransactionResult {
                    transaction_id,
                    success: false,
                    applied_count: 0,
                    error: Some(e),
                };
            }
        }

        let revision_after = session
            .lock()
            .map(|s| s.document_revision)
            .unwrap_or(revision_before);
        snapshot.descriptor.revision_after = revision_after;

        // --- commit: push undo entry, clear redo, mark dirty ---
        self.undo_stacks
            .entry(session_id.clone())
            .or_default()
            .push(HistoryEntry {
                tx,
                before_snapshot: snapshot,
            });

        self.clear_redo_stack(&session_id);
        self.prune_to_budget(&session_id);

        if let Ok(mut s) = session.lock() {
            s.is_dirty = true;
        }

        EditTransactionResult {
            transaction_id,
            success: true,
            applied_count,
            error: None,
        }
    }

    // -----------------------------------------------------------------------
    // undo
    // -----------------------------------------------------------------------

    pub fn undo(
        &mut self,
        session_id: &str,
        session: &Arc<Mutex<DocumentSession>>,
    ) -> Result<EditTransactionResult, EditingCoreError> {
        let undo_stack = self
            .undo_stacks
            .get_mut(session_id)
            .ok_or_else(|| EditingCoreError::SessionNotFound(session_id.to_string()))?;

        let entry = undo_stack
            .last()
            .cloned()
            .ok_or(EditingCoreError::UndoStackEmpty)?;

        let transaction_id = entry.tx.transaction_id.clone();
        let applied_count = entry.tx.operations.len();

        // Snapshot current bytes as after_bytes for the redo entry.
        let (after_bytes, revision_after) = {
            let s = session
                .lock()
                .map_err(|_| EditingCoreError::TransactionFailed("session lock poisoned".to_string()))?;
            (s.document.bytes.clone(), s.document_revision)
        };

        let before_bytes = Self::restore_snapshot(&entry.before_snapshot)?;
        let _ = self
            .undo_stacks
            .get_mut(session_id)
            .and_then(|stack| stack.pop());

        // Restore before_bytes.
        {
            let mut s = session
                .lock()
                .map_err(|_| EditingCoreError::TransactionFailed("session lock poisoned".to_string()))?;
            s.document.bytes = before_bytes;
            s.is_dirty = true;
            s.invalidate_cached_document();
        }

        let redo_snapshot =
            self.create_snapshot(&entry.tx, after_bytes, entry.before_snapshot.descriptor.revision_before, revision_after)?;

        // Push redo entry.
        self.redo_stacks
            .entry(session_id.to_string())
            .or_default()
            .push(RedoEntry {
                tx: entry.tx,
                after_snapshot: redo_snapshot,
            });

        Ok(EditTransactionResult {
            transaction_id,
            success: true,
            applied_count,
            error: None,
        })
    }

    // -----------------------------------------------------------------------
    // redo
    // -----------------------------------------------------------------------

    pub fn redo(
        &mut self,
        session_id: &str,
        session: &Arc<Mutex<DocumentSession>>,
    ) -> Result<EditTransactionResult, EditingCoreError> {
        let redo_stack = self
            .redo_stacks
            .get_mut(session_id)
            .ok_or_else(|| EditingCoreError::SessionNotFound(session_id.to_string()))?;

        let entry = redo_stack
            .last()
            .cloned()
            .ok_or(EditingCoreError::RedoStackEmpty)?;

        let transaction_id = entry.tx.transaction_id.clone();
        let applied_count = entry.tx.operations.len();

        // Snapshot current bytes as before_bytes for the new undo entry.
        let (before_bytes, revision_before) = {
            let s = session
                .lock()
                .map_err(|_| EditingCoreError::TransactionFailed("session lock poisoned".to_string()))?;
            (s.document.bytes.clone(), s.document_revision)
        };

        let after_bytes = Self::restore_snapshot(&entry.after_snapshot)?;
        let _ = self
            .redo_stacks
            .get_mut(session_id)
            .and_then(|stack| stack.pop());

        // Restore after_bytes (the post-apply state).
        {
            let mut s = session
                .lock()
                .map_err(|_| EditingCoreError::TransactionFailed("session lock poisoned".to_string()))?;
            s.document.bytes = after_bytes;
            s.is_dirty = true;
            s.invalidate_cached_document();
        }

        let undo_snapshot = self.create_snapshot(
            &entry.tx,
            before_bytes,
            revision_before,
            entry.after_snapshot.descriptor.revision_after,
        )?;

        // Push undo entry.
        self.undo_stacks
            .entry(session_id.to_string())
            .or_default()
            .push(HistoryEntry {
                tx: entry.tx,
                before_snapshot: undo_snapshot,
            });
        self.prune_to_budget(session_id);

        Ok(EditTransactionResult {
            transaction_id,
            success: true,
            applied_count,
            error: None,
        })
    }

    // -----------------------------------------------------------------------
    // undo_redo_state (read-only, no session needed)
    // -----------------------------------------------------------------------

    pub fn undo_redo_state(&self, session_id: &str) -> UndoRedoState {
        let undo_depth = self
            .undo_stacks
            .get(session_id)
            .map(|v| v.len())
            .unwrap_or(0);
        let redo_depth = self
            .redo_stacks
            .get(session_id)
            .map(|v| v.len())
            .unwrap_or(0);
        let last_description = self
            .undo_stacks
            .get(session_id)
            .and_then(|v| v.last())
            .map(|e| e.tx.description.clone());

        let (memory, disk) = self.usage_for_session(session_id);
        UndoRedoState {
            session_id: session_id.to_string(),
            undo_depth,
            redo_depth,
            last_description,
            undo_entry_count: undo_depth,
            redo_entry_count: redo_depth,
            in_memory_undo_bytes: memory,
            disk_backed_undo_bytes: disk,
            pruned_entry_count: self.pruned_entry_count,
            latest_snapshot_tier: self.latest_snapshot_tier.clone(),
            memory_budget: self.budget.max_in_memory_bytes,
            disk_budget: self.budget.max_disk_backed_bytes,
            undo_degraded: self.pruned_entry_count > 0,
        }
    }
}

// ---------------------------------------------------------------------------
// apply_mupdf_op â€” text operations (task 25)
// ---------------------------------------------------------------------------

/// Dispatch a single `EditOperation` to the appropriate MuPDF API.
///
/// Text operations (InsertText, DeleteText, ReplaceText, Redact) are
/// implemented here using the MuPDF Rust bindings.  Page and form operations
/// are handled in tasks 26-27.
fn apply_mupdf_op(
    op: &EditOperation,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    match op.op_type {
        EditOperationType::InsertText => {
            let payload: InsertTextPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("InsertText payload parse error: {e}"))?;
            apply_insert_text(payload, session)
        }
        EditOperationType::DeleteText => {
            let payload: DeleteTextPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("DeleteText payload parse error: {e}"))?;
            apply_delete_text(payload, session)
        }
        EditOperationType::ReplaceText => {
            let payload: ReplaceTextPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("ReplaceText payload parse error: {e}"))?;
            apply_replace_text(payload, session)
        }
        EditOperationType::Redact => {
            let payload: RedactPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("Redact payload parse error: {e}"))?;
            apply_redact(payload, session)
        }
        EditOperationType::DeletePage => {
            let payload: DeletePagePayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("DeletePage payload parse error: {e}"))?;
            apply_delete_page(payload, session)
        }
        EditOperationType::InsertBlankPage => {
            let payload: InsertBlankPagePayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("InsertBlankPage payload parse error: {e}"))?;
            apply_insert_blank_page(payload, session)
        }
        EditOperationType::MovePage => {
            let payload: MovePagePayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("MovePage payload parse error: {e}"))?;
            apply_move_page(payload, session)
        }
        EditOperationType::RotatePage => {
            let payload: RotatePagePayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("RotatePage payload parse error: {e}"))?;
            apply_rotate_page(payload, session)
        }
        EditOperationType::FillFormField => {
            let payload: FillFormFieldPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("FillFormField payload parse error: {e}"))?;
            apply_fill_form_field(payload, session)
        }
        EditOperationType::AddSignatureField => {
            let payload: AddSignatureFieldPayload = serde_json::from_str(&op.payload_json)
                .map_err(|e| format!("AddSignatureField payload parse error: {e}"))?;
            apply_add_signature_field(payload, session)
        }
        // Other operation types (MoveObject, ResizeObject, DeleteObject, SetProperty)
        // are not yet implemented; pass through as no-ops.
        _ => Ok(()),
    }
}

fn default_snapshot_root() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("R2H-PDF")
            .join("undo-snapshots");
    }
    std::env::temp_dir().join("R2H-PDF").join("undo-snapshots")
}

fn cleanup_stale_snapshot_root(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::create_dir_all(root);
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn add_snapshot_usage(snapshot: &StoredSnapshot, memory: &mut usize, disk: &mut usize) {
    match snapshot.descriptor.storage_tier {
        SnapshotStorageTier::Memory => {
            *memory = memory.saturating_add(snapshot.descriptor.snapshot_size);
        }
        SnapshotStorageTier::Disk => {
            *disk = disk.saturating_add(snapshot.descriptor.snapshot_size);
        }
    }
}

fn with_pdf_document<F>(session: &Arc<Mutex<DocumentSession>>, f: F) -> Result<(), String>
where
    F: FnOnce(&mut PdfDocument) -> Result<(), String>,
{
    // Read current bytes.
    let bytes = {
        session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?
            .document
            .bytes
            .clone()
    };

    // Open as PdfDocument.
    let mut pdf = PdfDocument::from_bytes(&bytes)
        .map_err(|e| format!("failed to open PDF for editing: {e}"))?;

    // Apply the mutation.
    f(&mut pdf)?;

    // Serialise back to bytes.
    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes)
        .map_err(|e| format!("failed to serialise mutated PDF: {e}"))?;

    // Write back into the session and invalidate the cached parsed document.
    let mut s = session
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;
    s.document.bytes = new_bytes;
    s.invalidate_cached_document();

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: load a PdfPage from a PdfDocument by index.
// ---------------------------------------------------------------------------

fn load_pdf_page(pdf: &PdfDocument, page_index: usize) -> Result<PdfPage, String> {
    let page_count = pdf
        .page_count()
        .map_err(|e| format!("page_count error: {e}"))?;
    let page_count = usize::try_from(page_count).unwrap_or(0);
    if page_index >= page_count {
        return Err(format!(
            "PAGE_OUT_OF_RANGE: requested page {page_index}, document has {page_count} pages"
        ));
    }
    let page_no = i32::try_from(page_index)
        .map_err(|e| format!("page index conversion error: {e}"))?;
    let fz_page = pdf
        .load_page(page_no)
        .map_err(|e| format!("load_page error: {e}"))?;
    PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage conversion error: {e}"))
}

// ---------------------------------------------------------------------------
// InsertText: add a FreeText annotation at (x, y) with the given text.
//
// MuPDF's FreeText annotation type is the standard way to add visible text
// to a PDF page without rewriting the content stream.  The annotation is
// embedded in the PDF when the document is serialised.
// ---------------------------------------------------------------------------

fn apply_insert_text(
    payload: InsertTextPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    with_pdf_document(session, |pdf| {
        let mut page = load_pdf_page(pdf, payload.page_index)?;

        // Build a small rect around the insertion point.
        // Width is estimated from text length; height from font size.
        let char_width_estimate = payload.size * 0.6;
        let text_width = char_width_estimate * payload.text.len() as f32;
        let rect = Rect::new(
            payload.x,
            payload.y,
            payload.x + text_width.max(payload.size),
            payload.y + payload.size * 1.2,
        );

        // Create the FreeText annotation.
        let mut annot = page
            .create_annotation(PdfAnnotationType::FreeText)
            .map_err(|e| format!("create FreeText annotation error: {e}"))?;

        annot
            .set_rect(rect)
            .map_err(|e| format!("set_rect error: {e}"))?;

        drop(annot);

        // Set /Contents and /DA on the annotation object via the page's /Annots array.
        // After create_annotation, the new annotation is the last entry in /Annots.
        let page_obj = page.object();
        if let Some(annots) = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?
        {
            let len = annots
                .len()
                .map_err(|e| format!("Annots len error: {e}"))?;
            if len > 0 {
                if let Some(mut last_annot) = annots
                    .get_array(i32::try_from(len - 1).unwrap_or(0))
                    .map_err(|e| format!("get_array error: {e}"))?
                {
                    // /Contents â€” the visible text.
                    let contents_val = mupdf::pdf::PdfObject::new_string(&payload.text)
                        .map_err(|e| format!("new_string Contents error: {e}"))?;
                    last_annot
                        .dict_put("Contents", contents_val)
                        .map_err(|e| format!("dict_put Contents error: {e}"))?;

                    // /DA â€” default appearance: font name, size, and colour.
                    let da = format!("/{} {} Tf 0 g", payload.font, payload.size);
                    let da_val = mupdf::pdf::PdfObject::new_string(&da)
                        .map_err(|e| format!("new_string DA error: {e}"))?;
                    last_annot
                        .dict_put("DA", da_val)
                        .map_err(|e| format!("dict_put DA error: {e}"))?;
                }
            }
        }

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// DeleteText: place a Redact annotation over the bbox and apply redaction.
// ---------------------------------------------------------------------------

fn apply_delete_text(
    payload: DeleteTextPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    let [x0, y0, x1, y1] = payload.bbox;
    let rect = Rect::new(x0, y0, x1, y1);
    apply_redact_rect(payload.page_index, rect, session)
}

// ---------------------------------------------------------------------------
// ReplaceText: redact the old region, then insert a FreeText annotation.
// ---------------------------------------------------------------------------

fn apply_replace_text(
    payload: ReplaceTextPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    // Step 1: redact the old region.
    let [x0, y0, x1, y1] = payload.bbox;
    let rect = Rect::new(x0, y0, x1, y1);
    apply_redact_rect(payload.page_index, rect, session)?;

    // Step 2: insert the new text at the top-left of the old bbox.
    let insert_payload = InsertTextPayload {
        page_index: payload.page_index,
        x: x0,
        y: y0,
        text: payload.new_text,
        font: "Helvetica".to_string(),
        size: (y1 - y0).max(10.0),
    };
    apply_insert_text(insert_payload, session)
}

// ---------------------------------------------------------------------------
// Redact: place a black-fill Redact annotation and apply it.
// ---------------------------------------------------------------------------

fn apply_redact(
    payload: RedactPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    let [x0, y0, x1, y1] = payload.rect;
    let rect = Rect::new(x0, y0, x1, y1);
    apply_redact_rect(payload.page_index, rect, session)
}

// ---------------------------------------------------------------------------
// Shared helper: create a Redact annotation over `rect` on `page_index`,
// then call page.redact() to burn it in.
// ---------------------------------------------------------------------------

fn apply_redact_rect(
    page_index: usize,
    rect: Rect,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    with_pdf_document(session, |pdf| {
        let mut page = load_pdf_page(pdf, page_index)?;

        let mut annot = page
            .create_annotation(PdfAnnotationType::Redact)
            .map_err(|e| format!("create Redact annotation error: {e}"))?;

        annot
            .set_rect(rect)
            .map_err(|e| format!("set_rect error: {e}"))?;

        // Black fill for the redaction overlay.
        annot
            .set_color(AnnotationColor::Gray(0.0))
            .map_err(|e| format!("set_color error: {e}"))?;

        drop(annot);

        // Apply all redact annotations on this page.
        page.redact()
            .map_err(|e| format!("redact error: {e}"))?;

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Helper: open PdfDocument, apply a mutation, re-serialise bytes, and then
// update session.document.pages and session.page_count from the new document.
//
// Used by page operations that change the page count or page metadata.
// ---------------------------------------------------------------------------

fn with_pdf_document_and_sync_pages<F>(
    session: &Arc<Mutex<DocumentSession>>,
    f: F,
) -> Result<(), String>
where
    F: FnOnce(&mut PdfDocument) -> Result<(), String>,
{
    // Read current bytes.
    let bytes = {
        session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?
            .document
            .bytes
            .clone()
    };

    // Open as PdfDocument.
    let mut pdf = PdfDocument::from_bytes(&bytes)
        .map_err(|e| format!("failed to open PDF for editing: {e}"))?;

    // Apply the mutation.
    f(&mut pdf)?;

    // Serialise back to bytes.
    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes)
        .map_err(|e| format!("failed to serialise mutated PDF: {e}"))?;

    // Re-open the serialised bytes to read the updated page list.
    let updated_pdf = PdfDocument::from_bytes(&new_bytes)
        .map_err(|e| format!("failed to re-open mutated PDF for page sync: {e}"))?;

    let new_page_count = updated_pdf
        .page_count()
        .map_err(|e| format!("page_count error after mutation: {e}"))?;
    let new_page_count = usize::try_from(new_page_count).unwrap_or(0);

    // Build an updated PageInfo list from the new document.
    let mut new_pages: Vec<PageInfo> = Vec::with_capacity(new_page_count);
    for i in 0..new_page_count {
        let page_no = i32::try_from(i).unwrap_or(0);
        let (width, height, rotation) = if let Ok(fz_page) = updated_pdf.load_page(page_no) {
            if let Ok(pdf_page) = PdfPage::try_from(fz_page) {
                let bounds = pdf_page.bounds().unwrap_or(mupdf::Rect::new(0.0, 0.0, 612.0, 792.0));
                let rot = pdf_page.rotation().unwrap_or(0);
                (bounds.width(), bounds.height(), rot)
            } else {
                (612.0, 792.0, 0)
            }
        } else {
            (612.0, 792.0, 0)
        };
        new_pages.push(PageInfo {
            index: i,
            width_points: width,
            height_points: height,
            rotation,
            has_text: false, // conservative; re-extraction happens on demand
        });
    }

    // Write back into the session and invalidate cached document.
    {
        let mut s = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        s.document.bytes = new_bytes;
        s.document.pages = new_pages;
        s.page_count = new_page_count;
        // Clamp current_page to valid range.
        if new_page_count > 0 && s.current_page >= new_page_count {
            s.current_page = new_page_count - 1;
        }
        s.invalidate_cached_document();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// DeletePage: remove a page from the document (Req 5.1, 5.5).
// ---------------------------------------------------------------------------

fn apply_delete_page(
    payload: DeletePagePayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    // Validate index before opening the document.
    {
        let s = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        let page_count = s.document.pages.len();
        if payload.page_index >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: requested page {}, document has {} pages",
                payload.page_index, page_count
            ));
        }
    }

    let page_no = i32::try_from(payload.page_index)
        .map_err(|e| format!("page index conversion error: {e}"))?;

    with_pdf_document_and_sync_pages(session, |pdf| {
        // Re-validate inside the closure (document may differ from session cache).
        let page_count = pdf
            .page_count()
            .map_err(|e| format!("page_count error: {e}"))?;
        if page_no >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: requested page {page_no}, document has {page_count} pages"
            ));
        }
        pdf.delete_page(page_no)
            .map_err(|e| format!("delete_page error: {e}"))
    })
}

// ---------------------------------------------------------------------------
// InsertBlankPage: insert a new blank page at the given index (Req 5.2).
// ---------------------------------------------------------------------------

fn apply_insert_blank_page(
    payload: InsertBlankPagePayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    // `at_index` may equal page_count (append at end); validate accordingly.
    {
        let s = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        let page_count = s.document.pages.len();
        if payload.at_index > page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: insertion index {}, document has {} pages",
                payload.at_index, page_count
            ));
        }
    }

    // MuPDF new_page_at uses -1 to mean "append at end"; map page_count â†’ -1.
    let page_no: i32 = {
        let s = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        let page_count = s.document.pages.len();
        if payload.at_index == page_count {
            -1
        } else {
            i32::try_from(payload.at_index)
                .map_err(|e| format!("at_index conversion error: {e}"))?
        }
    };

    let size = Size::new(payload.width_pts, payload.height_pts);

    with_pdf_document_and_sync_pages(session, |pdf| {
        // new_page_at creates the page and inserts it at the given position.
        let _page = pdf
            .new_page_at(page_no, size)
            .map_err(|e| format!("new_page_at error: {e}"))?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// MovePage: reorder pages by copying the page object then deleting the
// original (Req 5.3).
//
// Strategy:
//   1. Find the page object at `from_index`.
//   2. Insert it at `to_index` (adjusting for the shift caused by the
//      eventual deletion).
//   3. Delete the original page (its index shifts by Â±1 after the insert).
// ---------------------------------------------------------------------------

fn apply_move_page(
    payload: MovePagePayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    // Validate both indices.
    {
        let s = session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        let page_count = s.document.pages.len();
        if payload.from_index >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: from_index {}, document has {} pages",
                payload.from_index, page_count
            ));
        }
        if payload.to_index >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: to_index {}, document has {} pages",
                payload.to_index, page_count
            ));
        }
    }

    if payload.from_index == payload.to_index {
        return Ok(()); // no-op
    }

    let from = i32::try_from(payload.from_index)
        .map_err(|e| format!("from_index conversion error: {e}"))?;
    let to = i32::try_from(payload.to_index)
        .map_err(|e| format!("to_index conversion error: {e}"))?;

    with_pdf_document_and_sync_pages(session, |pdf| {
        // Re-validate inside the closure.
        let page_count = pdf
            .page_count()
            .map_err(|e| format!("page_count error: {e}"))?;
        if from >= page_count || to >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: from={from} to={to}, document has {page_count} pages"
            ));
        }

        // Retrieve the page object (indirect reference) at `from`.
        let page_obj = pdf
            .find_page(from)
            .map_err(|e| format!("find_page({from}) error: {e}"))?;

        // Determine the insertion position.
        // After inserting at `insert_at`, the original page's index shifts:
        //   - if from < to: original is now at `from` (insert was after it)
        //   - if from > to: original is now at `from + 1` (insert was before it)
        let insert_at = if from < to { to + 1 } else { to };
        let delete_at = if from < to { from } else { from + 1 };

        pdf.insert_page(insert_at, &page_obj)
            .map_err(|e| format!("insert_page({insert_at}) error: {e}"))?;

        pdf.delete_page(delete_at)
            .map_err(|e| format!("delete_page({delete_at}) error: {e}"))?;

        Ok(())
    })
}

// ---------------------------------------------------------------------------
// RotatePage: set the /Rotate entry on a page (Req 5.4).
//
// Valid values are 0, 90, 180, 270.  MuPDF normalises the value internally.
// ---------------------------------------------------------------------------

fn apply_rotate_page(
    payload: RotatePagePayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    // Validate rotation value.
    if !matches!(payload.degrees, 0 | 90 | 180 | 270) {
        return Err(format!(
            "invalid rotation {}: must be 0, 90, 180, or 270",
            payload.degrees
        ));
    }

    with_pdf_document_and_sync_pages(session, |pdf| {
        let mut page = load_pdf_page(pdf, payload.page_index)?;
        page.set_rotation(payload.degrees)
            .map_err(|e| format!("set_rotation error: {e}"))
    })
}

// ---------------------------------------------------------------------------
// FillFormField: locate a form field by name and set its value (Req 6.1).
//
// Strategy:
//   1. Open the document as a PdfDocument.
//   2. Walk the AcroForm /Fields array recursively.
//   3. For each field, compare its /T (partial field name) against the
//      requested field_name.
//   4. When found, set /V to the new value string and update /AP (appearance
//      stream) by removing the cached appearance so MuPDF regenerates it on
//      the next render.
//   5. Serialise back to bytes.
//
// Note: The vendored mupdf-0.6.0 Rust crate does not expose `find_widget` or
// `set_text_field_value` as safe Rust methods.  We therefore manipulate the
// PDF object tree directly via the `PdfObject` dict API, which is the same
// approach used by MuPDF's own `pdf_set_field_value` C function internally.
// ---------------------------------------------------------------------------

fn apply_fill_form_field(
    payload: FillFormFieldPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    with_pdf_document(session, |pdf| {
        // Locate the AcroForm /Fields array.
        let trailer = pdf
            .trailer()
            .map_err(|e| format!("trailer error: {e}"))?;

        let root = trailer
            .get_dict("Root")
            .map_err(|e| format!("get Root error: {e}"))?
            .ok_or_else(|| "PDF has no /Root".to_string())?;

        let acro_form = root
            .get_dict("AcroForm")
            .map_err(|e| format!("get AcroForm error: {e}"))?
            .ok_or_else(|| "PDF has no AcroForm".to_string())?;

        let fields = acro_form
            .get_dict("Fields")
            .map_err(|e| format!("get Fields error: {e}"))?
            .ok_or_else(|| "AcroForm has no /Fields".to_string())?;

        // Recursively search for the field and set its value.
        fill_field_recursive(&fields, &payload.field_name, &payload.value)
            .map_err(|e| format!("FillFormField error: {e}"))?
            .ok_or_else(|| {
                format!(
                    "FIELD_NOT_FOUND: form field '{}' not found in document",
                    payload.field_name
                )
            })
    })
}

/// Recursively walk the PDF field tree looking for a field whose /T entry
/// matches `field_name`.  Returns `Ok(Some(()))` when the field is found and
/// updated, `Ok(None)` when not found, and `Err` on any PDF error.
fn fill_field_recursive(
    fields_array: &mupdf::pdf::PdfObject,
    field_name: &str,
    value: &str,
) -> Result<Option<()>, String> {
    let len = fields_array
        .len()
        .map_err(|e| format!("array len error: {e}"))?;

    for i in 0..len {
        let idx = i32::try_from(i).unwrap_or(0);
        let field = match fields_array
            .get_array(idx)
            .map_err(|e| format!("get_array({i}) error: {e}"))?
        {
            Some(f) => f,
            None => continue,
        };

        // Resolve indirect reference so we can read/write the actual dict.
        let mut resolved = match field
            .resolve()
            .map_err(|e| format!("resolve error: {e}"))?
        {
            Some(r) => r,
            None => continue,
        };

        // Check /T (partial field name).
        if let Some(t_obj) = resolved
            .get_dict("T")
            .map_err(|e| format!("get T error: {e}"))?
        {
            if let Ok(name) = t_obj.as_string() {
                if name == field_name {
                    // Found the field â€” set /V to the new value.
                    let v_val = mupdf::pdf::PdfObject::new_string(value)
                        .map_err(|e| format!("new_string error: {e}"))?;
                    resolved
                        .dict_put("V", v_val)
                        .map_err(|e| format!("dict_put V error: {e}"))?;

                    // Remove cached appearance stream so MuPDF regenerates it.
                    let _ = resolved.dict_delete("AP");

                    return Ok(Some(()));
                }
            }
        }

        // If this field has /Kids, recurse into them.
        if let Some(kids) = resolved
            .get_dict("Kids")
            .map_err(|e| format!("get Kids error: {e}"))?
        {
            if let Some(found) = fill_field_recursive(&kids, field_name, value)? {
                return Ok(Some(found));
            }
        }
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// AddSignatureField: create a signature widget annotation on a page and
// register it in the document's AcroForm (Req 6.2).
//
// Strategy:
//   1. Open the document as a PdfDocument.
//   2. Load the target page and create a Widget annotation on it.
//   3. Set the annotation's /Rect.
//   4. On the annotation's underlying PDF object, set:
//        /FT /Sig          â€” field type: signature
//        /T  (field_name)  â€” partial field name
//        /Ff 0             â€” no special flags
//   5. Ensure the document has an AcroForm and append the new field object
//      (as an indirect reference) to /Fields.
//   6. Serialise back to bytes.
// ---------------------------------------------------------------------------

fn apply_add_signature_field(
    payload: AddSignatureFieldPayload,
    session: &Arc<Mutex<DocumentSession>>,
) -> Result<(), String> {
    with_pdf_document(session, |pdf| {
        // --- 1. Load the target page ---
        let mut page = load_pdf_page(pdf, payload.page_index)?;

        // --- 2. Create a Widget annotation ---
        let mut annot = page
            .create_annotation(PdfAnnotationType::Widget)
            .map_err(|e| format!("create Widget annotation error: {e}"))?;

        // --- 3. Set the bounding rect ---
        let [x0, y0, x1, y1] = payload.rect;
        let rect = Rect::new(x0, y0, x1, y1);
        annot
            .set_rect(rect)
            .map_err(|e| format!("set_rect error: {e}"))?;

        // --- 4. Configure the annotation object as a signature field ---
        // We need to access the underlying PDF object of the annotation.
        // The annotation object is the last entry in the page's /Annots array.
        drop(annot);

        let page_obj = page.object();
        let annots = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?
            .ok_or_else(|| "page has no /Annots after widget creation".to_string())?;

        let annots_len = annots
            .len()
            .map_err(|e| format!("Annots len error: {e}"))?;

        if annots_len == 0 {
            return Err("Annots array is empty after widget creation".to_string());
        }

        let last_idx = i32::try_from(annots_len - 1).unwrap_or(0);
        let widget_ref = annots
            .get_array(last_idx)
            .map_err(|e| format!("get_array error: {e}"))?
            .ok_or_else(|| "could not retrieve widget annotation object".to_string())?;

        // Resolve to the actual dict so we can write to it.
        let mut widget_obj = widget_ref
            .resolve()
            .map_err(|e| format!("resolve widget error: {e}"))?
            .ok_or_else(|| "widget annotation resolved to null".to_string())?;

        // /FT /Sig â€” field type
        let ft_val = mupdf::pdf::PdfObject::new_name("Sig")
            .map_err(|e| format!("new_name Sig error: {e}"))?;
        widget_obj
            .dict_put("FT", ft_val)
            .map_err(|e| format!("dict_put FT error: {e}"))?;

        // /T â€” partial field name
        let t_val = mupdf::pdf::PdfObject::new_string(&payload.field_name)
            .map_err(|e| format!("new_string T error: {e}"))?;
        widget_obj
            .dict_put("T", t_val)
            .map_err(|e| format!("dict_put T error: {e}"))?;

        // /Ff 0 â€” no special field flags
        let ff_val = mupdf::pdf::PdfObject::new_int(0)
            .map_err(|e| format!("new_int Ff error: {e}"))?;
        widget_obj
            .dict_put("Ff", ff_val)
            .map_err(|e| format!("dict_put Ff error: {e}"))?;

        // --- 5. Register in AcroForm /Fields ---
        ensure_acroform_field(pdf, widget_ref)?;

        Ok(())
    })
}

/// Ensure the document has an AcroForm and append `field_ref` to its /Fields
/// array.  Creates the AcroForm dict if it does not already exist.
fn ensure_acroform_field(
    pdf: &mut PdfDocument,
    field_ref: mupdf::pdf::PdfObject,
) -> Result<(), String> {
    let trailer = pdf
        .trailer()
        .map_err(|e| format!("trailer error: {e}"))?;

    let mut root = trailer
        .get_dict("Root")
        .map_err(|e| format!("get Root error: {e}"))?
        .ok_or_else(|| "PDF has no /Root".to_string())?;

    // Get or create the AcroForm dict.
    let mut acro_form = match root
        .get_dict("AcroForm")
        .map_err(|e| format!("get AcroForm error: {e}"))?
    {
        Some(af) => af,
        None => {
            // Create a new AcroForm dict and attach it to /Root.
            let new_af = pdf
                .new_dict()
                .map_err(|e| format!("new_dict AcroForm error: {e}"))?;
            let new_af_indirect = pdf
                .add_object(&new_af)
                .map_err(|e| format!("add_object AcroForm error: {e}"))?;
            root.dict_put("AcroForm", new_af_indirect.clone())
                .map_err(|e| format!("dict_put AcroForm error: {e}"))?;
            new_af_indirect
        }
    };

    // Get or create the /Fields array.
    let mut fields = match acro_form
        .get_dict("Fields")
        .map_err(|e| format!("get Fields error: {e}"))?
    {
        Some(f) => f,
        None => {
            let new_fields = pdf
                .new_array()
                .map_err(|e| format!("new_array Fields error: {e}"))?;
            acro_form
                .dict_put("Fields", new_fields.clone())
                .map_err(|e| format!("dict_put Fields error: {e}"))?;
            new_fields
        }
    };

    // Append the new field reference.
    fields
        .array_push(field_ref)
        .map_err(|e| format!("array_push Fields error: {e}"))
}

// ---------------------------------------------------------------------------
// Unit tests (Req 25.2)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::editing_core::types::{SnapshotStorageTier, UndoMemoryBudget};
    use std::sync::{Arc, Mutex};

    use super::EditingEngine;
    use crate::document_core::{
        engine::OpenedDocument,
        render::RenderPipeline,
        session::DocumentSession,
        text::TextExtractionPipeline,
        types::{
            DocumentSummary, PageInfo, SessionPermissions, ViewportState,
        },
    };
    use crate::editing_core::{
        errors::EditingCoreError,
        types::{EditOperation, EditOperationType, EditTransaction},
    };

    // -----------------------------------------------------------------------
    // Minimal valid PDF fixture
    // -----------------------------------------------------------------------

    /// A minimal but structurally valid PDF 1.4 document with one page.
    /// Used as the in-memory bytes for every test session.
    const MINIMAL_PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
xref\n0 4\n\
0000000000 65535 f \n\
0000000009 00000 n \n\
0000000058 00000 n \n\
0000000115 00000 n \n\
trailer\n<< /Size 4 /Root 1 0 R >>\n\
startxref\n190\n%%EOF";

    // -----------------------------------------------------------------------
    // Helper: build a test DocumentSession wrapped in Arc<Mutex<>>
    // -----------------------------------------------------------------------

    fn make_session(session_id: &str) -> Arc<Mutex<DocumentSession>> {
        make_session_with_bytes(session_id, MINIMAL_PDF.to_vec())
    }

    fn make_session_with_bytes(session_id: &str, bytes: Vec<u8>) -> Arc<Mutex<DocumentSession>> {
        let opened = OpenedDocument {
            source_path: "test.pdf".to_string(),
            document_hash: crate::document_core::session::document_hash(&bytes),
            repaired: false,
            bytes,
            is_scanned: false,
            summary: DocumentSummary {
                page_count: 1,
                object_count: 3,
                title: None,
                author: None,
                producer: None,
            },
            pages: vec![PageInfo {
                index: 0,
                width_points: 612.0,
                height_points: 792.0,
                rotation: 0,
                has_text: false,
            }],
            fonts: vec![],
            objects: vec![],
            recovery_report: None,
            parsed_document: None,
        };

        let session = DocumentSession {
            session_id: session_id.to_string(),
            file_path: "test.pdf".to_string(),
            title: "Test".to_string(),
            page_count: 1,
            current_page: 0,
            zoom: 1.0,
            rotation: 0,
            is_dirty: false,
            is_scanned: false,
            permissions: SessionPermissions {
                can_print: true,
                can_copy: true,
                can_edit: true,
                can_annotate: true,
            },
            last_saved_at: None,
            viewport: ViewportState {
                page_index: 0,
                zoom: 1.0,
                scroll_x: 0.0,
                scroll_y: 0.0,
                fit_mode: "page".to_string(),
            },
            document: opened,
            opened_at_epoch_ms: 0,
            document_revision: 0,
            parsed_document_open_count: 0,
            render_pipeline: RenderPipeline::new(),
            text_pipeline: TextExtractionPipeline::new(),
            cached_document: None,
        };

        Arc::new(Mutex::new(session))
    }

    /// Build a no-op transaction (uses `MoveObject` which is a pass-through in
    /// `apply_mupdf_op`).  The payload_json is an empty object because
    /// `MoveObject` is handled by the catch-all `_ => Ok(())` branch.
    fn make_noop_tx(session_id: &str, tx_id: &str) -> EditTransaction {
        EditTransaction {
            transaction_id: tx_id.to_string(),
            session_id: session_id.to_string(),
            operations: vec![EditOperation {
                id: "op-1".to_string(),
                op_type: EditOperationType::MoveObject,
                session_id: session_id.to_string(),
                page_index: 0,
                object_ref: None,
                payload_json: "{}".to_string(),
            }],
            description: "test no-op".to_string(),
        }
    }

    fn test_budget(memory: usize, disk: usize, entries: usize, spill: usize) -> UndoMemoryBudget {
        UndoMemoryBudget {
            max_in_memory_bytes: memory,
            max_disk_backed_bytes: disk,
            max_undo_entries: entries,
            spill_to_disk_threshold: spill,
            compression_enabled: false,
            current_in_memory_usage: 0,
            current_disk_usage: 0,
        }
    }

    fn snapshot_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "r2h_undo_test_{}_{}",
            std::process::id(),
            name
        ))
    }

    // -----------------------------------------------------------------------
    // 1. apply_transaction succeeds and pushes to the undo stack
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_transaction_success() {
        let session_id = "sess-1";
        let session = make_session(session_id);
        let mut engine = EditingEngine::new();

        let tx = make_noop_tx(session_id, "tx-1");
        let result = engine.apply_transaction(tx, &session);

        assert!(result.success, "apply_transaction should succeed");
        assert_eq!(result.transaction_id, "tx-1");
        assert_eq!(result.applied_count, 1);
        assert!(result.error.is_none());

        // Undo stack should have exactly one entry.
        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.undo_depth, 1, "undo stack should have 1 entry after apply");
        assert_eq!(state.redo_depth, 0, "redo stack should be empty after apply");
    }

    #[test]
    fn small_transaction_stores_snapshot_in_memory() {
        let session_id = "small-memory";
        let session = make_session(session_id);
        let root = snapshot_root("small_memory");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(1024 * 1024, 1024 * 1024, 100, 4096), root);

        let result = engine.apply_transaction(make_noop_tx(session_id, "tx-small"), &session);
        assert!(result.success);
        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.latest_snapshot_tier, Some(SnapshotStorageTier::Memory));
        assert!(state.in_memory_undo_bytes > 0);
        assert_eq!(state.disk_backed_undo_bytes, 0);
        engine.clear_session(session_id);
    }

    #[test]
    fn large_transaction_spills_snapshot_to_disk_and_restores() {
        let session_id = "large-disk";
        let large_bytes = vec![b'%'; 8192];
        let session = make_session_with_bytes(session_id, large_bytes.clone());
        let root = snapshot_root("large_disk");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(1024, 1024 * 1024, 100, 1024), root.clone());

        let result = engine.apply_transaction(make_noop_tx(session_id, "tx-large"), &session);
        assert!(result.success);
        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.latest_snapshot_tier, Some(SnapshotStorageTier::Disk));
        assert_eq!(state.in_memory_undo_bytes, 0);
        assert!(state.disk_backed_undo_bytes >= large_bytes.len());

        engine.undo(session_id, &session).unwrap();
        assert_eq!(session.lock().unwrap().document.bytes, large_bytes);
        engine.clear_session(session_id);
        assert!(!root.join(session_id).exists());
    }

    #[test]
    fn budget_overflow_prunes_oldest_and_marks_degraded() {
        let session_id = "prune";
        let session = make_session(session_id);
        let root = snapshot_root("prune");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(1024 * 1024, 1024 * 1024, 2, 4096), root);

        for index in 0..4 {
            let result = engine.apply_transaction(
                make_noop_tx(session_id, &format!("tx-prune-{index}")),
                &session,
            );
            assert!(result.success);
        }

        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.undo_depth, 2);
        assert_eq!(state.pruned_entry_count, 2);
        assert!(state.undo_degraded);
        assert!(!session.lock().unwrap().document.bytes.is_empty());
        engine.clear_session(session_id);
    }

    #[test]
    fn new_transaction_clears_redo_stack_and_disk_snapshots_clean_on_close() {
        let session_id = "redo-clear";
        let session = make_session_with_bytes(session_id, vec![b'a'; 4096]);
        let root = snapshot_root("redo_clear");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(1024, 1024 * 1024, 100, 1024), root.clone());

        assert!(engine.apply_transaction(make_noop_tx(session_id, "tx-a"), &session).success);
        engine.undo(session_id, &session).unwrap();
        assert_eq!(engine.undo_redo_state(session_id).redo_depth, 1);
        assert!(engine.apply_transaction(make_noop_tx(session_id, "tx-b"), &session).success);
        assert_eq!(engine.undo_redo_state(session_id).redo_depth, 0);

        engine.clear_session(session_id);
        assert!(!root.join(session_id).exists());
    }

    #[test]
    fn missing_disk_snapshot_returns_typed_error() {
        let session_id = "missing-disk";
        let session = make_session_with_bytes(session_id, vec![b'a'; 4096]);
        let root = snapshot_root("missing_disk");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(1024, 1024 * 1024, 100, 1024), root);

        assert!(engine.apply_transaction(make_noop_tx(session_id, "tx-missing"), &session).success);
        if let Some(entry) = engine.undo_stacks.get(session_id).and_then(|stack| stack.last()) {
            if let Some(path) = &entry.before_snapshot.descriptor.disk_path {
                std::fs::remove_file(path).unwrap();
            }
        }
        let result = engine.undo(session_id, &session);
        assert!(matches!(result, Err(EditingCoreError::SnapshotUnavailable(_))));
        engine.clear_session(session_id);
    }

    #[test]
    fn stress_many_large_edits_respects_memory_budget() {
        let session_id = "stress";
        let session = make_session_with_bytes(session_id, vec![b'a'; 8192]);
        let root = snapshot_root("stress");
        let mut engine =
            EditingEngine::with_budget_for_tests(test_budget(2048, 128 * 1024, 100, 1024), root);

        for index in 0..30 {
            assert!(engine
                .apply_transaction(make_noop_tx(session_id, &format!("tx-stress-{index}")), &session)
                .success);
        }

        let state = engine.undo_redo_state(session_id);
        assert!(state.in_memory_undo_bytes <= state.memory_budget);
        assert!(state.disk_backed_undo_bytes <= state.disk_budget);
        assert!(state.pruned_entry_count > 0);
        engine.clear_session(session_id);
    }

    // -----------------------------------------------------------------------
    // 2. undo restores the original bytes
    // -----------------------------------------------------------------------

    #[test]
    fn test_undo_restores_bytes() {
        let session_id = "sess-2";
        let session = make_session(session_id);
        let mut engine = EditingEngine::new();

        // Capture original bytes.
        let original_bytes = session.lock().unwrap().document.bytes.clone();

        // Apply a transaction (no-op, so bytes are unchanged, but the snapshot
        // mechanism is still exercised).
        let tx = make_noop_tx(session_id, "tx-2");
        let apply_result = engine.apply_transaction(tx, &session);
        assert!(apply_result.success);

        // Undo.
        let undo_result = engine.undo(session_id, &session);
        assert!(undo_result.is_ok(), "undo should succeed: {:?}", undo_result);

        // Bytes should be restored to the pre-apply snapshot.
        let restored_bytes = session.lock().unwrap().document.bytes.clone();
        assert_eq!(
            restored_bytes, original_bytes,
            "undo should restore bytes to pre-apply state"
        );

        // Undo stack should now be empty; redo stack should have one entry.
        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.undo_depth, 0);
        assert_eq!(state.redo_depth, 1);
    }

    // -----------------------------------------------------------------------
    // 3. redo restores the post-apply bytes
    // -----------------------------------------------------------------------

    #[test]
    fn test_redo_restores_post_apply_bytes() {
        let session_id = "sess-3";
        let session = make_session(session_id);
        let mut engine = EditingEngine::new();

        // Apply.
        let tx = make_noop_tx(session_id, "tx-3");
        let apply_result = engine.apply_transaction(tx, &session);
        assert!(apply_result.success);

        // Capture post-apply bytes.
        let post_apply_bytes = session.lock().unwrap().document.bytes.clone();

        // Undo.
        engine.undo(session_id, &session).expect("undo should succeed");

        // Redo.
        let redo_result = engine.redo(session_id, &session);
        assert!(redo_result.is_ok(), "redo should succeed: {:?}", redo_result);

        // Bytes should match the post-apply snapshot.
        let after_redo_bytes = session.lock().unwrap().document.bytes.clone();
        assert_eq!(
            after_redo_bytes, post_apply_bytes,
            "redo should restore bytes to post-apply state"
        );

        // Undo stack should have one entry again; redo stack should be empty.
        let state = engine.undo_redo_state(session_id);
        assert_eq!(state.undo_depth, 1);
        assert_eq!(state.redo_depth, 0);
    }

    // -----------------------------------------------------------------------
    // 4. undo on an empty stack returns UndoStackEmpty
    // -----------------------------------------------------------------------

    #[test]
    fn test_undo_empty_stack_error() {
        let session_id = "sess-4";
        let session = make_session(session_id);
        let mut engine = EditingEngine::new();

        // Initialise the undo stack entry for this session (required so the
        // engine knows the session exists but the stack is empty).
        // We do this by applying then undoing once, leaving an empty undo stack.
        let tx = make_noop_tx(session_id, "tx-4a");
        engine.apply_transaction(tx, &session);
        engine.undo(session_id, &session).expect("first undo should succeed");

        // Now the undo stack is empty â€” a second undo should fail.
        let result = engine.undo(session_id, &session);
        assert!(
            matches!(result, Err(EditingCoreError::UndoStackEmpty)),
            "expected UndoStackEmpty, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // 5. redo on an empty stack returns RedoStackEmpty
    // -----------------------------------------------------------------------

    #[test]
    fn test_redo_empty_stack_error() {
        let session_id = "sess-5";
        let session = make_session(session_id);
        let mut engine = EditingEngine::new();

        // Initialise the redo stack entry for this session.
        // Apply, undo (fills redo), then redo (empties redo).
        let tx = make_noop_tx(session_id, "tx-5a");
        engine.apply_transaction(tx, &session);
        engine.undo(session_id, &session).expect("undo should succeed");
        engine.redo(session_id, &session).expect("first redo should succeed");

        // Now the redo stack is empty â€” a second redo should fail.
        let result = engine.redo(session_id, &session);
        assert!(
            matches!(result, Err(EditingCoreError::RedoStackEmpty)),
            "expected RedoStackEmpty, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Property 7 (Req 4.5): apply-then-undo restores original bytes
    //
    // **Validates: Requirements 4.5**
    //
    // For any valid transaction (arbitrary description and transaction id),
    // applying the transaction and then undoing it must yield document bytes
    // identical to the bytes before the apply.
    // -----------------------------------------------------------------------

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]
        #[test]
        fn prop_apply_then_undo_restores_bytes(
            description in "[a-zA-Z0-9 ]{1,50}",
            tx_id in "[a-z]{1,10}",
        ) {
            let session_id = "prop-sess";
            let session = make_session(session_id);
            let mut engine = EditingEngine::new();

            // Capture original bytes before any mutation.
            let original_bytes = session.lock().unwrap().document.bytes.clone();

            // Build a no-op transaction using MoveObject (pass-through in
            // apply_mupdf_op) so the byte-snapshot mechanism is exercised
            // without requiring a real MuPDF mutation.
            let tx = EditTransaction {
                transaction_id: tx_id.clone(),
                session_id: session_id.to_string(),
                operations: vec![EditOperation {
                    id: "op-1".to_string(),
                    op_type: EditOperationType::MoveObject,
                    session_id: session_id.to_string(),
                    page_index: 0,
                    object_ref: None,
                    payload_json: "{}".to_string(),
                }],
                description,
            };

            // Apply the transaction â€” must succeed.
            let apply_result = engine.apply_transaction(tx, &session);
            proptest::prop_assert!(apply_result.success, "apply_transaction failed: {:?}", apply_result.error);

            // Undo the transaction â€” must succeed.
            engine.undo(session_id, &session).expect("undo should succeed");

            // Bytes after undo must equal the bytes before apply.
            let restored_bytes = session.lock().unwrap().document.bytes.clone();
            proptest::prop_assert_eq!(
                restored_bytes,
                original_bytes,
                "undo did not restore original bytes"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property 8 (Req 4.6): apply-undo-redo restores post-apply bytes
    //
    // **Validates: Requirements 4.6**
    //
    // For any valid transaction (arbitrary description and transaction id),
    // applying the transaction, undoing it, and then redoing it must yield
    // document bytes identical to the bytes immediately after the apply.
    // -----------------------------------------------------------------------

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]
        #[test]
        fn prop_apply_undo_redo_restores_post_apply_bytes(
            description in "[a-zA-Z0-9 ]{1,50}",
            tx_id in "[a-z]{1,10}",
        ) {
            let session_id = "prop-sess-redo";
            let session = make_session(session_id);
            let mut engine = EditingEngine::new();

            // Build a no-op transaction using MoveObject (pass-through in
            // apply_mupdf_op) so the byte-snapshot mechanism is exercised
            // without requiring a real MuPDF mutation.
            let tx = EditTransaction {
                transaction_id: tx_id.clone(),
                session_id: session_id.to_string(),
                operations: vec![EditOperation {
                    id: "op-1".to_string(),
                    op_type: EditOperationType::MoveObject,
                    session_id: session_id.to_string(),
                    page_index: 0,
                    object_ref: None,
                    payload_json: "{}".to_string(),
                }],
                description,
            };

            // Apply the transaction â€” must succeed.
            let apply_result = engine.apply_transaction(tx, &session);
            proptest::prop_assert!(apply_result.success, "apply_transaction failed: {:?}", apply_result.error);

            // Capture post-apply bytes.
            let post_apply_bytes = session.lock().unwrap().document.bytes.clone();

            // Undo the transaction â€” must succeed.
            engine.undo(session_id, &session).expect("undo should succeed");

            // Redo the transaction â€” must succeed.
            let redo_result = engine.redo(session_id, &session);
            proptest::prop_assert!(redo_result.is_ok(), "redo failed: {:?}", redo_result.err());

            // Bytes after redo must equal the bytes captured after apply.
            let after_redo_bytes = session.lock().unwrap().document.bytes.clone();
            proptest::prop_assert_eq!(
                after_redo_bytes,
                post_apply_bytes,
                "redo did not restore post-apply bytes"
            );
        }
    }
}
