use std::path::Path;

use mupdf::Document as MuDocument;

use super::errors::DocumentCoreError;
use super::types::{
    DocumentSummary, FontInfo, FormField, IncrementalSaveRequest, IncrementalSaveResponse, PageInfo,
    PageLines, PdfObjectSummary, RecoveryReport, RenderRequest, RenderResponse,
    TextExtractionRequest, TextExtractionResponse,
};
use super::vector::{NativeVectorPageResult, NativeVectorRequest};

/// Wrapper around `mupdf::Document` that implements `Send`.
///
/// # Safety
/// `mupdf::Document` holds a raw `*mut fz_document` pointer tied to a
/// thread-local fz_context.  All synchronous Tauri commands execute on the
/// same main thread, so the document is never actually sent across threads.
/// The `Send` impl is required only to satisfy the `Arc<Mutex<DocumentSession>>`
/// bound — the Mutex is locked and released on the same thread.
pub struct SendDocument(pub MuDocument);

// SAFETY: See doc comment above. The document is only accessed from the
// Tauri command thread (sync commands). The Mutex ensures exclusive access.
unsafe impl Send for SendDocument {}

impl std::fmt::Debug for SendDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SendDocument").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct OpenedDocument {
    pub source_path: String,
    pub document_hash: String,
    pub repaired: bool,
    pub bytes: Vec<u8>,
    pub is_scanned: bool,
    pub summary: DocumentSummary,
    pub pages: Vec<PageInfo>,
    pub fonts: Vec<FontInfo>,
    pub objects: Vec<PdfObjectSummary>,
    #[allow(dead_code)]
    pub recovery_report: Option<RecoveryReport>,
    pub parsed_document: Option<SendDocument>,
}

pub trait PdfEngine: Send + Sync + 'static {
    fn open(&self, path: &Path, recover_if_damaged: bool) -> Result<OpenedDocument, DocumentCoreError>;

    fn render_page(
        &self,
        doc: &OpenedDocument,
        request: &RenderRequest,
    ) -> Result<RenderResponse, DocumentCoreError>;

    fn extract_text(
        &self,
        doc: &OpenedDocument,
        request: &TextExtractionRequest,
    ) -> Result<TextExtractionResponse, DocumentCoreError>;

    fn extract_native_vectors(
        &self,
        doc: &OpenedDocument,
        request: &NativeVectorRequest,
    ) -> Result<NativeVectorPageResult, DocumentCoreError>;

    fn incremental_save(
        &self,
        doc: &OpenedDocument,
        request: &IncrementalSaveRequest,
    ) -> Result<IncrementalSaveResponse, DocumentCoreError>;

    fn recover_bytes(&self, path: &Path) -> Result<RecoveryReport, DocumentCoreError>;

    /// Extract the full text of every page in a single call, returning one
    /// `String` per page.  Callers (e.g. the search-index builder) use this
    /// instead of issuing N individual `extract_text` IPC calls, each of which
    /// would otherwise re-parse the document from bytes.
    fn extract_all_pages_text(&self, doc: &OpenedDocument) -> Result<Vec<String>, DocumentCoreError>;

    /// Phase 25A: extract every page's text broken into individual lines, each
    /// with a bbox in PDF point space (origin bottom-left). Used by the
    /// document-compare engine to attach pixel-accurate bounding boxes to
    /// removed/modified change rows so the reviewer can place precise
    /// strikethrough / underline / highlight overlays.
    fn extract_all_pages_with_line_bboxes(
        &self,
        doc: &OpenedDocument,
    ) -> Result<Vec<PageLines>, DocumentCoreError>;

    /// Walk the AcroForm /Fields array and return a `FormField` descriptor for
    /// every widget found in the document.  Returns an empty `Vec` for
    /// documents that have no AcroForm.
    fn list_form_fields(&self, doc: &OpenedDocument) -> Result<Vec<FormField>, DocumentCoreError>;
}
