//! Versioned, application-data project persistence for document overlays.
//!
//! The renderer owns only the editable projection of a document.  This module
//! is the authoritative durable store for that projection and deliberately
//! keeps volatile Tauri session ids out of the persisted identity.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

pub const PROJECT_SCHEMA_VERSION: u32 = 2;
const MAX_PROJECT_BYTES: usize = 10 * 1024 * 1024;
const MAX_DOCUMENTS: usize = 100;
const MAX_OVERLAYS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSaveRequest {
    pub source_path: String,
    pub project_name: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub page_count: usize,
    pub overlay_objects: Vec<Value>,
    #[serde(default)]
    pub review_state: Option<Value>,
    #[serde(default)]
    pub view_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLoadRequest {
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSaveResponse {
    pub status: String,
    pub project_id: String,
    pub document_id: String,
    pub sidecar_path: String,
    pub integrity_sha256: String,
    pub saved_at_epoch_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLoadResponse {
    pub project: Option<ProjectData>,
    pub project_id: String,
    pub document_id: String,
    pub sidecar_path: String,
    pub migrated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub schema_version: u32,
    pub migration_version: u32,
    pub project_id: String,
    pub project_name: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub created_at_epoch_ms: u128,
    pub last_modified_at_epoch_ms: u128,
    pub documents: Vec<ProjectDocumentData>,
    /// SHA-256 over the same JSON structure with this field empty.
    pub integrity_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocumentData {
    pub document_id: String,
    pub original_source_path: String,
    pub current_saved_pdf_path: String,
    pub source_hash_sha256: Option<String>,
    pub saved_hash_sha256: Option<String>,
    pub page_count: usize,
    /// The sole authoritative overlay collection.  OCR/AI provenance is
    /// retained in each object's metadata.source and is not duplicated into
    /// parallel arrays.
    pub overlay_objects: Vec<Value>,
    #[serde(default)]
    pub review_state: Option<Value>,
    #[serde(default)]
    pub view_state: Option<Value>,
    #[serde(default)]
    pub last_export: Option<Value>,
}

#[derive(Debug, Clone)]
struct ProjectIdentity {
    project_id: String,
    document_id: String,
    canonical_source_path: String,
}

impl ProjectIdentity {
    fn for_path(source_path: &str) -> Result<Self, String> {
        let trimmed = source_path.trim();
        if trimmed.is_empty() || trimmed.starts_with("session://") {
            return Err("INVALID_PROJECT_PATH: a real PDF path is required".to_string());
        }

        let path = Path::new(trimmed);
        let canonical =
            fs::canonicalize(path).map_err(|err| format!("PROJECT_SOURCE_UNAVAILABLE: {err}"))?;
        let canonical_source_path = canonical.to_string_lossy().to_string();
        let document_id = stable_id("document", &canonical_source_path);
        let project_id = stable_id("project", &canonical_source_path);
        Ok(Self {
            project_id,
            document_id,
            canonical_source_path,
        })
    }
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
    format!("{}-{:x}", prefix, hasher.finalize())
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(sha256_hex(&bytes))
}

fn project_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("projects"))
        .map_err(|err| format!("PROJECT_STORAGE_UNAVAILABLE: {err}"))
}

fn sidecar_path(root: &Path, project_id: &str) -> PathBuf {
    root.join(format!("{project_id}.json"))
}

fn serialize_with_integrity(project: &mut ProjectData) -> Result<Vec<u8>, String> {
    project.integrity_sha256.clear();
    let unsigned =
        serde_json::to_vec(project).map_err(|err| format!("PROJECT_SERIALIZE_FAILED: {err}"))?;
    project.integrity_sha256 = sha256_hex(&unsigned);
    serde_json::to_vec_pretty(project).map_err(|err| format!("PROJECT_SERIALIZE_FAILED: {err}"))
}

fn verify_integrity(project: &ProjectData) -> Result<(), String> {
    if project.integrity_sha256.is_empty() {
        return Ok(());
    }
    let mut unsigned = project.clone();
    let expected = unsigned.integrity_sha256.clone();
    unsigned.integrity_sha256.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|err| format!("PROJECT_INTEGRITY_CHECK_FAILED: {err}"))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err("PROJECT_INTEGRITY_MISMATCH: sidecar content hash is invalid".to_string());
    }
    Ok(())
}

fn validate_overlay(value: &Value, page_count: usize) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "PROJECT_INVALID_OVERLAY: overlay must be an object".to_string())?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "PROJECT_INVALID_OVERLAY: overlay id is required".to_string())?;
    if id.is_empty() || id.len() > 256 {
        return Err("PROJECT_INVALID_OVERLAY: overlay id length is invalid".to_string());
    }
    let object_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "PROJECT_INVALID_OVERLAY: overlay type is required".to_string())?;
    if !matches!(
        object_type,
        "textBox"
            | "comment"
            | "highlight"
            | "rectangle"
            | "redaction"
            | "stamp"
            | "image"
            | "strikethrough"
            | "underline"
    ) {
        return Err(format!(
            "PROJECT_INVALID_OVERLAY: unsupported type {object_type}"
        ));
    }

    let page_index = object
        .get("pageIndex")
        .or_else(|| object.get("page_index"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "PROJECT_INVALID_OVERLAY: page index is required".to_string())?;
    if page_count > 0 && page_index >= page_count as u64 {
        return Err(format!(
            "PROJECT_INVALID_OVERLAY: page {page_index} is out of range"
        ));
    }

    let rect = object
        .get("rect")
        .and_then(Value::as_object)
        .ok_or_else(|| "PROJECT_INVALID_OVERLAY: rect is required".to_string())?;
    for key in ["x", "y", "width", "height"] {
        let number = rect
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("PROJECT_INVALID_OVERLAY: rect.{key} is invalid"))?;
        if !number.is_finite() || (key == "width" || key == "height") && number < 0.0 {
            return Err(format!(
                "PROJECT_INVALID_OVERLAY: rect.{key} is out of range"
            ));
        }
    }
    Ok(())
}

fn validate_project(project: &ProjectData, identity: &ProjectIdentity) -> Result<(), String> {
    if project.schema_version != PROJECT_SCHEMA_VERSION {
        return Err(format!(
            "PROJECT_SCHEMA_UNSUPPORTED: expected {}, got {}",
            PROJECT_SCHEMA_VERSION, project.schema_version
        ));
    }
    if project.project_id != identity.project_id {
        return Err("PROJECT_ID_MISMATCH: project does not belong to this document".to_string());
    }
    if project.documents.len() > MAX_DOCUMENTS {
        return Err("PROJECT_INVALID: too many documents".to_string());
    }
    for document in &project.documents {
        if document.overlay_objects.len() > MAX_OVERLAYS {
            return Err("PROJECT_INVALID: too many overlay objects".to_string());
        }
        for overlay in &document.overlay_objects {
            validate_overlay(overlay, document.page_count)?;
        }
    }
    if !project
        .documents
        .iter()
        .any(|doc| doc.document_id == identity.document_id)
    {
        return Err("PROJECT_DOCUMENT_NOT_FOUND: document record is missing".to_string());
    }
    Ok(())
}

fn migrate_legacy(value: Value, identity: &ProjectIdentity) -> Result<ProjectData, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "PROJECT_CORRUPT: root must be an object".to_string())?;
    let legacy_document = object
        .get("document")
        .and_then(Value::as_object)
        .ok_or_else(|| "PROJECT_MIGRATION_FAILED: legacy document is missing".to_string())?;
    let legacy_objects = object
        .get("editor")
        .and_then(Value::as_object)
        .and_then(|editor| editor.get("objects"))
        .and_then(Value::as_array)
        .ok_or_else(|| "PROJECT_MIGRATION_FAILED: legacy editor objects are missing".to_string())?;
    let page_count = legacy_document
        .get("pageCount")
        .or_else(|| legacy_document.get("page_count"))
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let source_path = legacy_document
        .get("path")
        .or_else(|| legacy_document.get("source_path"))
        .and_then(Value::as_str)
        .unwrap_or(&identity.canonical_source_path)
        .to_string();
    let now = epoch_ms();
    let project = ProjectData {
        schema_version: PROJECT_SCHEMA_VERSION,
        migration_version: PROJECT_SCHEMA_VERSION,
        project_id: identity.project_id.clone(),
        project_name: "Migrated project".to_string(),
        workspace_id: None,
        created_at_epoch_ms: now,
        last_modified_at_epoch_ms: now,
        documents: vec![ProjectDocumentData {
            document_id: identity.document_id.clone(),
            original_source_path: source_path.clone(),
            current_saved_pdf_path: source_path,
            source_hash_sha256: None,
            saved_hash_sha256: None,
            page_count,
            overlay_objects: legacy_objects.clone(),
            review_state: None,
            view_state: None,
            last_export: None,
        }],
        integrity_sha256: String::new(),
    };
    validate_project(&project, identity)?;
    Ok(project)
}

fn read_project(path: &Path, identity: &ProjectIdentity) -> Result<(ProjectData, bool), String> {
    let bytes = fs::read(path).map_err(|err| format!("PROJECT_READ_FAILED: {err}"))?;
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err("PROJECT_TOO_LARGE: project sidecar exceeds the size limit".to_string());
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|err| format!("PROJECT_CORRUPT: invalid JSON ({err})"))?;
    let version = value
        .get("schema_version")
        .or_else(|| value.get("schemaVersion"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "PROJECT_CORRUPT: schema version is missing".to_string())?
        as u32;
    if version > PROJECT_SCHEMA_VERSION {
        return Err(format!(
            "PROJECT_SCHEMA_UNSUPPORTED: future version {version} cannot be opened"
        ));
    }
    if version < PROJECT_SCHEMA_VERSION && value.get("documents").is_none() {
        let project = migrate_legacy(value, identity)?;
        return Ok((project, true));
    }
    let project: ProjectData = serde_json::from_value(value)
        .map_err(|err| format!("PROJECT_CORRUPT: incompatible schema ({err})"))?;
    verify_integrity(&project)?;
    validate_project(&project, identity)?;
    Ok((project, false))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "PROJECT_WRITE_FAILED: sidecar has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("PROJECT_WRITE_FAILED: create directory: {err}"))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project.json");
    let temp_path = parent.join(format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        epoch_ms()
    ));
    let backup_path = parent.join(format!(".{file_name}.bak"));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|err| format!("PROJECT_WRITE_FAILED: create temp: {err}"))?;
        file.write_all(bytes)
            .map_err(|err| format!("PROJECT_WRITE_FAILED: write temp: {err}"))?;
        file.flush()
            .map_err(|err| format!("PROJECT_WRITE_FAILED: flush temp: {err}"))?;
        file.sync_all()
            .map_err(|err| format!("PROJECT_WRITE_FAILED: sync temp: {err}"))?;

        let target_existed = path.exists();
        if target_existed {
            if backup_path.exists() {
                fs::remove_file(&backup_path)
                    .map_err(|err| format!("PROJECT_WRITE_FAILED: remove old backup: {err}"))?;
            }
            fs::rename(path, &backup_path)
                .map_err(|err| format!("PROJECT_WRITE_FAILED: stage old sidecar: {err}"))?;
        }

        if let Err(err) = fs::rename(&temp_path, path) {
            if target_existed && backup_path.exists() {
                let _ = fs::rename(&backup_path, path);
            }
            return Err(format!("PROJECT_WRITE_FAILED: commit sidecar: {err}"));
        }

        if target_existed && backup_path.exists() {
            let _ = fs::remove_file(&backup_path);
        }
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn build_project(
    request: ProjectSaveRequest,
    identity: &ProjectIdentity,
    previous: Option<&ProjectData>,
) -> Result<ProjectData, String> {
    if request.page_count == 0 {
        return Err("PROJECT_INVALID: page_count must be greater than zero".to_string());
    }
    if request.overlay_objects.len() > MAX_OVERLAYS {
        return Err("PROJECT_INVALID: too many overlay objects".to_string());
    }
    for overlay in &request.overlay_objects {
        validate_overlay(overlay, request.page_count)?;
    }

    let now = epoch_ms();
    let previous_document = previous.and_then(|project| {
        project
            .documents
            .iter()
            .find(|doc| doc.document_id == identity.document_id)
    });
    let project = ProjectData {
        schema_version: PROJECT_SCHEMA_VERSION,
        migration_version: PROJECT_SCHEMA_VERSION,
        project_id: identity.project_id.clone(),
        project_name: if request.project_name.trim().is_empty() {
            "Untitled project".to_string()
        } else {
            request.project_name.trim().to_string()
        },
        workspace_id: request
            .workspace_id
            .filter(|workspace_id| !workspace_id.trim().is_empty())
            .or_else(|| previous.and_then(|project| project.workspace_id.clone())),
        created_at_epoch_ms: previous
            .map(|project| project.created_at_epoch_ms)
            .unwrap_or(now),
        last_modified_at_epoch_ms: now,
        documents: vec![ProjectDocumentData {
            document_id: identity.document_id.clone(),
            original_source_path: previous_document
                .map(|doc| doc.original_source_path.clone())
                .unwrap_or_else(|| identity.canonical_source_path.clone()),
            current_saved_pdf_path: identity.canonical_source_path.clone(),
            source_hash_sha256: sha256_file(Path::new(&identity.canonical_source_path)),
            saved_hash_sha256: sha256_file(Path::new(&identity.canonical_source_path)),
            page_count: request.page_count,
            overlay_objects: request.overlay_objects,
            review_state: request.review_state,
            view_state: request.view_state,
            last_export: previous_document.and_then(|doc| doc.last_export.clone()),
        }],
        integrity_sha256: String::new(),
    };
    validate_project(&project, identity)?;
    Ok(project)
}

#[tauri::command]
pub fn project_load(
    app: AppHandle,
    request: ProjectLoadRequest,
) -> Result<ProjectLoadResponse, String> {
    let identity = ProjectIdentity::for_path(&request.source_path)?;
    let root = project_root(&app)?;
    let path = sidecar_path(&root, &identity.project_id);
    if !path.exists() {
        return Ok(ProjectLoadResponse {
            project: None,
            project_id: identity.project_id,
            document_id: identity.document_id,
            sidecar_path: path.to_string_lossy().to_string(),
            migrated: false,
        });
    }
    let (project, migrated) = read_project(&path, &identity)?;
    Ok(ProjectLoadResponse {
        project: Some(project),
        project_id: identity.project_id,
        document_id: identity.document_id,
        sidecar_path: path.to_string_lossy().to_string(),
        migrated,
    })
}

#[tauri::command]
pub fn project_save(
    app: AppHandle,
    request: ProjectSaveRequest,
) -> Result<ProjectSaveResponse, String> {
    let identity = ProjectIdentity::for_path(&request.source_path)?;
    let root = project_root(&app)?;
    let path = sidecar_path(&root, &identity.project_id);
    let previous = if path.exists() {
        Some(read_project(&path, &identity)?.0)
    } else {
        None
    };
    let mut project = build_project(request, &identity, previous.as_ref())?;
    let bytes = serialize_with_integrity(&mut project)?;
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err("PROJECT_TOO_LARGE: serialized project exceeds the size limit".to_string());
    }
    let integrity_sha256 = project.integrity_sha256.clone();
    atomic_write(&path, &bytes)?;
    Ok(ProjectSaveResponse {
        status: "project_saved".to_string(),
        project_id: identity.project_id,
        document_id: identity.document_id,
        sidecar_path: path.to_string_lossy().to_string(),
        integrity_sha256,
        saved_at_epoch_ms: project.last_modified_at_epoch_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "r2h-project-persistence-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn identity(root: &Path) -> ProjectIdentity {
        let pdf = root.join("document.pdf");
        fs::write(&pdf, b"%PDF-1.7\n%%EOF\n").unwrap();
        ProjectIdentity::for_path(&pdf.to_string_lossy()).unwrap()
    }

    fn minimal_project(identity: &ProjectIdentity) -> ProjectData {
        ProjectData {
            schema_version: PROJECT_SCHEMA_VERSION,
            migration_version: PROJECT_SCHEMA_VERSION,
            project_id: identity.project_id.clone(),
            project_name: "Test".to_string(),
            workspace_id: None,
            created_at_epoch_ms: 1,
            last_modified_at_epoch_ms: 2,
            documents: vec![ProjectDocumentData {
                document_id: identity.document_id.clone(),
                original_source_path: identity.canonical_source_path.clone(),
                current_saved_pdf_path: identity.canonical_source_path.clone(),
                source_hash_sha256: None,
                saved_hash_sha256: None,
                page_count: 1,
                overlay_objects: vec![],
                review_state: None,
                view_state: None,
                last_export: None,
            }],
            integrity_sha256: String::new(),
        }
    }

    #[test]
    fn stable_ids_are_path_derived_and_not_session_ids() {
        let root = temp_root("ids");
        let first = identity(&root);
        let second = identity(&root);
        assert_eq!(first.project_id, second.project_id);
        assert_eq!(first.document_id, second.document_id);
        assert!(first.document_id.starts_with("document-"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn integrity_round_trip_and_atomic_replace_preserve_latest_valid_state() {
        let root = temp_root("atomic");
        let id = identity(&root);
        let path = sidecar_path(&root, &id.project_id);
        let mut first = minimal_project(&id);
        let first_bytes = serialize_with_integrity(&mut first).unwrap();
        atomic_write(&path, &first_bytes).unwrap();
        let (loaded, migrated) = read_project(&path, &id).unwrap();
        assert!(!migrated);
        assert_eq!(loaded.project_name, "Test");

        let mut second = loaded.clone();
        second.project_name = "Updated".to_string();
        let second_bytes = serialize_with_integrity(&mut second).unwrap();
        atomic_write(&path, &second_bytes).unwrap();
        let (reloaded, _) = read_project(&path, &id).unwrap();
        assert_eq!(reloaded.project_name, "Updated");
        assert!(!path
            .with_file_name(format!(
                ".{}.bak",
                path.file_name().unwrap().to_string_lossy()
            ))
            .exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn future_versions_and_corrupt_integrity_are_rejected() {
        let root = temp_root("reject");
        let id = identity(&root);
        let path = sidecar_path(&root, &id.project_id);
        let mut project = minimal_project(&id);
        project.schema_version = PROJECT_SCHEMA_VERSION + 1;
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        let err = read_project(&path, &id).unwrap_err();
        assert!(err.contains("PROJECT_SCHEMA_UNSUPPORTED"));

        project.schema_version = PROJECT_SCHEMA_VERSION;
        project.integrity_sha256 = "bad".to_string();
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        let err = read_project(&path, &id).unwrap_err();
        assert!(err.contains("PROJECT_INTEGRITY_MISMATCH"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_project_migrates_without_array_index_identity() {
        let root = temp_root("migration");
        let id = identity(&root);
        let value = serde_json::json!({
            "schemaVersion": 1,
            "document": { "path": id.canonical_source_path, "pageCount": 1 },
            "editor": { "objects": [] }
        });
        let migrated = migrate_legacy(value, &id).unwrap();
        assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(migrated.documents[0].document_id, id.document_id);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_overlay_is_rejected() {
        let root = temp_root("overlay");
        let id = identity(&root);
        let mut project = minimal_project(&id);
        project.documents[0].overlay_objects = vec![serde_json::json!({ "id": "bad" })];
        let err = validate_project(&project, &id).unwrap_err();
        assert!(err.contains("PROJECT_INVALID_OVERLAY"));
        let _ = fs::remove_dir_all(root);
    }
}
