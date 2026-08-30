use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mupdf::pdf::{PdfAnnotationType, PdfDocument, PdfPage};
use mupdf::{Point, Rect};

use crate::annotation_core::{
    errors::AnnotationCoreError,
    types::{
        Annotation, AnnotationColor, AnnotationListResponse, AnnotationType,
        CreateAnnotationRequest, UpdateAnnotationRequest,
    },
};
use crate::document_core::session::DocumentSession;

/// Returns the current UTC time formatted as an ISO 8601 string
/// (`YYYY-MM-DDTHH:MM:SSZ`), used for annotation timestamps.
fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Manual formatting — no external chrono dependency needed for this resolution.
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days_since_epoch = secs / 86400;

    // Gregorian calendar conversion from days since 1970-01-01.
    let mut y = 1970u32;
    let mut remaining = days_since_epoch;
    loop {
        let days_in_year =
            if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                366
            } else {
                365
            };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &days in &month_days {
        if remaining < days as u64 {
            break;
        }
        remaining -= days as u64;
        month += 1;
    }
    let day = remaining as u32 + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, month, day, h, m, s
    )
}

pub struct AnnotationEngine {
    /// (session_id, annotation_id) → Annotation
    store: HashMap<String, HashMap<String, Annotation>>,
    counter: u64,
    source_by_session: HashMap<String, String>,
}

impl AnnotationEngine {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            counter: 0,
            source_by_session: HashMap::new(),
        }
    }

    fn sidecar_path(source_path: &str) -> Option<PathBuf> {
        let path = Path::new(source_path);
        let file_name = path.file_name()?.to_str()?;
        let sidecar_name = format!("{}.r2h.annotations.json", file_name);
        Some(path.with_file_name(sidecar_name))
    }

    fn persist_session(&self, session_id: &str) -> Result<(), AnnotationCoreError> {
        let Some(source) = self.source_by_session.get(session_id) else {
            return Ok(());
        };
        let Some(sidecar) = Self::sidecar_path(source) else {
            return Err(AnnotationCoreError::ExportFailed(
                "unable to derive annotation sidecar path".to_string(),
            ));
        };

        let entries = self
            .store
            .get(session_id)
            .map(|m| m.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        let payload = serde_json::to_vec_pretty(&entries)
            .map_err(|err| AnnotationCoreError::ExportFailed(err.to_string()))?;
        std::fs::write(sidecar, payload)
            .map_err(|err| AnnotationCoreError::ExportFailed(err.to_string()))
    }

    fn ensure_loaded_for_session(
        &mut self,
        session_id: &str,
        source_path: Option<&str>,
    ) -> Result<(), AnnotationCoreError> {
        let Some(source) = source_path else {
            return Ok(());
        };

        self.source_by_session
            .entry(session_id.to_string())
            .or_insert_with(|| source.to_string());

        if self.store.contains_key(session_id) {
            return Ok(());
        }

        let mut loaded = HashMap::<String, Annotation>::new();
        if let Some(sidecar) = Self::sidecar_path(source) {
            if sidecar.exists() {
                let bytes = std::fs::read(&sidecar)
                    .map_err(|err| AnnotationCoreError::ExportFailed(err.to_string()))?;
                let entries: Vec<Annotation> = serde_json::from_slice(&bytes)
                    .map_err(|err| AnnotationCoreError::ExportFailed(err.to_string()))?;
                for entry in entries {
                    loaded.insert(entry.id.clone(), entry);
                }
            }
        }

        self.store.insert(session_id.to_string(), loaded);
        Ok(())
    }

    fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("annot-{}", self.counter)
    }

    /// Validates rich-type fields for the given request.
    ///
    /// - `Stamp`: `stamp_name` must be one of the five recognised values.
    /// - `FreeText` with callout: `callout_points` must be present and non-empty.
    /// - `Ink`: `ink_paths` must be present and non-empty.
    fn validate_rich_fields(req: &CreateAnnotationRequest) -> Result<(), AnnotationCoreError> {
        use crate::annotation_core::types::AnnotationType;

        match &req.annot_type {
            AnnotationType::Stamp => {
                const VALID_STAMPS: &[&str] =
                    &["Approved", "Confidential", "Draft", "Rejected", "Void"];
                match &req.stamp_name {
                    None => {
                        return Err(AnnotationCoreError::InvalidType(
                            "Stamp annotation requires a stamp_name".to_string(),
                        ));
                    }
                    Some(name) if !VALID_STAMPS.contains(&name.as_str()) => {
                        return Err(AnnotationCoreError::InvalidType(format!(
                            "invalid stamp_name '{}'; must be one of: Approved, Confidential, Draft, Rejected, Void",
                            name
                        )));
                    }
                    _ => {}
                }
            }
            AnnotationType::FreeText => {
                // If callout_points is supplied it must be non-empty (2+ points for a line).
                if let Some(pts) = &req.callout_points {
                    if pts.is_empty() {
                        return Err(AnnotationCoreError::InvalidType(
                            "FreeText callout annotation requires at least one callout point"
                                .to_string(),
                        ));
                    }
                }
            }
            AnnotationType::Ink => match &req.ink_paths {
                None => {
                    return Err(AnnotationCoreError::InvalidType(
                        "Ink annotation requires ink_paths".to_string(),
                    ));
                }
                Some(paths) if paths.is_empty() => {
                    return Err(AnnotationCoreError::InvalidType(
                        "Ink annotation requires at least one ink path".to_string(),
                    ));
                }
                _ => {}
            },
            // Note, Highlight, Underline, Strikeout, Squiggly, Link, FileAttachment
            // have no additional required rich fields.
            _ => {}
        }
        Ok(())
    }

    pub fn create(
        &mut self,
        req: CreateAnnotationRequest,
    ) -> Result<Annotation, AnnotationCoreError> {
        self.create_with_session(req, None)
    }

    /// Create an annotation, optionally embedding it in the PDF document bytes
    /// via MuPDF when a session reference is provided.
    ///
    /// Rich annotation types (Note, FreeText with callout, Stamp, Ink) are
    /// applied to the PDF using MuPDF APIs so they persist in the document
    /// when saved.  All annotations are also persisted to the JSON sidecar.
    pub fn create_with_session(
        &mut self,
        req: CreateAnnotationRequest,
        session: Option<&Arc<Mutex<DocumentSession>>>,
    ) -> Result<Annotation, AnnotationCoreError> {
        Self::validate_rich_fields(&req)?;
        let _ = self.ensure_loaded_for_session(&req.session_id, req.source_path.as_deref());
        let id = self.next_id();
        let now = iso8601_now();

        // Apply annotation to PDF bytes via MuPDF if session is available.
        if let Some(session_arc) = session {
            if let Err(e) = Self::apply_annotation_to_pdf(&req, session_arc) {
                // Log but don't fail — the annotation still persists in the sidecar.
                eprintln!(
                    "[AnnotationEngine] MuPDF annotation creation warning: {}",
                    e
                );
            }
        }

        let annot = Annotation {
            id: id.clone(),
            session_id: req.session_id.clone(),
            page_index: req.page_index,
            annot_type: req.annot_type,
            color: req.color,
            contents: req.contents,
            author: req.author,
            rect: req.rect,
            created_at: now.clone(),
            modified_at: now,
            callout_points: req.callout_points,
            stamp_name: req.stamp_name,
            ink_paths: req.ink_paths,
        };
        self.store
            .entry(req.session_id)
            .or_default()
            .insert(id, annot.clone());
        let _ = self.persist_session(&annot.session_id);
        Ok(annot)
    }

    // ── MuPDF annotation creation ────────────────────────────────────────────

    /// Apply the annotation to the PDF document bytes held in the session.
    ///
    /// This uses MuPDF APIs to create the appropriate annotation type:
    /// - `Note`: creates a PDF_ANNOT_TEXT (sticky note) annotation
    /// - `FreeText` with callout: creates a FreeText annotation with callout intent and points
    /// - `Stamp`: creates a Stamp annotation with the specified stamp name
    /// - `Ink`: creates an Ink annotation with the specified stroke paths
    /// - Other types (Highlight, Underline, etc.): creates the corresponding MuPDF annotation
    fn apply_annotation_to_pdf(
        req: &CreateAnnotationRequest,
        session: &Arc<Mutex<DocumentSession>>,
    ) -> Result<(), String> {
        // Read current bytes from session.
        let bytes = {
            session
                .lock()
                .map_err(|_| "session lock poisoned".to_string())?
                .document
                .bytes
                .clone()
        };

        let new_bytes = Self::apply_annotation_to_bytes(req, &bytes)?;

        // Write back into the session only for the direct annotation command.
        session
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?
            .document
            .bytes = new_bytes;

        Ok(())
    }

    /// Apply one annotation to an independent PDF byte snapshot.
    ///
    /// Export uses this helper so a failed export never mutates the live
    /// document session before the output has been validated and committed.
    pub(crate) fn apply_annotation_to_bytes(
        req: &CreateAnnotationRequest,
        bytes: &[u8],
    ) -> Result<Vec<u8>, String> {
        if bytes.is_empty() {
            return Ok(bytes.to_vec());
        }

        let pdf = PdfDocument::from_bytes(bytes)
            .map_err(|e| format!("failed to open PDF for annotation: {e}"))?;

        // Load the target page.
        let page_count = pdf
            .page_count()
            .map_err(|e| format!("page_count error: {e}"))?;
        let page_count = usize::try_from(page_count).unwrap_or(0);
        if req.page_index >= page_count {
            return Err(format!(
                "PAGE_OUT_OF_RANGE: page_index {}, document has {} pages",
                req.page_index, page_count
            ));
        }

        let page_no = i32::try_from(req.page_index)
            .map_err(|e| format!("page index conversion error: {e}"))?;
        let fz_page = pdf
            .load_page(page_no)
            .map_err(|e| format!("load_page error: {e}"))?;
        let mut pdf_page =
            PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage conversion error: {e}"))?;

        // Map our AnnotationType to MuPDF's PdfAnnotationType.
        let mupdf_type = Self::to_mupdf_annot_type(&req.annot_type);

        // Create the annotation on the page.
        let mut annot = pdf_page
            .create_annotation(mupdf_type)
            .map_err(|e| format!("create_annotation error: {e}"))?;

        // Set the bounding rectangle.
        let [x0, y0, x1, y1] = req.rect;
        annot
            .set_rect(Rect::new(x0, y0, x1, y1))
            .map_err(|e| format!("set_rect error: {e}"))?;

        // Set the color (RGB).
        annot
            .set_color(mupdf::color::AnnotationColor::Rgb {
                red: req.color.r,
                green: req.color.g,
                blue: req.color.b,
            })
            .map_err(|e| format!("set_color error: {e}"))?;

        // Set author if non-empty.
        if !req.author.is_empty() {
            annot
                .set_author(&req.author)
                .map_err(|e| format!("set_author error: {e}"))?;
        }

        // Type-specific MuPDF configuration.
        match &req.annot_type {
            AnnotationType::Note => {
                // Note (sticky note) — set contents via the annotation object dict.
                Self::set_annot_contents_on_page(&pdf_page, &req.contents)?;
            }
            AnnotationType::FreeText => {
                // FreeText — set contents and optionally callout points.
                Self::set_annot_contents_on_page(&pdf_page, &req.contents)?;

                if let Some(callout_points) = &req.callout_points {
                    // Set the FreeText callout intent and callout line points.
                    annot
                        .set_intent(mupdf::pdf::Intent::FreetextCallout)
                        .map_err(|e| format!("set_intent callout error: {e}"))?;

                    Self::set_callout_points_on_page(&pdf_page, callout_points)?;
                }
            }
            AnnotationType::Stamp => {
                // Stamp — set the stamp name (/Name entry).
                if let Some(stamp_name) = &req.stamp_name {
                    Self::set_stamp_name_on_page(&pdf_page, stamp_name)?;
                }
                // Also set contents if provided.
                if !req.contents.is_empty() {
                    Self::set_annot_contents_on_page(&pdf_page, &req.contents)?;
                }
            }
            AnnotationType::Ink => {
                // Ink — set ink paths via the annotation object's /InkList array.
                if let Some(ink_paths) = &req.ink_paths {
                    Self::set_ink_paths_on_page(&pdf, &pdf_page, ink_paths)?;
                }
                // Also set contents if provided.
                if !req.contents.is_empty() {
                    Self::set_annot_contents_on_page(&pdf_page, &req.contents)?;
                }
            }
            _ => {
                // For other types (Highlight, Underline, etc.), set contents.
                if !req.contents.is_empty() {
                    Self::set_annot_contents_on_page(&pdf_page, &req.contents)?;
                }
            }
        }

        // Drop the annotation and page references before serialising.
        drop(annot);
        drop(pdf_page);

        // Serialise back to bytes.
        let mut new_bytes: Vec<u8> = Vec::new();
        pdf.write_to(&mut new_bytes)
            .map_err(|e| format!("failed to serialise PDF after annotation: {e}"))?;

        Ok(new_bytes)
    }

    /// Map our `AnnotationType` to MuPDF's `PdfAnnotationType`.
    fn to_mupdf_annot_type(annot_type: &AnnotationType) -> PdfAnnotationType {
        match annot_type {
            AnnotationType::Highlight => PdfAnnotationType::Highlight,
            AnnotationType::Underline => PdfAnnotationType::Underline,
            AnnotationType::Strikeout => PdfAnnotationType::StrikeOut,
            AnnotationType::Squiggly => PdfAnnotationType::Squiggly,
            AnnotationType::FreeText => PdfAnnotationType::FreeText,
            AnnotationType::Stamp => PdfAnnotationType::Stamp,
            AnnotationType::Ink => PdfAnnotationType::Ink,
            AnnotationType::Link => PdfAnnotationType::Link,
            AnnotationType::Note => PdfAnnotationType::Text,
            AnnotationType::FileAttachment => PdfAnnotationType::FileAttachment,
        }
    }

    /// Set /Contents on the last annotation in the page's /Annots array.
    fn set_annot_contents_on_page(page: &PdfPage, contents: &str) -> Result<(), String> {
        let page_obj = page.object();
        let annots = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?;
        if let Some(annots) = annots {
            let len = annots.len().map_err(|e| format!("Annots len error: {e}"))?;
            if len > 0 {
                if let Some(mut last_annot) = annots
                    .get_array(i32::try_from(len - 1).unwrap_or(0))
                    .map_err(|e| format!("get_array error: {e}"))?
                {
                    let contents_val = mupdf::pdf::PdfObject::new_string(contents)
                        .map_err(|e| format!("new_string Contents error: {e}"))?;
                    last_annot
                        .dict_put("Contents", contents_val)
                        .map_err(|e| format!("dict_put Contents error: {e}"))?;
                }
            }
        }
        Ok(())
    }

    /// Set callout line points (/CL array) on the last annotation in the page's /Annots array.
    ///
    /// The callout line is stored as a flat array of coordinates:
    /// - 2 points (4 values): [x1, y1, x2, y2] — line from callout to text box
    /// - 3 points (6 values): [x1, y1, x2, y2, x3, y3] — knee-joint callout
    fn set_callout_points_on_page(
        page: &PdfPage,
        callout_points: &[[f32; 2]],
    ) -> Result<(), String> {
        if callout_points.is_empty() {
            return Ok(());
        }

        let page_obj = page.object();
        let annots = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?;
        if let Some(annots) = annots {
            let len = annots.len().map_err(|e| format!("Annots len error: {e}"))?;
            if len > 0 {
                if let Some(mut last_annot) = annots
                    .get_array(i32::try_from(len - 1).unwrap_or(0))
                    .map_err(|e| format!("get_array error: {e}"))?
                {
                    // Get the document to create a new array.
                    let doc = last_annot
                        .document()
                        .ok_or_else(|| "no document bound to annotation object".to_string())?;

                    let mut cl_array = doc
                        .new_array()
                        .map_err(|e| format!("new_array CL error: {e}"))?;

                    for point in callout_points {
                        let x_val = mupdf::pdf::PdfObject::new_real(point[0])
                            .map_err(|e| format!("new_real x error: {e}"))?;
                        cl_array
                            .array_push(x_val)
                            .map_err(|e| format!("array_push x error: {e}"))?;
                        let y_val = mupdf::pdf::PdfObject::new_real(point[1])
                            .map_err(|e| format!("new_real y error: {e}"))?;
                        cl_array
                            .array_push(y_val)
                            .map_err(|e| format!("array_push y error: {e}"))?;
                    }

                    last_annot
                        .dict_put("CL", cl_array)
                        .map_err(|e| format!("dict_put CL error: {e}"))?;

                    // Set /IT (intent) to /FreeTextCallout.
                    let intent_name = mupdf::pdf::PdfObject::new_name("FreeTextCallout")
                        .map_err(|e| format!("new_name IT error: {e}"))?;
                    last_annot
                        .dict_put("IT", intent_name)
                        .map_err(|e| format!("dict_put IT error: {e}"))?;
                }
            }
        }
        Ok(())
    }

    /// Set the stamp name (/Name entry) on the last annotation in the page's /Annots array.
    fn set_stamp_name_on_page(page: &PdfPage, stamp_name: &str) -> Result<(), String> {
        let page_obj = page.object();
        let annots = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?;
        if let Some(annots) = annots {
            let len = annots.len().map_err(|e| format!("Annots len error: {e}"))?;
            if len > 0 {
                if let Some(mut last_annot) = annots
                    .get_array(i32::try_from(len - 1).unwrap_or(0))
                    .map_err(|e| format!("get_array error: {e}"))?
                {
                    let name_val = mupdf::pdf::PdfObject::new_name(stamp_name)
                        .map_err(|e| format!("new_name stamp error: {e}"))?;
                    last_annot
                        .dict_put("Name", name_val)
                        .map_err(|e| format!("dict_put Name error: {e}"))?;
                }
            }
        }
        Ok(())
    }

    /// Set ink paths (/InkList array) on the last annotation in the page's /Annots array.
    ///
    /// The InkList is an array of arrays, where each inner array is a sequence of
    /// alternating x, y coordinates representing a single stroke path.
    fn set_ink_paths_on_page(
        pdf: &PdfDocument,
        page: &PdfPage,
        ink_paths: &[Vec<[f32; 2]>],
    ) -> Result<(), String> {
        if ink_paths.is_empty() {
            return Ok(());
        }

        let page_obj = page.object();
        let annots = page_obj
            .get_dict("Annots")
            .map_err(|e| format!("get Annots error: {e}"))?;
        if let Some(annots) = annots {
            let len = annots.len().map_err(|e| format!("Annots len error: {e}"))?;
            if len > 0 {
                if let Some(mut last_annot) = annots
                    .get_array(i32::try_from(len - 1).unwrap_or(0))
                    .map_err(|e| format!("get_array error: {e}"))?
                {
                    let doc = last_annot
                        .document()
                        .ok_or_else(|| "no document bound to annotation object".to_string())?;

                    // Create the outer InkList array.
                    let mut ink_list = doc
                        .new_array()
                        .map_err(|e| format!("new_array InkList error: {e}"))?;

                    for path in ink_paths {
                        // Each stroke is a flat array of coordinates [x0, y0, x1, y1, ...].
                        let mut stroke_array = doc
                            .new_array()
                            .map_err(|e| format!("new_array stroke error: {e}"))?;

                        for point in path {
                            let x_val = mupdf::pdf::PdfObject::new_real(point[0])
                                .map_err(|e| format!("new_real ink x error: {e}"))?;
                            stroke_array
                                .array_push(x_val)
                                .map_err(|e| format!("array_push ink x error: {e}"))?;
                            let y_val = mupdf::pdf::PdfObject::new_real(point[1])
                                .map_err(|e| format!("new_real ink y error: {e}"))?;
                            stroke_array
                                .array_push(y_val)
                                .map_err(|e| format!("array_push ink y error: {e}"))?;
                        }

                        ink_list
                            .array_push(stroke_array)
                            .map_err(|e| format!("array_push stroke error: {e}"))?;
                    }

                    last_annot
                        .dict_put("InkList", ink_list)
                        .map_err(|e| format!("dict_put InkList error: {e}"))?;
                }
            }
        }
        Ok(())
    }

    pub fn update(
        &mut self,
        req: UpdateAnnotationRequest,
    ) -> Result<Annotation, AnnotationCoreError> {
        self.ensure_loaded_for_session(&req.session_id, req.source_path.as_deref())?;
        let session_map = self
            .store
            .get_mut(&req.session_id)
            .ok_or_else(|| AnnotationCoreError::SessionNotFound(req.session_id.clone()))?;

        let annot = session_map
            .get_mut(&req.annotation_id)
            .ok_or_else(|| AnnotationCoreError::AnnotationNotFound(req.annotation_id.clone()))?;

        if let Some(c) = req.color {
            annot.color = c;
        }
        if let Some(t) = req.contents {
            annot.contents = t;
        }
        if let Some(r) = req.rect {
            annot.rect = r;
        }
        annot.modified_at = iso8601_now();

        let updated = annot.clone();
        self.persist_session(&req.session_id)?;
        Ok(updated)
    }

    pub fn delete(
        &mut self,
        session_id: &str,
        annotation_id: &str,
    ) -> Result<(), AnnotationCoreError> {
        let source = self.source_by_session.get(session_id).cloned();
        self.ensure_loaded_for_session(session_id, source.as_deref())?;
        let session_map = self
            .store
            .get_mut(session_id)
            .ok_or_else(|| AnnotationCoreError::SessionNotFound(session_id.to_string()))?;

        session_map
            .remove(annotation_id)
            .ok_or_else(|| AnnotationCoreError::AnnotationNotFound(annotation_id.to_string()))?;

        self.persist_session(session_id)?;
        Ok(())
    }

    pub fn list(&mut self, session_id: &str, page_index: Option<usize>) -> AnnotationListResponse {
        let source = self.source_by_session.get(session_id).cloned();
        let _ = self.ensure_loaded_for_session(session_id, source.as_deref());
        let annotations: Vec<Annotation> = self
            .store
            .get(session_id)
            .map(|m| {
                m.values()
                    .filter(|a| page_index.is_none_or(|p| a.page_index == p))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let total = annotations.len();
        AnnotationListResponse {
            session_id: session_id.to_string(),
            page_index,
            annotations,
            total,
        }
    }

    /// Generates a standards-compliant FDF 1.2 file containing all annotations
    /// for the given session.
    ///
    /// The output conforms to the FDF specification (PDF Reference, §12.7.7):
    /// - Starts with `%FDF-1.2` header
    /// - Contains a catalog object with `/FDF` dictionary holding `/Annots` array
    /// - Each annotation entry includes `/Type`, `/Subtype`, `/Rect`, `/C` (color),
    ///   `/Contents`, `/Page` (0-based page index), and type-specific fields
    /// - Ends with a valid xref table, trailer, and `%%EOF`
    ///
    /// The produced FDF is parseable by Adobe Acrobat Reader and MuPDF.
    pub fn export_fdf(&mut self, session_id: &str) -> Result<String, AnnotationCoreError> {
        let annots = self.list(session_id, None).annotations;

        // Build each annotation dictionary entry using PDF dict syntax.
        let mut annot_entries: Vec<String> = Vec::with_capacity(annots.len());
        for annot in &annots {
            let subtype = Self::annot_type_to_pdf_name(&annot.annot_type);
            let [x0, y0, x1, y1] = annot.rect;
            let r = annot.color.r;
            let g = annot.color.g;
            let b = annot.color.b;

            // Escape PDF literal string: backslash, parentheses, and control chars.
            let contents_escaped = Self::escape_pdf_string(&annot.contents);

            // FDF uses /Page with a 0-based page index (per PDF Reference §12.7.7.3).
            let page_index = annot.page_index;

            // Start building the annotation dict.
            let mut dict_parts: Vec<String> = Vec::new();
            dict_parts.push("/Type /Annot".to_string());
            dict_parts.push(format!("/Subtype /{subtype}"));
            dict_parts.push(format!("/Rect [{x0:.4} {y0:.4} {x1:.4} {y1:.4}]"));
            dict_parts.push(format!("/Contents ({contents_escaped})"));
            dict_parts.push(format!("/C [{r:.4} {g:.4} {b:.4}]"));
            dict_parts.push(format!("/Page {page_index}"));

            // Type-specific fields.
            match &annot.annot_type {
                AnnotationType::Stamp => {
                    if let Some(ref stamp_name) = annot.stamp_name {
                        dict_parts.push(format!("/Name /{stamp_name}"));
                    }
                }
                AnnotationType::FreeText => {
                    if let Some(ref callout_points) = annot.callout_points {
                        if !callout_points.is_empty() {
                            // /IT /FreeTextCallout
                            dict_parts.push("/IT /FreeTextCallout".to_string());
                            // /CL [x1 y1 x2 y2 ...]
                            let cl_values: Vec<String> = callout_points
                                .iter()
                                .flat_map(|p| vec![format!("{:.4}", p[0]), format!("{:.4}", p[1])])
                                .collect();
                            dict_parts.push(format!("/CL [{}]", cl_values.join(" ")));
                        }
                    }
                }
                AnnotationType::Ink => {
                    if let Some(ref ink_paths) = annot.ink_paths {
                        if !ink_paths.is_empty() {
                            // /InkList [[x0 y0 x1 y1 ...] [x0 y0 ...] ...]
                            let strokes: Vec<String> = ink_paths
                                .iter()
                                .map(|path| {
                                    let coords: Vec<String> = path
                                        .iter()
                                        .flat_map(|p| {
                                            vec![format!("{:.4}", p[0]), format!("{:.4}", p[1])]
                                        })
                                        .collect();
                                    format!("[{}]", coords.join(" "))
                                })
                                .collect();
                            dict_parts.push(format!("/InkList [{}]", strokes.join(" ")));
                        }
                    }
                }
                _ => {}
            }

            let entry = format!("<< {} >>", dict_parts.join(" "));
            annot_entries.push(entry);
        }

        // Construct the FDF file structure.
        //
        // Structure (per PDF Reference §12.7.7):
        //   %FDF-1.2
        //   <binary comment for robustness>
        //   1 0 obj  << /FDF << /Annots [...] >> >>  endobj
        //   xref
        //   trailer << /Root 1 0 R >>
        //   startxref
        //   %%EOF

        // Header — identifies this as FDF version 1.2.
        // The binary comment line ensures transport systems treat the file as binary.
        // We use high-byte characters (>127) per PDF spec recommendation.
        let header = "%FDF-1.2\n%\u{00e2}\u{00e3}\u{00cf}\u{00d3}\n";

        // Object 1: the FDF catalog dictionary containing the /Annots array.
        let annots_array = if annot_entries.is_empty() {
            "[]".to_string()
        } else {
            format!(
                "[\n{}\n]",
                annot_entries
                    .iter()
                    .map(|e| format!("  {e}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let obj1_body = format!("1 0 obj\n<< /FDF << /Annots {annots_array} >> >>\nendobj\n");

        // Calculate byte offset of object 1 (immediately after header).
        let obj1_offset = header.len();

        // xref table — two entries: free-list head (obj 0) and object 1.
        // Per PDF spec, each xref entry is exactly 20 bytes: "nnnnnnnnnn ggggg k \r\n"
        // We use the space + \r\n variant for maximum compatibility.
        let xref_offset = header.len() + obj1_body.len();

        let xref = format!(
            "xref\n\
             0 2\n\
             0000000000 65535 f \r\n\
             {obj1_offset:010} 00000 n \r\n"
        );

        let trailer =
            format!("trailer\n<< /Size 2 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF");

        let fdf = format!("{header}{obj1_body}{xref}{trailer}");
        Ok(fdf)
    }

    /// Escapes a string for use as a PDF literal string (parenthesised).
    ///
    /// Handles backslash, parentheses, and common control characters per
    /// PDF Reference §7.3.4.2.
    fn escape_pdf_string(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        for ch in s.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '(' => out.push_str("\\("),
                ')' => out.push_str("\\)"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\x08' => out.push_str("\\b"),
                '\x0c' => out.push_str("\\f"),
                _ => out.push(ch),
            }
        }
        out
    }

    pub fn export_json(&mut self, session_id: &str) -> Result<String, AnnotationCoreError> {
        let annotations = self.list(session_id, None).annotations;
        serde_json::to_string_pretty(&annotations)
            .map_err(|err| AnnotationCoreError::ExportFailed(err.to_string()))
    }

    /// Parses an FDF string (as produced by `export_fdf`) and imports all annotation
    /// entries into the given session.  Returns the full `AnnotationListResponse`
    /// for the session after import.
    ///
    /// The FDF format we generate contains annotation dicts of the form:
    /// ```text
    /// << /Type /Annot /Subtype /Highlight /Rect [x0 y0 x1 y1]
    ///    /Contents (text) /C [r g b] /P page_ref >>
    /// ```
    /// We extract these with simple string parsing — no full PDF parser is needed.
    pub fn import_fdf(
        &mut self,
        session_id: &str,
        fdf_string: &str,
    ) -> Result<AnnotationListResponse, AnnotationCoreError> {
        // Collect all << ... >> blocks from the FDF string.
        let blocks = Self::extract_dict_blocks(fdf_string);

        for block in &blocks {
            // Skip the outer FDF catalog dict (it contains /FDF, not /Type /Annot).
            if !block.contains("/Type /Annot") && !block.contains("/Type/Annot") {
                continue;
            }

            // /Subtype /Name
            let subtype = Self::fdf_extract_name(block, "Subtype");
            let annot_type = match subtype.as_deref() {
                Some("Highlight") => AnnotationType::Highlight,
                Some("Underline") => AnnotationType::Underline,
                Some("StrikeOut") => AnnotationType::Strikeout,
                Some("Squiggly") => AnnotationType::Squiggly,
                Some("FreeText") => AnnotationType::FreeText,
                Some("Stamp") => AnnotationType::Stamp,
                Some("Ink") => AnnotationType::Ink,
                Some("Link") => AnnotationType::Link,
                Some("Text") => AnnotationType::Note,
                Some("FileAttachment") => AnnotationType::FileAttachment,
                _ => AnnotationType::Note, // fallback
            };

            // /Rect [x0 y0 x1 y1]
            let rect = Self::fdf_extract_rect(block).unwrap_or([0.0, 0.0, 0.0, 0.0]);

            // /Contents (text)
            let contents = Self::fdf_extract_string(block, "Contents").unwrap_or_default();

            // /C [r g b]
            let color = Self::fdf_extract_color(block).unwrap_or(AnnotationColor {
                r: 1.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            });

            // /Page page_index — stored as 0-based integer in FDF 1.2 (per spec).
            // Also handle legacy /P (1-based) for backward compatibility.
            let page_index = Self::fdf_extract_page_index(block)
                .or_else(|| {
                    Self::fdf_extract_page(block).map(|p| if p > 0 { p as usize - 1 } else { 0 })
                })
                .unwrap_or(0);

            let req = CreateAnnotationRequest {
                session_id: session_id.to_string(),
                source_path: None,
                page_index,
                annot_type,
                color,
                contents,
                author: String::new(),
                rect,
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            };

            // Ignore per-annotation errors so a single bad entry doesn't abort the import.
            let _ = self.create(req);
        }

        Ok(self.list(session_id, None))
    }

    // ── FDF parsing helpers ──────────────────────────────────────────────────

    /// Extracts all top-level `<< ... >>` dict blocks from an FDF string.
    /// Handles nesting so that inner dicts are included in the outer block.
    fn extract_dict_blocks(s: &str) -> Vec<String> {
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut blocks = Vec::new();
        let mut i = 0;

        while i + 1 < len {
            // Look for opening <<
            if bytes[i] == b'<' && bytes[i + 1] == b'<' {
                let start = i;
                let mut depth = 0usize;
                let mut j = i;
                while j + 1 < len {
                    if bytes[j] == b'<' && bytes[j + 1] == b'<' {
                        depth += 1;
                        j += 2;
                    } else if bytes[j] == b'>' && bytes[j + 1] == b'>' {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            blocks.push(s[start..j].to_string());
                            i = j;
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                if depth != 0 {
                    // Unmatched — skip past the opening <<
                    i += 2;
                }
            } else {
                i += 1;
            }
        }
        blocks
    }

    /// Extracts a PDF name value: `/Key /Name` → `"Name"`.
    fn fdf_extract_name(block: &str, key: &str) -> Option<String> {
        let pattern = format!("/{key}");
        let pos = block.find(&pattern)?;
        let after = block[pos + pattern.len()..].trim_start();
        let name: String = after
            .strip_prefix('/')?
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>' && *c != '[')
            .collect();
        Some(name)
    }

    /// Extracts `/Rect [x0 y0 x1 y1]` → `[x0, y0, x1, y1]`.
    fn fdf_extract_rect(block: &str) -> Option<[f32; 4]> {
        let pos = block.find("/Rect")?;
        let after = block[pos + 5..].trim_start();
        if !after.starts_with('[') {
            return None;
        }
        let end = after.find(']')?;
        let inner = &after[1..end];
        let nums: Vec<f32> = inner
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() >= 4 {
            Some([nums[0], nums[1], nums[2], nums[3]])
        } else {
            None
        }
    }

    /// Extracts a PDF literal string: `/Key (value)` → `"value"`.
    /// Handles the escape sequences we produce in `export_fdf`.
    fn fdf_extract_string(block: &str, key: &str) -> Option<String> {
        let pattern = format!("/{key}");
        let pos = block.find(&pattern)?;
        let after = block[pos + pattern.len()..].trim_start();
        if !after.starts_with('(') {
            return None;
        }
        // Walk forward collecting chars, respecting backslash escapes.
        let chars: Vec<char> = after.chars().collect();
        let mut result = String::new();
        let mut k = 1usize; // skip opening '('
        let mut depth = 1i32;
        while k < chars.len() {
            match chars[k] {
                '\\' if k + 1 < chars.len() => match chars[k + 1] {
                    '(' => {
                        result.push('(');
                        k += 2;
                    }
                    ')' => {
                        result.push(')');
                        k += 2;
                    }
                    '\\' => {
                        result.push('\\');
                        k += 2;
                    }
                    'n' => {
                        result.push('\n');
                        k += 2;
                    }
                    'r' => {
                        result.push('\r');
                        k += 2;
                    }
                    't' => {
                        result.push('\t');
                        k += 2;
                    }
                    _ => {
                        result.push(chars[k + 1]);
                        k += 2;
                    }
                },
                '(' => {
                    depth += 1;
                    result.push('(');
                    k += 1;
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    result.push(')');
                    k += 1;
                }
                c => {
                    result.push(c);
                    k += 1;
                }
            }
        }
        Some(result)
    }

    /// Extracts `/C [r g b]` → `AnnotationColor`.
    fn fdf_extract_color(block: &str) -> Option<AnnotationColor> {
        let pos = block.find("/C ")?;
        let after = block[pos + 3..].trim_start();
        if !after.starts_with('[') {
            return None;
        }
        let end = after.find(']')?;
        let inner = &after[1..end];
        let nums: Vec<f32> = inner
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() >= 3 {
            Some(AnnotationColor {
                r: nums[0],
                g: nums[1],
                b: nums[2],
                a: 1.0,
            })
        } else {
            None
        }
    }

    /// Extracts `/P page_ref` (integer) → page number (1-based legacy format).
    fn fdf_extract_page(block: &str) -> Option<u32> {
        let pos = block.find("/P ")?;
        let after = block[pos + 3..].trim_start();
        let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        num.parse().ok()
    }

    /// Extracts `/Page page_index` (integer) → 0-based page index (FDF 1.2 standard).
    fn fdf_extract_page_index(block: &str) -> Option<usize> {
        let pos = block.find("/Page ")?;
        let after = block[pos + 6..].trim_start();
        let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        num.parse().ok()
    }

    /// Maps an `AnnotationType` to the corresponding PDF annotation subtype name.
    fn annot_type_to_pdf_name(annot_type: &AnnotationType) -> &'static str {
        match annot_type {
            AnnotationType::Highlight => "Highlight",
            AnnotationType::Underline => "Underline",
            AnnotationType::Strikeout => "StrikeOut",
            AnnotationType::Squiggly => "Squiggly",
            AnnotationType::FreeText => "FreeText",
            AnnotationType::Stamp => "Stamp",
            AnnotationType::Ink => "Ink",
            AnnotationType::Link => "Link",
            AnnotationType::Note => "Text",
            AnnotationType::FileAttachment => "FileAttachment",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation_core::types::{
        AnnotationColor, AnnotationType, CreateAnnotationRequest, UpdateAnnotationRequest,
    };
    use proptest::prelude::*;

    fn make_color() -> AnnotationColor {
        AnnotationColor {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        }
    }

    fn make_create_req(session_id: &str, page_index: usize) -> CreateAnnotationRequest {
        CreateAnnotationRequest {
            session_id: session_id.to_string(),
            source_path: None,
            page_index,
            annot_type: AnnotationType::Highlight,
            color: make_color(),
            contents: "Test annotation".to_string(),
            author: "Tester".to_string(),
            rect: [10.0, 20.0, 100.0, 40.0],
            callout_points: None,
            stamp_name: None,
            ink_paths: None,
        }
    }

    // 1. Create an annotation and verify returned fields match the request.
    #[test]
    fn test_create_annotation() {
        let mut engine = AnnotationEngine::new();
        let req = make_create_req("session-1", 0);

        let annot = engine.create(req).expect("create should succeed");

        assert_eq!(annot.session_id, "session-1");
        assert_eq!(annot.page_index, 0);
        assert_eq!(annot.contents, "Test annotation");
        assert_eq!(annot.author, "Tester");
        assert_eq!(annot.rect, [10.0, 20.0, 100.0, 40.0]);
        assert!(!annot.id.is_empty());
        assert!(!annot.created_at.is_empty());
        assert!(!annot.modified_at.is_empty());
    }

    // 2. Create then update; verify updated fields are reflected.
    #[test]
    fn test_update_annotation() {
        let mut engine = AnnotationEngine::new();
        let annot = engine
            .create(make_create_req("session-2", 0))
            .expect("create should succeed");

        let update_req = UpdateAnnotationRequest {
            session_id: "session-2".to_string(),
            source_path: None,
            annotation_id: annot.id.clone(),
            color: Some(AnnotationColor {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            }),
            contents: Some("Updated contents".to_string()),
            rect: Some([5.0, 5.0, 50.0, 25.0]),
        };

        let updated = engine.update(update_req).expect("update should succeed");

        assert_eq!(updated.id, annot.id);
        assert_eq!(updated.contents, "Updated contents");
        assert_eq!(updated.rect, [5.0, 5.0, 50.0, 25.0]);
        assert!((updated.color.b - 1.0).abs() < f32::EPSILON);
        // modified_at should be set (non-empty)
        assert!(!updated.modified_at.is_empty());
    }

    // 3. Create then delete; verify the annotation is gone from the list.
    #[test]
    fn test_delete_annotation() {
        let mut engine = AnnotationEngine::new();
        let annot = engine
            .create(make_create_req("session-3", 0))
            .expect("create should succeed");

        engine
            .delete("session-3", &annot.id)
            .expect("delete should succeed");

        let list = engine.list("session-3", None);
        assert_eq!(list.total, 0);
        assert!(list.annotations.is_empty());
    }

    // 4. Create multiple annotations on different pages; list all returns all of them.
    #[test]
    fn test_list_all_annotations() {
        let mut engine = AnnotationEngine::new();

        engine.create(make_create_req("session-4", 0)).unwrap();
        engine.create(make_create_req("session-4", 1)).unwrap();
        engine.create(make_create_req("session-4", 2)).unwrap();

        let list = engine.list("session-4", None);
        assert_eq!(list.total, 3);
        assert_eq!(list.annotations.len(), 3);
        assert_eq!(list.page_index, None);
    }

    // 5. Create annotations on different pages; list by page_index returns only that page.
    #[test]
    fn test_list_single_page() {
        let mut engine = AnnotationEngine::new();

        engine.create(make_create_req("session-5", 0)).unwrap();
        engine.create(make_create_req("session-5", 0)).unwrap();
        engine.create(make_create_req("session-5", 1)).unwrap();

        let page0 = engine.list("session-5", Some(0));
        assert_eq!(page0.total, 2, "page 0 should have 2 annotations");
        assert!(page0.annotations.iter().all(|a| a.page_index == 0));

        let page1 = engine.list("session-5", Some(1));
        assert_eq!(page1.total, 1, "page 1 should have 1 annotation");
        assert!(page1.annotations.iter().all(|a| a.page_index == 1));

        let page2 = engine.list("session-5", Some(2));
        assert_eq!(page2.total, 0, "page 2 should have no annotations");
    }

    // 6. Create an annotation, export FDF, verify it starts with `%FDF-1.2`.
    #[test]
    fn test_fdf_export_starts_with_fdf_header() {
        let mut engine = AnnotationEngine::new();
        engine.create(make_create_req("session-6", 0)).unwrap();

        let fdf = engine
            .export_fdf("session-6")
            .expect("FDF export should succeed");

        assert!(
            fdf.starts_with("%FDF-1.2"),
            "FDF output must start with '%FDF-1.2', got: {:?}",
            &fdf[..fdf.len().min(20)]
        );
        // Also verify it ends with %%EOF
        assert!(
            fdf.trim_end().ends_with("%%EOF"),
            "FDF output must end with '%%EOF'"
        );
    }

    // Property 13, Req 7: for any annotation type + valid rect + content,
    // create then list returns it with matching fields.
    //
    // **Validates: Requirements 7**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn prop_create_then_list_round_trip(
            contents in "[a-zA-Z0-9 ]{1,100}",
            page_index in 0usize..10,
            x0 in 0.0f32..500.0,
            y0 in 0.0f32..500.0,
            x1 in 0.0f32..500.0,
            y1 in 0.0f32..500.0,
        ) {
            let mut engine = AnnotationEngine::new();
            let req = CreateAnnotationRequest {
                session_id: "prop-sess".to_string(),
                source_path: None,
                page_index,
                annot_type: AnnotationType::Highlight,
                color: AnnotationColor { r: 1.0, g: 1.0, b: 0.0, a: 1.0 },
                contents: contents.clone(),
                author: "test".to_string(),
                rect: [x0, y0, x1, y1],
                callout_points: None,
                stamp_name: None,
                ink_paths: None,
            };

            let created = engine.create(req).expect("create should succeed");
            let list = engine.list("prop-sess", None);

            prop_assert_eq!(list.total, 1);
            let found = &list.annotations[0];
            prop_assert_eq!(&found.id, &created.id);
            prop_assert_eq!(&found.contents, &contents);
            prop_assert_eq!(found.page_index, page_index);
            prop_assert_eq!(found.rect, [x0, y0, x1, y1]);
        }
    }
}
