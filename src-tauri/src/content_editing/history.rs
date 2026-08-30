//! Content edit history: tracks all native edits for audit and undo.
//! Stores session byte snapshots for reversible edits.

use std::sync::Mutex;
use super::types::*;
use crate::document_core::DocumentCoreState;

/// Maximum number of reversible snapshots per session (memory limit).
const MAX_SNAPSHOTS: usize = 10;
const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

/// State container for content edit history.
pub struct ContentEditHistoryState {
    pub records: Mutex<Vec<ContentEditRecord>>,
}

impl ContentEditHistoryState {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// Store a snapshot of session bytes before an edit for undo capability.
    pub fn add_record_with_snapshot(&self, mut record: ContentEditRecord, snapshot: Vec<u8>) {
        if let Ok(mut records) = self.records.lock() {
            if snapshot.len() > MAX_SNAPSHOT_BYTES {
                record.before_stream_snapshot = None;
                record.reversible = false;
                record.warnings.push(format!(
                    "Snapshot exceeded reversible history memory limit ({} bytes).",
                    MAX_SNAPSHOT_BYTES
                ));
                records.push(record);
                return;
            }

            // Enforce memory limit: mark oldest snapshots as non-reversible.
            let session_snapshots: usize = records.iter()
                .filter(|r| r.session_id == record.session_id && r.before_stream_snapshot.is_some())
                .count();
            if session_snapshots >= MAX_SNAPSHOTS {
                // Remove oldest snapshot.
                if let Some(oldest) = records.iter_mut()
                    .filter(|r| r.session_id == record.session_id && r.before_stream_snapshot.is_some())
                    .next()
                {
                    oldest.before_stream_snapshot = None;
                    oldest.reversible = false;
                }
            }
            record.before_stream_snapshot = Some(snapshot);
            record.reversible = true;
            records.push(record);
        }
    }

    pub fn add_record(&self, record: ContentEditRecord) {
        if let Ok(mut records) = self.records.lock() {
            records.push(record);
        }
    }

    pub fn list_records(&self, session_id: &str) -> Vec<ContentEditRecord> {
        self.records.lock()
            .map(|r| r.iter()
                .filter(|rec| rec.session_id == session_id)
                .map(|rec| {
                    // Don't send snapshot bytes to frontend — just metadata.
                    let mut clean = rec.clone();
                    clean.before_stream_snapshot = None;
                    clean
                })
                .collect())
            .unwrap_or_default()
    }

    pub fn get_record(&self, edit_id: &str) -> Option<ContentEditRecord> {
        self.records.lock().ok()
            .and_then(|r| r.iter().find(|rec| rec.edit_id == edit_id).cloned())
    }

    /// Revert a content edit by restoring the session bytes from the snapshot.
    pub fn revert_edit(
        &self,
        doc_state: &DocumentCoreState,
        session_id: &str,
        edit_id: &str,
    ) -> Result<(), String> {
        let snapshot = {
            let records = self.records.lock().map_err(|_| "history lock poisoned".to_string())?;
            let record = records.iter().find(|r| r.edit_id == edit_id && r.session_id == session_id)
                .ok_or_else(|| format!("Edit record not found: {}", edit_id))?;
            if !record.reversible {
                return Err("This edit is no longer reversible (snapshot expired).".to_string());
            }
            record.before_stream_snapshot.clone()
                .ok_or_else(|| "No snapshot available for this edit.".to_string())?
        };

        // Restore session bytes.
        let arc = doc_state.store.get_session_arc_pub(session_id)
            .map_err(|e| e.to_string())?;
        let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        session.document.bytes = snapshot;
        session.is_dirty = true;
        session.invalidate_cached_document();

        // Mark the edit as reverted in history.
        if let Ok(mut records) = self.records.lock() {
            if let Some(record) = records.iter_mut().find(|r| r.edit_id == edit_id) {
                record.reversible = false;
                record.warnings.push("REVERTED".to_string());
            }
        }

        Ok(())
    }

    pub fn clear_session(&self, session_id: &str) {
        if let Ok(mut records) = self.records.lock() {
            records.retain(|r| r.session_id != session_id);
        }
    }
}
