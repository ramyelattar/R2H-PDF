//! Versioned application-data persistence for production shell records.
//!
//! The renderer may keep a projection of these records for display, but it
//! does not own their identity or write them directly.  This registry stores
//! only metadata and workflow records; it never stores PDF bytes, filesystem
//! snapshots, OCR text, or canvas/editor state.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

pub const SHELL_LIBRARY_SCHEMA_VERSION: u32 = 1;
const MAX_LIBRARY_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORKSPACES: usize = 100;
const MAX_RECENT_FILES: usize = 50;
const MAX_REVIEW_SESSIONS: usize = 100;
const MAX_EXPORT_BUNDLES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at_epoch_ms: u64,
    pub modified_at_epoch_ms: u64,
    pub document_count: usize,
    pub last_opened_at_epoch_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFileRecord {
    pub id: String,
    pub title: String,
    pub path: String,
    pub workspace_id: String,
    pub modified_at_epoch_ms: u64,
    pub last_opened_at_epoch_ms: u64,
    pub pages: usize,
    #[serde(default)]
    pub pinned: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearchRecord {
    pub id: String,
    pub name: String,
    pub query: String,
    pub scope: String,
    pub filters: serde_json::Value,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSessionRecord {
    pub id: String,
    pub project_id: String,
    pub document_id: String,
    pub status: String,
    pub created_at_epoch_ms: u64,
    pub updated_at_epoch_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffPackageRecord {
    pub id: String,
    pub project_id: String,
    pub destination_path: String,
    pub created_at_epoch_ms: u64,
    pub included_documents: Vec<String>,
    pub output_sha256: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundleRecord {
    pub id: String,
    pub project_id: String,
    pub document_id: String,
    pub destination: String,
    pub export_type: String,
    pub timestamp_epoch_ms: u64,
    pub size_bytes: u64,
    pub sha256: String,
    pub page_count: usize,
    pub included_overlays: usize,
    pub warnings: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub schema_version: u32,
    pub workspaces: Vec<WorkspaceRecord>,
    pub recent_files: Vec<RecentFileRecord>,
    pub saved_searches: Vec<SavedSearchRecord>,
    pub review_sessions: Vec<ReviewSessionRecord>,
    pub handoff_packages: Vec<HandoffPackageRecord>,
    pub export_bundles: Vec<ExportBundleRecord>,
    /// SHA-256 over this same structure with `integritySha256` empty.
    pub integrity_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecordOpenRequest {
    pub source_path: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub page_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecordProjectRequest {
    pub source_path: String,
    pub project_name: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub page_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecordExportRequest {
    pub source_path: String,
    pub destination: String,
    pub export_type: String,
    pub timestamp_epoch_ms: u64,
    pub size_bytes: u64,
    pub sha256: String,
    pub page_count: usize,
    pub included_overlays: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecordReviewRequest {
    pub source_path: String,
    pub review_id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
struct FileIdentity {
    canonical_path: String,
    workspace_path: String,
    title: String,
    file_id: String,
    modified_at_epoch_ms: u64,
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_default()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn stable_id(prefix: &str, value: &str) -> String {
    let normalized = if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    hasher.update(normalized.as_bytes());
    format!("{prefix}-{:x}", hasher.finalize())
}

fn library_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("projects"))
        .map_err(|err| format!("LIBRARY_STORAGE_UNAVAILABLE: {err}"))
}

fn library_path(root: &Path) -> PathBuf {
    root.join("shell-library.json")
}

fn empty_snapshot() -> LibrarySnapshot {
    LibrarySnapshot {
        schema_version: SHELL_LIBRARY_SCHEMA_VERSION,
        workspaces: Vec::new(),
        recent_files: Vec::new(),
        saved_searches: Vec::new(),
        review_sessions: Vec::new(),
        handoff_packages: Vec::new(),
        export_bundles: Vec::new(),
        integrity_sha256: String::new(),
    }
}

fn serialize_with_integrity(snapshot: &mut LibrarySnapshot) -> Result<Vec<u8>, String> {
    snapshot.integrity_sha256.clear();
    let unsigned =
        serde_json::to_vec(snapshot).map_err(|err| format!("LIBRARY_SERIALIZE_FAILED: {err}"))?;
    snapshot.integrity_sha256 = sha256_hex(&unsigned);
    serde_json::to_vec_pretty(snapshot).map_err(|err| format!("LIBRARY_SERIALIZE_FAILED: {err}"))
}

fn verify_integrity(snapshot: &LibrarySnapshot) -> Result<(), String> {
    if snapshot.integrity_sha256.is_empty() {
        return Err("LIBRARY_CORRUPT: integrity hash is missing".to_string());
    }
    let expected = snapshot.integrity_sha256.clone();
    let mut unsigned = snapshot.clone();
    unsigned.integrity_sha256.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|err| format!("LIBRARY_INTEGRITY_CHECK_FAILED: {err}"))?;
    if sha256_hex(&bytes) != expected {
        return Err("LIBRARY_INTEGRITY_MISMATCH: registry content hash is invalid".to_string());
    }
    Ok(())
}

fn validate_snapshot(snapshot: &LibrarySnapshot) -> Result<(), String> {
    if snapshot.schema_version != SHELL_LIBRARY_SCHEMA_VERSION {
        return Err(format!(
            "LIBRARY_SCHEMA_UNSUPPORTED: expected {}, got {}",
            SHELL_LIBRARY_SCHEMA_VERSION, snapshot.schema_version
        ));
    }
    if snapshot.workspaces.len() > MAX_WORKSPACES {
        return Err("LIBRARY_INVALID: too many workspaces".to_string());
    }
    if snapshot.recent_files.len() > MAX_RECENT_FILES {
        return Err("LIBRARY_INVALID: too many recent files".to_string());
    }
    if snapshot.review_sessions.len() > MAX_REVIEW_SESSIONS {
        return Err("LIBRARY_INVALID: too many review sessions".to_string());
    }
    if snapshot.export_bundles.len() > MAX_EXPORT_BUNDLES {
        return Err("LIBRARY_INVALID: too many export bundles".to_string());
    }
    let workspace_ids = snapshot
        .workspaces
        .iter()
        .map(|workspace| workspace.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    for file in &snapshot.recent_files {
        if !file.workspace_id.is_empty() && !workspace_ids.contains(file.workspace_id.as_str()) {
            return Err("LIBRARY_INVALID: recent file references an unknown workspace".to_string());
        }
        if file.path.trim().is_empty() || file.id.trim().is_empty() {
            return Err("LIBRARY_INVALID: recent file identity is missing".to_string());
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<LibrarySnapshot, String> {
    let bytes = fs::read(path).map_err(|err| format!("LIBRARY_READ_FAILED: {err}"))?;
    if bytes.len() > MAX_LIBRARY_BYTES {
        return Err("LIBRARY_TOO_LARGE: registry exceeds the size limit".to_string());
    }
    let snapshot: LibrarySnapshot = serde_json::from_slice(&bytes)
        .map_err(|err| format!("LIBRARY_CORRUPT: invalid JSON ({err})"))?;
    verify_integrity(&snapshot)?;
    validate_snapshot(&snapshot)?;
    Ok(refresh_availability(snapshot))
}

fn refresh_availability(mut snapshot: LibrarySnapshot) -> LibrarySnapshot {
    for file in &mut snapshot.recent_files {
        file.available = Path::new(&file.path).is_file();
    }
    snapshot
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "LIBRARY_WRITE_FAILED: registry has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("LIBRARY_WRITE_FAILED: create directory: {err}"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("shell-library.json");
    let temp_path = parent.join(format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        epoch_ms()
    ));
    let backup_path = parent.join(format!(".{file_name}.bak"));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|err| format!("LIBRARY_WRITE_FAILED: create temp: {err}"))?;
        file.write_all(bytes)
            .map_err(|err| format!("LIBRARY_WRITE_FAILED: write temp: {err}"))?;
        file.flush()
            .map_err(|err| format!("LIBRARY_WRITE_FAILED: flush temp: {err}"))?;
        file.sync_all()
            .map_err(|err| format!("LIBRARY_WRITE_FAILED: sync temp: {err}"))?;

        let target_existed = path.exists();
        if target_existed {
            if backup_path.exists() {
                fs::remove_file(&backup_path)
                    .map_err(|err| format!("LIBRARY_WRITE_FAILED: remove old backup: {err}"))?;
            }
            fs::rename(path, &backup_path)
                .map_err(|err| format!("LIBRARY_WRITE_FAILED: stage old registry: {err}"))?;
        }
        if let Err(err) = fs::rename(&temp_path, path) {
            if target_existed && backup_path.exists() {
                let _ = fs::rename(&backup_path, path);
            }
            return Err(format!("LIBRARY_WRITE_FAILED: commit registry: {err}"));
        }
        if target_existed && backup_path.exists() {
            let _ = fs::remove_file(&backup_path);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn canonical_file_identity(source_path: &str) -> Result<FileIdentity, String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() || trimmed.starts_with("session://") {
        return Err("INVALID_LIBRARY_PATH: a real PDF path is required".to_string());
    }
    let path = Path::new(trimmed);
    let canonical =
        fs::canonicalize(path).map_err(|err| format!("LIBRARY_SOURCE_UNAVAILABLE: {err}"))?;
    if !canonical.is_file() {
        return Err("LIBRARY_SOURCE_UNAVAILABLE: source is not a file".to_string());
    }
    if canonical
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("pdf"))
        != Some(true)
    {
        return Err("INVALID_LIBRARY_PATH: source must have a PDF extension".to_string());
    }
    let canonical_path = canonical.to_string_lossy().to_string();
    let workspace_path = canonical
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_string_lossy()
        .to_string();
    let title = canonical
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("Untitled PDF")
        .to_string();
    let modified_at_epoch_ms = fs::metadata(&canonical)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_else(epoch_ms);
    Ok(FileIdentity {
        file_id: stable_id("file", &canonical_path),
        canonical_path,
        workspace_path,
        title,
        modified_at_epoch_ms,
    })
}

fn load_snapshot(path: &Path) -> Result<LibrarySnapshot, String> {
    if !path.exists() {
        return Ok(empty_snapshot());
    }
    read_snapshot(path)
}

fn upsert_open_file(
    mut snapshot: LibrarySnapshot,
    identity: FileIdentity,
    workspace_id: Option<String>,
    page_count: usize,
) -> Result<LibrarySnapshot, String> {
    let now = epoch_ms();
    let selected_workspace_id = workspace_id
        .filter(|id| {
            snapshot
                .workspaces
                .iter()
                .any(|workspace| workspace.id == *id)
        })
        .unwrap_or_default();
    if let Some(workspace) = snapshot.workspaces.iter_mut().find(|workspace| {
        !selected_workspace_id.is_empty() && workspace.id == selected_workspace_id
    }) {
        workspace.modified_at_epoch_ms = now;
        workspace.last_opened_at_epoch_ms = Some(now);
    }

    let pinned = snapshot
        .recent_files
        .iter()
        .find(|file| file.id == identity.file_id || file.path == identity.canonical_path)
        .map(|file| file.pinned)
        .unwrap_or(false);
    snapshot
        .recent_files
        .retain(|file| file.id != identity.file_id && file.path != identity.canonical_path);
    snapshot.recent_files.insert(
        0,
        RecentFileRecord {
            id: identity.file_id,
            title: identity.title,
            path: identity.canonical_path,
            workspace_id: selected_workspace_id.clone(),
            modified_at_epoch_ms: identity.modified_at_epoch_ms,
            last_opened_at_epoch_ms: now,
            pages: page_count,
            pinned,
            available: true,
        },
    );
    snapshot.recent_files.truncate(MAX_RECENT_FILES);
    for workspace in &mut snapshot.workspaces {
        workspace.document_count = snapshot
            .recent_files
            .iter()
            .filter(|file| file.workspace_id == workspace.id)
            .count();
    }
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn project_ids(identity: &FileIdentity) -> (String, String) {
    (
        stable_id("project", &identity.canonical_path),
        stable_id("document", &identity.canonical_path),
    )
}

fn upsert_project_record(
    mut snapshot: LibrarySnapshot,
    identity: FileIdentity,
    request: LibraryRecordProjectRequest,
) -> Result<LibrarySnapshot, String> {
    if request.page_count == 0 {
        return Err("LIBRARY_INVALID: page_count must be greater than zero".to_string());
    }
    let (project_id, _) = project_ids(&identity);
    let workspace_id = request
        .workspace_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| stable_id("workspace-project", &project_id));
    let now = epoch_ms();
    let workspace_name = if request.project_name.trim().is_empty() {
        identity.title.clone()
    } else {
        request.project_name.trim().to_string()
    };
    if let Some(workspace) = snapshot
        .workspaces
        .iter_mut()
        .find(|workspace| workspace.id == workspace_id)
    {
        workspace.modified_at_epoch_ms = now;
        workspace.last_opened_at_epoch_ms = Some(now);
        if workspace.name.trim().is_empty() {
            workspace.name = workspace_name;
        }
        if workspace.path.trim().is_empty() {
            workspace.path = identity.workspace_path.clone();
        }
    } else {
        if snapshot.workspaces.len() >= MAX_WORKSPACES {
            return Err("LIBRARY_LIMIT_REACHED: workspace limit reached".to_string());
        }
        snapshot.workspaces.push(WorkspaceRecord {
            id: workspace_id.clone(),
            name: workspace_name,
            path: identity.workspace_path.clone(),
            created_at_epoch_ms: now,
            modified_at_epoch_ms: now,
            document_count: 0,
            last_opened_at_epoch_ms: Some(now),
        });
    }
    upsert_open_file(snapshot, identity, Some(workspace_id), request.page_count)
}

fn record_export(
    mut snapshot: LibrarySnapshot,
    identity: &FileIdentity,
    request: LibraryRecordExportRequest,
) -> Result<LibrarySnapshot, String> {
    if request.export_type != "pdf" {
        return Err("LIBRARY_INVALID: only PDF export history is supported".to_string());
    }
    if request.destination.trim().is_empty()
        || request.sha256.trim().is_empty()
        || request.page_count == 0
    {
        return Err("LIBRARY_INVALID: completed export metadata is incomplete".to_string());
    }
    let (project_id, document_id) = project_ids(identity);
    let timestamp = if request.timestamp_epoch_ms == 0 {
        epoch_ms()
    } else {
        request.timestamp_epoch_ms
    };
    let id = stable_id(
        "export",
        &format!("{project_id}:{}:{timestamp}", request.destination),
    );
    snapshot.export_bundles.retain(|bundle| bundle.id != id);
    snapshot.export_bundles.insert(
        0,
        ExportBundleRecord {
            id,
            project_id,
            document_id,
            destination: request.destination,
            export_type: request.export_type,
            timestamp_epoch_ms: timestamp,
            size_bytes: request.size_bytes,
            sha256: request.sha256,
            page_count: request.page_count,
            included_overlays: request.included_overlays,
            warnings: request.warnings,
            status: "completed".to_string(),
        },
    );
    snapshot.export_bundles.truncate(MAX_EXPORT_BUNDLES);
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn record_review(
    mut snapshot: LibrarySnapshot,
    identity: &FileIdentity,
    request: LibraryRecordReviewRequest,
) -> Result<LibrarySnapshot, String> {
    if request.review_id.trim().is_empty() || request.status != "in-review" {
        return Err("LIBRARY_INVALID: completed review metadata is incomplete".to_string());
    }
    let status = request.status.clone();
    let (project_id, document_id) = project_ids(identity);
    let now = epoch_ms();
    if let Some(existing) = snapshot
        .review_sessions
        .iter_mut()
        .find(|session| session.id == request.review_id)
    {
        existing.updated_at_epoch_ms = now;
        existing.status = status;
    } else {
        snapshot.review_sessions.insert(
            0,
            ReviewSessionRecord {
                id: request.review_id,
                project_id,
                document_id,
                status,
                created_at_epoch_ms: now,
                updated_at_epoch_ms: now,
            },
        );
    }
    snapshot.review_sessions.truncate(MAX_REVIEW_SESSIONS);
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn persist_snapshot(path: &Path, mut snapshot: LibrarySnapshot) -> Result<LibrarySnapshot, String> {
    let bytes = serialize_with_integrity(&mut snapshot)?;
    if bytes.len() > MAX_LIBRARY_BYTES {
        return Err("LIBRARY_TOO_LARGE: serialized registry exceeds the size limit".to_string());
    }
    atomic_write(path, &bytes)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn library_load(app: AppHandle) -> Result<LibrarySnapshot, String> {
    let root = library_root(&app)?;
    load_snapshot(&library_path(&root))
}

#[tauri::command]
pub fn library_record_open(
    app: AppHandle,
    request: LibraryRecordOpenRequest,
) -> Result<LibrarySnapshot, String> {
    if request.page_count == 0 {
        return Err("LIBRARY_INVALID: page_count must be greater than zero".to_string());
    }
    let identity = canonical_file_identity(&request.source_path)?;
    let root = library_root(&app)?;
    let path = library_path(&root);
    let snapshot = load_snapshot(&path)?;
    let updated = upsert_open_file(snapshot, identity, request.workspace_id, request.page_count)?;
    persist_snapshot(&path, updated)
}

#[tauri::command]
pub fn library_record_project(
    app: AppHandle,
    request: LibraryRecordProjectRequest,
) -> Result<LibrarySnapshot, String> {
    let identity = canonical_file_identity(&request.source_path)?;
    let root = library_root(&app)?;
    let path = library_path(&root);
    let snapshot = load_snapshot(&path)?;
    let updated = upsert_project_record(snapshot, identity, request)?;
    persist_snapshot(&path, updated)
}

#[tauri::command]
pub fn library_record_export(
    app: AppHandle,
    request: LibraryRecordExportRequest,
) -> Result<LibrarySnapshot, String> {
    let identity = canonical_file_identity(&request.source_path)?;
    let root = library_root(&app)?;
    let path = library_path(&root);
    let snapshot = load_snapshot(&path)?;
    let updated = record_export(snapshot, &identity, request)?;
    persist_snapshot(&path, updated)
}

#[tauri::command]
pub fn library_record_review(
    app: AppHandle,
    request: LibraryRecordReviewRequest,
) -> Result<LibrarySnapshot, String> {
    let identity = canonical_file_identity(&request.source_path)?;
    let root = library_root(&app)?;
    let path = library_path(&root);
    let snapshot = load_snapshot(&path)?;
    let updated = record_review(snapshot, &identity, request)?;
    persist_snapshot(&path, updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("r2h-shell-library-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn identity(root: &Path) -> FileIdentity {
        let pdf = root.join("source.pdf");
        fs::write(&pdf, b"%PDF-1.7\n%%EOF\n").unwrap();
        canonical_file_identity(&pdf.to_string_lossy()).unwrap()
    }

    #[test]
    fn missing_registry_is_empty_and_open_is_path_stable() {
        let root = temp_root("identity");
        let first = identity(&root);
        let second = identity(&root);
        assert_eq!(first.file_id, second.file_id);
        assert!(load_snapshot(&root.join("missing.json"))
            .unwrap()
            .recent_files
            .is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn open_upserts_one_recent_record_and_preserves_real_metadata() {
        let root = temp_root("upsert");
        let file = identity(&root);
        let snapshot = upsert_open_file(empty_snapshot(), file.clone(), None, 7).unwrap();
        assert_eq!(snapshot.recent_files.len(), 1);
        assert_eq!(snapshot.recent_files[0].pages, 7);
        assert_eq!(snapshot.recent_files[0].path, file.canonical_path);
        assert!(snapshot.workspaces.is_empty());
        assert!(snapshot.recent_files[0].workspace_id.is_empty());
        let updated = upsert_open_file(snapshot, file, None, 8).unwrap();
        assert_eq!(updated.recent_files.len(), 1);
        assert_eq!(updated.recent_files[0].pages, 8);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_record_creates_workspace_only_from_persisted_project_data() {
        let root = temp_root("project");
        let file = identity(&root);
        let snapshot = upsert_project_record(
            empty_snapshot(),
            file.clone(),
            LibraryRecordProjectRequest {
                source_path: file.canonical_path.clone(),
                project_name: "Real Project".to_string(),
                workspace_id: None,
                page_count: 3,
            },
        )
        .unwrap();
        assert_eq!(snapshot.workspaces.len(), 1);
        assert_eq!(snapshot.workspaces[0].name, "Real Project");
        assert_eq!(snapshot.recent_files.len(), 1);
        assert_eq!(
            snapshot.recent_files[0].workspace_id,
            snapshot.workspaces[0].id
        );
        assert_eq!(snapshot.workspaces[0].document_count, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completed_export_and_review_records_are_deduplicated_and_typed() {
        let root = temp_root("operations");
        let file = identity(&root);
        let project = upsert_project_record(
            empty_snapshot(),
            file.clone(),
            LibraryRecordProjectRequest {
                source_path: file.canonical_path.clone(),
                project_name: "Real Project".to_string(),
                workspace_id: None,
                page_count: 2,
            },
        )
        .unwrap();
        let export_request = LibraryRecordExportRequest {
            source_path: file.canonical_path.clone(),
            destination: root.join("export.pdf").to_string_lossy().to_string(),
            export_type: "pdf".to_string(),
            timestamp_epoch_ms: 42,
            size_bytes: 10,
            sha256: "abc".to_string(),
            page_count: 2,
            included_overlays: 1,
            warnings: Vec::new(),
        };
        let export = record_export(project, &file, export_request.clone()).unwrap();
        assert_eq!(export.export_bundles.len(), 1);
        assert_eq!(export.export_bundles[0].status, "completed");
        let export = record_export(export, &file, export_request).unwrap();
        assert_eq!(export.export_bundles.len(), 1);
        let reviewed = record_review(
            export,
            &file,
            LibraryRecordReviewRequest {
                source_path: file.canonical_path.clone(),
                review_id: "review-1".to_string(),
                status: "in-review".to_string(),
            },
        )
        .unwrap();
        assert_eq!(reviewed.review_sessions.len(), 1);
        assert_eq!(reviewed.review_sessions[0].status, "in-review");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn integrity_round_trip_and_corruption_are_detected() {
        let root = temp_root("integrity");
        let file = identity(&root);
        let mut snapshot = upsert_open_file(empty_snapshot(), file, None, 1).unwrap();
        let bytes = serialize_with_integrity(&mut snapshot).unwrap();
        let path = root.join("library.json");
        atomic_write(&path, &bytes).unwrap();
        let loaded = read_snapshot(&path).unwrap();
        assert_eq!(loaded.recent_files.len(), 1);
        let mut corrupted = loaded;
        corrupted.recent_files[0].title = "tampered".to_string();
        fs::write(&path, serde_json::to_vec(&corrupted).unwrap()).unwrap();
        let error = read_snapshot(&path).unwrap_err();
        assert!(error.contains("LIBRARY_INTEGRITY_MISMATCH"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_pdf_and_virtual_paths_are_rejected() {
        assert!(canonical_file_identity("session://demo").is_err());
        let root = temp_root("paths");
        let txt = root.join("source.txt");
        fs::write(&txt, b"not a pdf").unwrap();
        assert!(canonical_file_identity(&txt.to_string_lossy()).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn future_schema_is_rejected_without_fallback_data() {
        let root = temp_root("future-schema");
        let path = root.join("library.json");
        let mut snapshot = empty_snapshot();
        snapshot.schema_version = SHELL_LIBRARY_SCHEMA_VERSION + 1;
        let bytes = serialize_with_integrity(&mut snapshot).unwrap();
        atomic_write(&path, &bytes).unwrap();
        let error = read_snapshot(&path).unwrap_err();
        assert!(error.contains("LIBRARY_SCHEMA_UNSUPPORTED"));
        let _ = fs::remove_dir_all(root);
    }
}
