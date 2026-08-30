use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mupdf::Document as MuDocument;
use sha2::{Digest, Sha256};

use super::engine::PdfEngine;
use super::engine::{OpenedDocument, SendDocument};
use super::errors::DocumentCoreError;
use super::mupdf_engine::MuPdfEngine;
use super::render::RenderPipeline;
use super::text::TextExtractionPipeline;
use super::types::{
    FormField, NavigateRequest, NavigateResponse, OpenDocumentRequest, OpenDocumentResponse,
    RecoveryReport, RenderRequest, RenderResponse, SessionDiagnostics, SessionPermissions,
    SessionStateResponse, TextExtractionRequest, TextExtractionResponse, ViewportState,
};
use super::vector::{NativeVectorPageResult, NativeVectorRequest};

pub struct DocumentSession {
    pub session_id: String,
    pub file_path: String,
    pub title: String,
    pub page_count: usize,
    pub current_page: usize,
    pub zoom: f32,
    pub rotation: i32,
    pub is_dirty: bool,
    pub is_scanned: bool,
    pub permissions: SessionPermissions,
    pub last_saved_at: Option<u128>,
    pub viewport: ViewportState,
    pub document: OpenedDocument,
    pub opened_at_epoch_ms: u128,
    pub document_revision: u64,
    pub parsed_document_open_count: usize,
    pub render_pipeline: RenderPipeline,
    pub text_pipeline: TextExtractionPipeline,
    /// Cached parsed MuPDF document handle. Avoids re-parsing `document.bytes`
    /// on every render call. Invalidated (set to `None`) after byte-mutating
    /// operations (edit, save, watermark, etc.) and rebuilt lazily on next use.
    pub cached_document: Option<SendDocument>,
}

impl DocumentSession {
    /// Get or rebuild the cached parsed MuDocument handle.
    /// Returns a reference to the inner `MuDocument`.
    pub fn get_or_parse_document(&mut self) -> Result<&MuDocument, DocumentCoreError> {
        if self.cached_document.is_none() {
            let doc = MuDocument::from_bytes(&self.document.bytes, "pdf")
                .map_err(|e| DocumentCoreError::RenderError(e.to_string()))?;
            self.cached_document = Some(SendDocument(doc));
            self.parsed_document_open_count += 1;
        }
        Ok(&self.cached_document.as_ref().unwrap().0)
    }

    /// Invalidate cached parsed/render state after bytes or page-affecting
    /// overlays change.
    pub fn invalidate_cached_document(&mut self) {
        self.increment_document_revision();
    }

    pub fn invalidate_document_cache(&mut self) {
        self.cached_document = None;
        self.render_pipeline.clear_cache();
    }

    pub fn invalidate_page_cache(&mut self, page_index: usize) {
        self.render_pipeline
            .invalidate_page_cache(&self.session_id, page_index);
    }

    pub fn increment_document_revision(&mut self) {
        self.document_revision = self.document_revision.saturating_add(1);
        self.document.document_hash = document_hash(&self.document.bytes);
        self.invalidate_document_cache();
    }

    pub fn mark_document_bytes_changed(&mut self) {
        self.increment_document_revision();
    }
}

pub fn document_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn session_to_state_response(s: &DocumentSession) -> SessionStateResponse {
    SessionStateResponse {
        session_id: s.session_id.clone(),
        file_path: s.file_path.clone(),
        source_path: s.document.source_path.clone(),
        title: s.title.clone(),
        current_page: s.current_page,
        active_page: s.current_page,
        page_count: s.page_count,
        total_pages: s.page_count,
        zoom: s.zoom,
        rotation: s.rotation,
        is_dirty: s.is_dirty,
        dirty: s.is_dirty,
        is_scanned: s.is_scanned,
        permissions: s.permissions.clone(),
        last_saved_at: s.last_saved_at,
        viewport: s.viewport.clone(),
        load_state: "ready".to_string(),
    }
}

/// Sessions are stored behind individual per-session locks. The outer
/// `sessions` map uses a short-lived `Mutex` only for map lookups and
/// insertions/removals.  CPU-intensive operations (rendering, text extraction,
/// save) lock only the target session's mutex, allowing multiple sessions to
/// operate concurrently without blocking each other.
pub struct SessionStore {
    engine: Arc<MuPdfEngine>,
    sessions: Mutex<HashMap<String, Arc<Mutex<DocumentSession>>>>,
    next_id: Mutex<u64>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(MuPdfEngine::new()),
            sessions: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Look up a session Arc from the map, releasing the map lock immediately.
    fn get_session_arc(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<DocumentSession>>, DocumentCoreError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| DocumentCoreError::LockPoisoned)?;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| DocumentCoreError::SessionNotFound(session_id.to_string()))
    } // map lock dropped here

    /// Public accessor for the session `Arc` ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â used by other engine modules
    /// (e.g. `editing_core`) that need to hold a reference to the session
    /// without going through `SessionStore`'s higher-level methods.
    pub fn get_session_arc_pub(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<DocumentSession>>, DocumentCoreError> {
        self.get_session_arc(session_id)
    }

    pub fn open_document(
        &self,
        request: OpenDocumentRequest,
    ) -> Result<OpenDocumentResponse, DocumentCoreError> {
        let path = Path::new(&request.path);
        let mut opened = self.engine.open(path, request.recover_if_damaged)?;

        let session_id = {
            let mut next_id = self
                .next_id
                .lock()
                .map_err(|_| DocumentCoreError::LockPoisoned)?;
            let id = format!("doc-session-{}", *next_id);
            *next_id += 1;
            id
        };

        let opened_at_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();

        let response = OpenDocumentResponse {
            session_id: session_id.clone(),
            source_path: opened.source_path.clone(),
            recovered: opened.repaired,
            summary: opened.summary.clone(),
            pages: opened.pages.clone(),
            fonts: opened.fonts.clone(),
            objects: opened.objects.clone(),
        };

        let title = response
            .summary
            .title
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                Path::new(&opened.source_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });

        let parsed_document = opened.parsed_document.take().ok_or_else(|| {
            DocumentCoreError::InvalidPdf(
                "opened document did not include parsed handle".to_string(),
            )
        })?;

        let session = DocumentSession {
            session_id: session_id.clone(),
            file_path: opened.source_path.clone(),
            title,
            page_count: opened.pages.len(),
            current_page: 0,
            zoom: 1.0,
            rotation: 0,
            is_dirty: false,
            is_scanned: opened.is_scanned,
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
            opened_at_epoch_ms,
            document_revision: 0,
            parsed_document_open_count: 1,
            render_pipeline: RenderPipeline::new(),
            text_pipeline: TextExtractionPipeline::new(),
            cached_document: Some(parsed_document),
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DocumentCoreError::LockPoisoned)?;
        sessions.insert(session_id, Arc::new(Mutex::new(session)));

        Ok(response)
    }

    pub fn close_document(&self, session_id: &str) -> Result<(), DocumentCoreError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| DocumentCoreError::LockPoisoned)?;
        if sessions.remove(session_id).is_some() {
            if cfg!(debug_assertions) {
                eprintln!(
                    "[document_core] close_session success session={}",
                    session_id
                );
            }
            Ok(())
        } else {
            Err(DocumentCoreError::SessionNotFound(session_id.to_string()))
        }
    }

    pub fn invalidate_document_cache(&self, session_id: &str) -> Result<(), DocumentCoreError> {
        let arc = self.get_session_arc(session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.invalidate_document_cache();
        Ok(())
    }

    pub fn invalidate_page_cache(
        &self,
        session_id: &str,
        page_index: usize,
    ) -> Result<(), DocumentCoreError> {
        let arc = self.get_session_arc(session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.invalidate_page_cache(page_index);
        Ok(())
    }

    pub fn increment_document_revision(&self, session_id: &str) -> Result<u64, DocumentCoreError> {
        let arc = self.get_session_arc(session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.increment_document_revision();
        Ok(session.document_revision)
    }

    pub fn render_page(&self, request: RenderRequest) -> Result<RenderResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;

        session.current_page = request.page_index;
        session.zoom = request.zoom;
        session.viewport.page_index = request.page_index;
        session.viewport.zoom = request.zoom;

        // Ensure the cached document is populated (lazy parse on first use).
        session.get_or_parse_document()?;

        // Now borrow the cached doc and document immutably for the render call.
        // This is safe because get_or_parse_document() guarantees cached_document is Some.
        let cached_doc = &session.cached_document.as_ref().unwrap().0;
        session.render_pipeline.render_from_parsed(
            cached_doc,
            &session.document,
            &request,
            session.document_revision,
            session.rotation,
        )
    }

    pub fn extract_text(
        &self,
        request: TextExtractionRequest,
    ) -> Result<TextExtractionResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.get_or_parse_document()?;
        let cached_doc = &session.cached_document.as_ref().unwrap().0;
        session
            .text_pipeline
            .extract_from_parsed(cached_doc, &session.document, &request)
    }

    pub fn extract_native_vectors(
        &self,
        request: NativeVectorRequest,
    ) -> Result<NativeVectorPageResult, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        self.engine
            .extract_native_vectors(&session.document, &request)
    }

    pub fn extract_all_pages_text(
        &self,
        session_id: String,
    ) -> Result<Vec<String>, DocumentCoreError> {
        let arc = self.get_session_arc(&session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.get_or_parse_document()?;
        let cached_doc = &session.cached_document.as_ref().unwrap().0;
        session
            .text_pipeline
            .extract_all_from_parsed(cached_doc, &session.document)
    }

    /// Phase 25A: per-line bboxes for every page in the session.
    pub fn extract_all_pages_with_line_bboxes(
        &self,
        session_id: String,
    ) -> Result<Vec<super::types::PageLines>, DocumentCoreError> {
        let arc = self.get_session_arc(&session_id)?;
        let session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let engine = &*self.engine;
        engine.extract_all_pages_with_line_bboxes(&session.document)
    }

    pub fn incremental_save(
        &self,
        request: super::types::IncrementalSaveRequest,
    ) -> Result<super::types::IncrementalSaveResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let response = self.engine.incremental_save(&session.document, &request)?;
        if request.target_path.is_some() {
            // Save As changes the authoritative current file reference for
            // this live session. The original source remains represented by
            // the project sidecar's original_source_path field.
            session.file_path = response.saved_to.clone();
            session.document.source_path = response.saved_to.clone();
        }
        session.is_dirty = false;
        session.last_saved_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default(),
        );
        // Invalidate cached document since save may have altered bytes.
        session.invalidate_cached_document();
        Ok(response)
    }

    pub fn navigate(
        &self,
        request: NavigateRequest,
    ) -> Result<NavigateResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;

        if request.page_index >= session.document.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: request.page_index,
                total: session.document.pages.len(),
            });
        }

        session.current_page = request.page_index;
        session.zoom = request.zoom;
        session.viewport.page_index = request.page_index;
        session.viewport.zoom = request.zoom;

        let prefetch_pages = session
            .render_pipeline
            .prefetch_hints(request.page_index, session.document.pages.len());

        Ok(NavigateResponse {
            session_id: request.session_id,
            active_page: request.page_index,
            prefetch_pages,
        })
    }

    pub fn recover_file(&self, path: String) -> Result<RecoveryReport, DocumentCoreError> {
        self.engine.recover_bytes(Path::new(&path))
    }

    pub fn diagnostics(&self, session_id: String) -> Result<SessionDiagnostics, DocumentCoreError> {
        let active_sessions = self
            .sessions
            .lock()
            .map_err(|_| DocumentCoreError::LockPoisoned)?
            .len();
        let arc = self.get_session_arc(&session_id)?;
        let session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let (page_cache_entries, page_cache_bytes) = session.render_pipeline.cache_stats();
        Ok(SessionDiagnostics {
            session_id,
            active_sessions,
            parsed_document_available: session.cached_document.is_some(),
            parsed_document_open_count: session.parsed_document_open_count,
            document_hash: session.document.document_hash.clone(),
            document_revision: session.document_revision,
            page_cache_entries,
            page_cache_bytes,
            render_cache_hit_count: session.render_pipeline.cache_hits.get(),
            render_cache_miss_count: session.render_pipeline.cache_misses.get(),
            opened_at_epoch_ms: session.opened_at_epoch_ms,
            total_renders: session.render_pipeline.total_renders.get(),
            total_text_extractions: session.text_pipeline.total_extractions.get(),
        })
    }

    pub fn session_state(
        &self,
        session_id: String,
    ) -> Result<SessionStateResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&session_id)?;
        let session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        Ok(session_to_state_response(&session))
    }

    pub fn set_zoom(
        &self,
        session_id: String,
        zoom: f32,
    ) -> Result<SessionStateResponse, DocumentCoreError> {
        let clamped_zoom = zoom.clamp(0.25, 4.0);
        let arc = self.get_session_arc(&session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        session.zoom = clamped_zoom;
        session.viewport.zoom = clamped_zoom;
        if cfg!(debug_assertions) {
            eprintln!(
                "[document_core] zoom_changed session={} zoom={:.3}",
                session_id, clamped_zoom
            );
        }
        Ok(session_to_state_response(&session))
    }

    pub fn go_to_page(
        &self,
        session_id: String,
        page_index: usize,
    ) -> Result<SessionStateResponse, DocumentCoreError> {
        let arc = self.get_session_arc(&session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;

        if page_index >= session.document.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: page_index,
                total: session.document.pages.len(),
            });
        }

        session.current_page = page_index;
        session.viewport.page_index = page_index;
        if cfg!(debug_assertions) {
            eprintln!(
                "[document_core] page_changed session={} page={}",
                session_id, page_index
            );
        }
        Ok(session_to_state_response(&session))
    }

    pub fn list_form_fields(
        &self,
        session_id: String,
    ) -> Result<Vec<FormField>, DocumentCoreError> {
        let arc = self.get_session_arc(&session_id)?;
        let session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let engine = &*self.engine;
        engine.list_form_fields(&session.document)
    }

    pub fn create_form_field(
        &self,
        request: super::forms::CreateFormFieldRequest,
    ) -> Result<super::forms::FormFieldOperationResult, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let engine = &*self.engine;
        let existing = engine
            .list_form_fields(&session.document)
            .unwrap_or_default();
        let (result, _name) =
            super::forms::create_form_field(&mut session.document, &request, &existing)?;
        // Rebuild page metadata since bytes changed.
        Self::refresh_doc_metadata(&mut session.document)?;
        session.is_dirty = true;
        session.invalidate_cached_document();
        Ok(result)
    }

    pub fn delete_form_field(
        &self,
        request: super::forms::DeleteFormFieldRequest,
    ) -> Result<super::forms::FormFieldOperationResult, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let result = super::forms::delete_form_field(&mut session.document, &request)?;
        Self::refresh_doc_metadata(&mut session.document)?;
        session.is_dirty = true;
        session.invalidate_cached_document();
        Ok(result)
    }

    pub fn update_form_field_properties(
        &self,
        request: super::forms::UpdateFormFieldPropertiesRequest,
    ) -> Result<super::forms::FormFieldOperationResult, DocumentCoreError> {
        let arc = self.get_session_arc(&request.session_id)?;
        let mut session = arc.lock().map_err(|_| DocumentCoreError::LockPoisoned)?;
        let result = super::forms::update_form_field_properties(&mut session.document, &request)?;
        Self::refresh_doc_metadata(&mut session.document)?;
        session.is_dirty = true;
        session.invalidate_cached_document();
        Ok(result)
    }

    /// Reload page/font/object metadata after the document bytes have changed.
    /// Used by form mutations to keep the session's metadata in sync without
    /// triggering a full reopen.
    fn refresh_doc_metadata(
        doc: &mut super::engine::OpenedDocument,
    ) -> Result<(), DocumentCoreError> {
        let mu_doc = mupdf::Document::from_bytes(&doc.bytes, "pdf")
            .map_err(|e| DocumentCoreError::InvalidPdf(e.to_string()))?;
        let page_count = usize::try_from(
            mu_doc
                .page_count()
                .map_err(|e| DocumentCoreError::InvalidPdf(e.to_string()))?,
        )
        .unwrap_or(0);
        let mut pages = Vec::with_capacity(page_count);
        for i in 0..page_count {
            let page = mu_doc
                .load_page(i as i32)
                .map_err(|e| DocumentCoreError::InvalidPdf(e.to_string()))?;
            let bounds = page
                .bounds()
                .map_err(|e| DocumentCoreError::InvalidPdf(e.to_string()))?;
            pages.push(super::types::PageInfo {
                index: i,
                width_points: bounds.width(),
                height_points: bounds.height(),
                rotation: 0,
                has_text: doc.pages.get(i).map(|p| p.has_text).unwrap_or(false),
            });
        }
        doc.pages = pages;
        doc.summary.page_count = page_count;
        Ok(())
    }
}

#[derive(Clone)]
pub struct DocumentCoreState {
    pub store: Arc<SessionStore>,
}

impl Default for DocumentCoreState {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentCoreState {
    pub fn new() -> Self {
        Self {
            store: Arc::new(SessionStore::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_core::types::{BBox, OpenDocumentRequest};

    fn make_valid_pdf(path: &Path, pages: usize) {
        let mut pdf = mupdf::pdf::PdfDocument::new();
        for _ in 0..pages {
            pdf.new_page(mupdf::Size {
                width: 120.0,
                height: 160.0,
            })
            .unwrap();
        }
        let mut bytes = Vec::new();
        pdf.write_to(&mut bytes).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn temp_pdf_path(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "r2h_session_cache_{}_{}.pdf",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn render_request(session_id: &str, page_index: usize, zoom: f32) -> RenderRequest {
        RenderRequest {
            session_id: session_id.to_string(),
            page_index,
            zoom,
            viewport: BBox {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 160.0,
            },
            device_pixel_ratio: 1.0,
        }
    }

    #[test]
    fn page_cache_invalidation_preserves_the_single_cached_document() {
        let path = temp_pdf_path("invalidate_page_cache");
        make_valid_pdf(&path, 2);

        let store = SessionStore::new();
        let opened = store
            .open_document(OpenDocumentRequest {
                path: path.to_string_lossy().to_string(),
                recover_if_damaged: false,
            })
            .expect("valid PDF should open");

        store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .expect("first page should render");

        store
            .invalidate_page_cache(&opened.session_id, 0)
            .expect("page cache invalidation should succeed");

        {
            let arc = store
                .get_session_arc(&opened.session_id)
                .expect("session should remain registered");
            let session = arc.lock().expect("session lock should not be poisoned");

            assert_eq!(session.parsed_document_open_count, 1);
            assert!(session.cached_document.is_some());
        }

        store
            .close_document(&opened.session_id)
            .expect("session should close");

        std::fs::remove_file(path).expect("temporary PDF should be removed");
    }
    #[test]
    fn opening_pdf_creates_one_parsed_session_and_reuses_it_for_renders() {
        let path = temp_pdf_path("reuse");
        make_valid_pdf(&path, 3);
        let store = SessionStore::new();
        let opened = store
            .open_document(OpenDocumentRequest {
                path: path.to_string_lossy().to_string(),
                recover_if_damaged: false,
            })
            .unwrap();

        let initial = store.diagnostics(opened.session_id.clone()).unwrap();
        assert!(initial.parsed_document_available);
        assert_eq!(initial.parsed_document_open_count, 1);

        store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        store
            .render_page(render_request(&opened.session_id, 1, 1.0))
            .unwrap();
        let diagnostics = store.diagnostics(opened.session_id.clone()).unwrap();
        assert_eq!(diagnostics.parsed_document_open_count, 1);
        assert_eq!(diagnostics.render_cache_miss_count, 2);

        store.close_document(&opened.session_id).unwrap();
        assert!(store.diagnostics(opened.session_id).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn render_cache_hits_same_page_and_splits_by_zoom() {
        let path = temp_pdf_path("cache");
        make_valid_pdf(&path, 1);
        let store = SessionStore::new();
        let opened = store
            .open_document(OpenDocumentRequest {
                path: path.to_string_lossy().to_string(),
                recover_if_damaged: false,
            })
            .unwrap();

        let first = store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        let second = store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        let zoomed = store
            .render_page(render_request(&opened.session_id, 0, 1.25))
            .unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert!(!zoomed.cache_hit);

        let diagnostics = store.diagnostics(opened.session_id.clone()).unwrap();
        assert_eq!(diagnostics.page_cache_entries, 2);
        assert_eq!(diagnostics.render_cache_hit_count, 1);
        assert_eq!(diagnostics.render_cache_miss_count, 2);

        store
            .increment_document_revision(&opened.session_id)
            .unwrap();
        let after_revision = store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        assert!(!after_revision.cache_hit);

        store.close_document(&opened.session_id).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repeated_render_smoke_uses_cache_after_first_render() {
        let path = temp_pdf_path("smoke");
        make_valid_pdf(&path, 5);
        let store = SessionStore::new();
        let opened = store
            .open_document(OpenDocumentRequest {
                path: path.to_string_lossy().to_string(),
                recover_if_damaged: false,
            })
            .unwrap();

        let first_start = std::time::Instant::now();
        store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        let first_elapsed = first_start.elapsed();

        let repeated_start = std::time::Instant::now();
        let repeated = store
            .render_page(render_request(&opened.session_id, 0, 1.0))
            .unwrap();
        let repeated_elapsed = repeated_start.elapsed();

        for page_index in 1..5 {
            store
                .render_page(render_request(&opened.session_id, page_index, 1.0))
                .unwrap();
        }
        store
            .render_page(render_request(&opened.session_id, 2, 1.5))
            .unwrap();

        let diagnostics = store.diagnostics(opened.session_id.clone()).unwrap();
        assert!(repeated.cache_hit);
        assert_eq!(diagnostics.parsed_document_open_count, 1);
        assert!(diagnostics.render_cache_hit_count >= 1);
        assert!(
            repeated_elapsed <= first_elapsed || repeated.render_time_ms == 0,
            "cache-hit render should avoid renderer work"
        );

        store.close_document(&opened.session_id).unwrap();
        let _ = std::fs::remove_file(path);
    }
}
