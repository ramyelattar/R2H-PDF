use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use mupdf::pdf::{PdfDocument, PdfPage};
use mupdf::{Colorspace, Document as MuDocument, Matrix, MetadataName, TextPageFlags};

use super::engine::{OpenedDocument, PdfEngine, SendDocument};
use super::errors::DocumentCoreError;
use super::recovery::RecoveryManager;
use super::types::{
    BBox, DocumentSummary, FontInfo, FormField, IncrementalSaveRequest, IncrementalSaveResponse,
    LineBbox, PageInfo, PageLines, PdfObjectSummary, RecoveryReport, RenderRequest, RenderResponse,
    TextExtractionRequest, TextExtractionResponse, TextSpan,
};
use super::vector::{extract_native_vectors, NativeVectorPageResult, NativeVectorRequest};

pub struct MuPdfEngine;

impl MuPdfEngine {
    pub fn new() -> Self {
        Self
    }
}

impl PdfEngine for MuPdfEngine {
    fn open(&self, path: &Path, recover_if_damaged: bool) -> Result<OpenedDocument, DocumentCoreError> {
        if cfg!(debug_assertions) { eprintln!("[document_core] open_pdf start path={}", path.display()); }
        let bytes = std::fs::read(path)?;

        let (bytes, repaired, recovery_report) = if bytes.starts_with(b"%PDF-") {
            (bytes, false, None)
        } else if recover_if_damaged {
            let (fixed, report) = RecoveryManager::recover_pdf_bytes(path, &bytes)?;
            (fixed, true, Some(report))
        } else {
            return Err(DocumentCoreError::InvalidPdf(format!(
                "{} does not begin with a PDF header",
                path.display()
            )));
        };

        let document = open_document_from_bytes(&bytes)?;
        let page_count = usize::try_from(document.page_count().map_err(map_mupdf_error)?).unwrap_or(0);
        if page_count == 0 {
            return Err(DocumentCoreError::InvalidPdf("document contains zero pages".to_string()));
        }

        let (pages, is_scanned) = load_page_infos(&document)?;
        let fonts = parse_fonts(&bytes);
        let objects = parse_objects(&bytes);
        let summary = DocumentSummary {
            page_count,
            object_count: objects.len(),
            title: read_metadata(&document, MetadataName::Title),
            author: read_metadata(&document, MetadataName::Author),
            producer: read_metadata(&document, MetadataName::Producer),
        };

        if cfg!(debug_assertions) {
            eprintln!(
                "[document_core] open_pdf success path={} pages={} repaired={}",
                path.display(),
                summary.page_count,
                repaired
            );
        }

        Ok(OpenedDocument {
            source_path: path.to_string_lossy().to_string(),
            document_hash: crc32_hex(&bytes),
            repaired,
            bytes,
            is_scanned,
            summary,
            pages,
            fonts,
            objects,
            recovery_report,
            parsed_document: Some(SendDocument(document)),
        })
    }

    fn render_page(
        &self,
        doc: &OpenedDocument,
        request: &RenderRequest,
    ) -> Result<RenderResponse, DocumentCoreError> {
        if cfg!(debug_assertions) {
            eprintln!(
                "[document_core] render_page start session={} page={} zoom={:.3}",
                request.session_id,
                request.page_index,
                request.zoom
            );
        }

        if request.page_index >= doc.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: request.page_index,
                total: doc.pages.len(),
            });
        }

        let start = Instant::now();
        let document = open_document_from_bytes(&doc.bytes)?;
        let page = document
            .load_page(i32::try_from(request.page_index).map_err(|e| DocumentCoreError::RenderError(e.to_string()))?)
            .map_err(map_mupdf_error)?;

        // Capture page bounds in PDF points BEFORE rendering so the frontend
        // can position the canvas hit-test overlay and the inline text editor
        // correctly when applying the PDF Y-up convention.
        let page_bounds = page.bounds().map_err(map_mupdf_error)?;
        let width_pts = page_bounds.width().abs();
        let height_pts = page_bounds.height().abs();

        let scale = (request.zoom.max(0.25) * request.device_pixel_ratio.max(1.0)).max(0.1);
        let matrix = Matrix::new_scale(scale, scale);
        let pixmap = page
            .to_pixmap(&matrix, &Colorspace::device_rgb(), true, true)
            .map_err(map_mupdf_error)?;

        let width_px = pixmap.width();
        let height_px = pixmap.height();
        let pixels_rgba = pixmap.samples().to_vec();
        let render_time_ms = start.elapsed().as_millis();
        let expected_len = (width_px as usize) * (height_px as usize) * 4;

        if width_px == 0 || height_px == 0 {
            return Err(DocumentCoreError::RenderError(format!(
                "renderer produced zero-size pixmap for session={} page={} ({}x{})",
                request.session_id, request.page_index, width_px, height_px,
            )));
        }
        if pixels_rgba.is_empty() {
            return Err(DocumentCoreError::RenderError(format!(
                "renderer produced an empty pixel buffer for session={} page={}",
                request.session_id, request.page_index,
            )));
        }
        if pixels_rgba.len() != expected_len {
            return Err(DocumentCoreError::RenderError(format!(
                "renderer pixel-buffer length mismatch for session={} page={}: got {} bytes, expected {}",
                request.session_id, request.page_index, pixels_rgba.len(), expected_len,
            )));
        }

        if cfg!(debug_assertions) {
            eprintln!(
                "[document_core] render_page success session={} page={} zoom={:.3} size={}x{} bytes={} time={}ms",
                request.session_id,
                request.page_index,
                request.zoom,
                width_px,
                height_px,
                pixels_rgba.len(),
                render_time_ms
            );
        }

        Ok(RenderResponse {
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            width_px,
            height_px,
            width: width_px,
            height: height_px,
            zoom: request.zoom,
            cache_hit: false,
            pixels_rgba,
            render_time_ms,
            width_pts,
            height_pts,
        })
    }

    fn extract_text(
        &self,
        doc: &OpenedDocument,
        request: &TextExtractionRequest,
    ) -> Result<TextExtractionResponse, DocumentCoreError> {
        if request.page_index >= doc.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: request.page_index,
                total: doc.pages.len(),
            });
        }

        let document = open_document_from_bytes(&doc.bytes)?;
        let page = document
            .load_page(i32::try_from(request.page_index).map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?)
            .map_err(map_mupdf_error)?;
        let text_page = page
            .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
            .map_err(map_mupdf_error)?;
        let full_text = text_page.to_text().map_err(map_mupdf_error)?;
        let spans = if full_text.trim().is_empty() {
            Vec::new()
        } else {
            vec![TextSpan {
                content: full_text.clone(),
                bbox: BBox {
                    x: 0.0,
                    y: 0.0,
                    width: doc
                        .pages
                        .get(request.page_index)
                        .map(|page| page.width_points)
                        .unwrap_or(0.0),
                    height: doc
                        .pages
                        .get(request.page_index)
                        .map(|page| page.height_points)
                        .unwrap_or(0.0),
                },
                font_name: None,
            }]
        };

        Ok(TextExtractionResponse {
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            full_text,
            spans,
        })
    }

    fn extract_native_vectors(
        &self,
        doc: &OpenedDocument,
        request: &NativeVectorRequest,
    ) -> Result<NativeVectorPageResult, DocumentCoreError> {
        extract_native_vectors(doc, request)
    }

    fn incremental_save(
        &self,
        doc: &OpenedDocument,
        request: &IncrementalSaveRequest,
    ) -> Result<IncrementalSaveResponse, DocumentCoreError> {
        let target_path = request
            .target_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| doc.source_path.clone());

        let target = Path::new(&target_path);
        let parent = target
            .parent()
            .ok_or_else(|| DocumentCoreError::SaveError("target has no parent directory".to_string()))?;

        std::fs::create_dir_all(parent)?;

        let tmp_name = format!(
            "{}.r2h.tmp",
            target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document.pdf")
        );
        let tmp_path = parent.join(tmp_name);

        let mut out = doc.bytes.clone();
        out.extend_from_slice(b"\n%R2H-INCREMENTAL-SAVE\n");
        out.extend_from_slice(format!("%SOURCE:{}\n", doc.source_path).as_bytes());
        out.extend_from_slice(b"%%EOF\n");

        {
            let mut file = std::fs::File::create(&tmp_path)?;
            use std::io::Write;
            file.write_all(&out)?;
            file.flush()?;
            if request.fsync {
                file.sync_all()?;
            }
        }

        // Windows-safe replacement strategy preserving prior file as backup.
        // 1) Move current target to .bak if it exists.
        // 2) Move temp file to target.
        // 3) Remove backup if replacement succeeded.
        let backup_path = parent.join(format!(
            "{}.r2h.bak",
            target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("document.pdf")
        ));

        let target_existed = target.exists();
        if target_existed {
            if backup_path.exists() {
                std::fs::remove_file(&backup_path)?;
            }
            std::fs::rename(target, &backup_path)?;
        }

        if let Err(err) = std::fs::rename(&tmp_path, target) {
            // Best-effort rollback: restore the previous target from backup.
            if target_existed && backup_path.exists() {
                let _ = std::fs::rename(&backup_path, target);
            }
            return Err(DocumentCoreError::SaveError(err.to_string()));
        }

        if target_existed && backup_path.exists() {
            std::fs::remove_file(&backup_path)?;
        }

        let integrity_hash = crc32_hex(&out);

        Ok(IncrementalSaveResponse {
            session_id: request.session_id.clone(),
            saved_to: target_path,
            bytes_written: out.len(),
            integrity_hash,
        })
    }

    fn recover_bytes(&self, path: &Path) -> Result<RecoveryReport, DocumentCoreError> {
        RecoveryManager::recover_file(path)
    }

    fn extract_all_pages_text(&self, doc: &OpenedDocument) -> Result<Vec<String>, DocumentCoreError> {
        // Parse the document once and extract text from all pages in a single
        // call, avoiding the per-page re-parse that plagued the old N-IPC path.
        let document = open_document_from_bytes(&doc.bytes)?;
        let page_count = doc.pages.len();
        let mut result = Vec::with_capacity(page_count);

        for page_index in 0..page_count {
            let page = document
                .load_page(
                    i32::try_from(page_index)
                        .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?,
                )
                .map_err(map_mupdf_error)?;
            let text_page = page
                .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
                .map_err(map_mupdf_error)?;
            let text = text_page.to_text().map_err(map_mupdf_error)?;
            result.push(text);
        }

        Ok(result)
    }

    fn extract_all_pages_with_line_bboxes(
        &self,
        doc: &OpenedDocument,
    ) -> Result<Vec<PageLines>, DocumentCoreError> {
        let document = open_document_from_bytes(&doc.bytes)?;
        let page_count = doc.pages.len();
        let mut out: Vec<PageLines> = Vec::with_capacity(page_count);

        for page_index in 0..page_count {
            let page_h = doc
                .pages
                .get(page_index)
                .map(|p| p.height_points)
                .unwrap_or(0.0);
            let page = document
                .load_page(
                    i32::try_from(page_index)
                        .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?,
                )
                .map_err(map_mupdf_error)?;
            let text_page = page
                .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
                .map_err(map_mupdf_error)?;

            let mut lines = Vec::new();
            for block in text_page.blocks() {
                // Only text blocks have characters; image blocks are skipped.
                if !matches!(block.r#type(), mupdf::text_page::TextBlockType::Text) {
                    continue;
                }
                for line in block.lines() {
                    let mut line_text = String::new();
                    for ch in line.chars() {
                        if let Some(c) = ch.char() {
                            line_text.push(c);
                        }
                    }
                    let trimmed = line_text.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let r = line.bounds();
                    // MuPDF text bboxes are returned with origin at the top-left
                    // (PDF convention for text-page coords). The rest of the app
                    // expects PDF user-space (origin bottom-left), so we flip Y.
                    let x0 = r.x0;
                    let x1 = r.x1;
                    let y0_top = r.y0;
                    let y1_top = r.y1;
                    let y0 = page_h - y1_top;
                    let y1 = page_h - y0_top;
                    lines.push(LineBbox {
                        text: trimmed,
                        bbox: [x0, y0, x1, y1],
                    });
                }
            }
            out.push(PageLines { page_index, lines });
        }
        Ok(out)
    }

    fn list_form_fields(&self, doc: &OpenedDocument) -> Result<Vec<FormField>, DocumentCoreError> {
        // Open as a PdfDocument so we can access the AcroForm dictionary.
        let pdf_doc = open_pdf_document_from_bytes(&doc.bytes)?;

        // Fast-path: no AcroForm means no fields.
        let has_form = pdf_doc.has_acro_form().map_err(map_mupdf_error)?;
        if !has_form {
            return Ok(Vec::new());
        }

        // Build a page-number → page_index lookup by iterating the page tree.
        // MuPDF's `find_page` returns the page object for a given 0-based index,
        // so we build a reverse map: object number → page_index.
        let page_count = doc.pages.len();
        let mut page_obj_to_index: std::collections::HashMap<i32, usize> =
            std::collections::HashMap::with_capacity(page_count);
        for page_idx in 0..page_count {
            if let Ok(page_obj) = pdf_doc.find_page(page_idx as i32) {
                if let Ok(obj_num) = page_obj.as_indirect() {
                    page_obj_to_index.insert(obj_num, page_idx);
                }
            }
        }

        let mut fields = Vec::new();

        // Walk the AcroForm /Fields array.
        let trailer = pdf_doc.trailer().map_err(map_mupdf_error)?;
        let root = match trailer.get_dict("Root").map_err(map_mupdf_error)? {
            Some(r) => r,
            None => return Ok(fields),
        };
        let acro_form = match root.get_dict("AcroForm").map_err(map_mupdf_error)? {
            Some(a) => a,
            None => return Ok(fields),
        };
        let top_fields = match acro_form.get_dict("Fields").map_err(map_mupdf_error)? {
            Some(f) => f,
            None => return Ok(fields),
        };

        // Recursively collect all terminal field nodes (widgets).
        collect_fields(&top_fields, &page_obj_to_index, &mut fields);

        Ok(fields)
    }
}

fn map_mupdf_error(error: mupdf::Error) -> DocumentCoreError {
    DocumentCoreError::RenderError(error.to_string())
}

fn open_document_from_bytes(bytes: &[u8]) -> Result<MuDocument, DocumentCoreError> {
    MuDocument::from_bytes(bytes, "pdf").map_err(map_mupdf_error)
}

fn open_pdf_document_from_bytes(bytes: &[u8]) -> Result<PdfDocument, DocumentCoreError> {
    PdfDocument::from_bytes(bytes).map_err(map_mupdf_error)
}

fn read_metadata(document: &MuDocument, key: MetadataName) -> Option<String> {
    document
        .metadata(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn load_page_infos(document: &MuDocument) -> Result<(Vec<PageInfo>, bool), DocumentCoreError> {
    let page_count = usize::try_from(document.page_count().map_err(map_mupdf_error)?).unwrap_or(0);
    let mut pages = Vec::with_capacity(page_count);
    let mut detected_text = false;

    for page_index in 0..page_count {
        let page = document
            .load_page(i32::try_from(page_index).map_err(|e| DocumentCoreError::InvalidPdf(e.to_string()))?)
            .map_err(map_mupdf_error)?;
        let bounds = page.bounds().map_err(map_mupdf_error)?;
        let has_text = if page_index < 3 {
            let text_page = page
                .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
                .map_err(map_mupdf_error)?;
            let text = text_page.to_text().map_err(map_mupdf_error)?;
            let page_has_text = !text.trim().is_empty();
            if page_has_text {
                detected_text = true;
            }
            page_has_text
        } else {
            detected_text
        };
        let rotation = PdfPage::try_from(page)
            .map_err(map_mupdf_error)?
            .rotation()
            .map_err(map_mupdf_error)?;
        pages.push(PageInfo {
            index: page_index,
            width_points: bounds.width(),
            height_points: bounds.height(),
            rotation,
            has_text,
        });
    }

    Ok((pages, !detected_text))
}

fn parse_fonts(bytes: &[u8]) -> Vec<FontInfo> {
    let mut fonts = HashSet::new();
    let mut cursor = 0_usize;
    let needle = b"/BaseFont/";

    while let Some(pos) = find_subsequence(&bytes[cursor..], needle) {
        let start = cursor + pos + needle.len();
        let end = bytes[start..]
            .iter()
            .position(|b| b.is_ascii_whitespace() || *b == b'/' || *b == b'>')
            .map(|v| start + v)
            .unwrap_or(bytes.len());

        if let Ok(name) = std::str::from_utf8(&bytes[start..end]) {
            if !name.is_empty() {
                fonts.insert(name.to_string());
            }
        }

        cursor = end;
        if cursor >= bytes.len() {
            break;
        }
    }

    if fonts.is_empty() {
        return vec![FontInfo {
            name: "Unknown".to_string(),
            embedded: false,
            subset: false,
        }];
    }

    fonts
        .into_iter()
        .map(|name| {
            let subset = name.contains('+');
            FontInfo {
                name,
                embedded: true,
                subset,
            }
        })
        .collect()
}

fn parse_objects(bytes: &[u8]) -> Vec<PdfObjectSummary> {
    let mut objects = Vec::new();
    let needle = b" obj";
    let mut cursor = 0_usize;

    while let Some(pos) = find_subsequence(&bytes[cursor..], needle) {
        let absolute = cursor + pos;
        let prefix_start = absolute.saturating_sub(24);
        let prefix = &bytes[prefix_start..absolute];
        let prefix_text = String::from_utf8_lossy(prefix);
        let mut parts = prefix_text
            .split_ascii_whitespace()
            .rev()
            .take(2)
            .collect::<Vec<_>>();
        parts.reverse();

        let object_number = parts
            .first()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(objects.len() as u32 + 1);
        let generation = parts
            .get(1)
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(0);

        // Scan the object body (up to 512 bytes after "obj") for a /Type /Name entry
        let body_start = absolute + needle.len();
        let body_end = (body_start + 512).min(bytes.len());
        let type_hint = detect_type_hint(&bytes[body_start..body_end]);

        objects.push(PdfObjectSummary {
            object_number,
            generation,
            byte_offset_hint: absolute,
            type_hint,
        });

        cursor = absolute + needle.len();
        if cursor >= bytes.len() {
            break;
        }
    }

    if objects.is_empty() {
        objects.push(PdfObjectSummary {
            object_number: 1,
            generation: 0,
            byte_offset_hint: 0,
            type_hint: "Synthetic".to_string(),
        });
    }

    objects
}

/// Scan a short window of object body bytes for `/Type /Name` and return a
/// human-readable type hint.  Recognises Stream (via `stream` keyword),
/// Catalog, Page, and Font type entries.
fn detect_type_hint(body: &[u8]) -> String {
    // Check for /Type /<Name> pattern
    if let Some(type_pos) = find_subsequence(body, b"/Type") {
        let after_type = &body[type_pos + 5..];
        // Skip whitespace to find the name
        let name_start = after_type.iter().position(|b| !b.is_ascii_whitespace());
        if let Some(ns) = name_start {
            let rest = &after_type[ns..];
            if rest.first() == Some(&b'/') {
                // Extract the name after the /
                let name_bytes = &rest[1..];
                let name_end = name_bytes
                    .iter()
                    .position(|b| b.is_ascii_whitespace() || *b == b'/' || *b == b'>' || *b == b'[' || *b == b'(')
                    .unwrap_or(name_bytes.len());
                if let Ok(name) = std::str::from_utf8(&name_bytes[..name_end]) {
                    match name {
                        "Catalog" => return "Catalog".to_string(),
                        "Page" => return "Page".to_string(),
                        "Pages" => return "Page".to_string(),
                        "Font" => return "Font".to_string(),
                        _ => return name.to_string(),
                    }
                }
            }
        }
    }

    // Check for stream keyword indicating a stream object
    if find_subsequence(body, b"stream").is_some() {
        return "Stream".to_string();
    }

    "Unknown".to_string()
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack.windows(needle.len()).position(|window| window == needle)
}

fn crc32_hex(bytes: &[u8]) -> String {
    format!("{:08x}", crc32fast::hash(bytes))
}

/// Recursively walk a PDF field node (or array of field nodes) and push a
/// `FormField` for every terminal widget found.
///
/// The PDF AcroForm tree can be arbitrarily deep: each node may have a
/// `/Kids` array containing either child field nodes or widget annotations.
/// A node is a terminal widget when it has a `/Subtype /Widget` entry or
/// when it has no `/Kids` array.
fn collect_fields(
    node: &mupdf::pdf::PdfObject,
    page_map: &std::collections::HashMap<i32, usize>,
    out: &mut Vec<FormField>,
) {
    // If this is an array, iterate its elements.
    if let Ok(true) = node.is_array() {
        let len = node.len().unwrap_or(0);
        for i in 0..len {
            if let Ok(Some(child)) = node.get_array(i as i32) {
                let resolved = child.resolve().ok().flatten().unwrap_or(child);
                collect_fields(&resolved, page_map, out);
            }
        }
        return;
    }

    // Check for /Kids — if present, recurse into them.
    if let Ok(Some(kids)) = node.get_dict("Kids") {
        let len = kids.len().unwrap_or(0);
        for i in 0..len {
            if let Ok(Some(kid)) = kids.get_array(i as i32) {
                let resolved = kid.resolve().ok().flatten().unwrap_or(kid);
                collect_fields(&resolved, page_map, out);
            }
        }
        return;
    }

    // Terminal node — extract field metadata.
    // Field name: /T key (partial name).
    let name = node
        .get_dict("T")
        .ok()
        .flatten()
        .and_then(|t| t.as_string().ok().map(|s| s.to_string()))
        .unwrap_or_default();

    // Field type: /FT key — /Tx, /Btn, /Ch, /Sig.
    let field_type = node
        .get_dict_inheritable("FT")
        .ok()
        .flatten()
        .and_then(|ft| ft.as_name().ok().map(|b| String::from_utf8_lossy(b).into_owned()))
        .map(|ft| match ft.as_str() {
            "Tx" => "text".to_string(),
            "Btn" => {
                // Distinguish checkbox vs radio via /Ff bit 16 (radio button flag).
                let ff = node
                    .get_dict_inheritable("Ff")
                    .ok()
                    .flatten()
                    .and_then(|f| f.as_int().ok())
                    .unwrap_or(0);
                if ff & (1 << 15) != 0 {
                    "radio".to_string()
                } else {
                    "checkbox".to_string()
                }
            }
            "Sig" => "signature".to_string(),
            other => other.to_lowercase(),
        })
        .unwrap_or_else(|| "text".to_string());

    // Current value: /V key.
    let value = node
        .get_dict("V")
        .ok()
        .flatten()
        .and_then(|v| {
            // /V can be a string or a name (e.g. /Yes for checkboxes).
            v.as_string()
                .ok()
                .map(|s| s.to_string())
                .or_else(|| v.as_name().ok().map(|b| String::from_utf8_lossy(b).into_owned()))
        })
        .unwrap_or_default();

    // Page index: look up the /P (page reference) indirect object number.
    let page_index = node
        .get_dict("P")
        .ok()
        .flatten()
        .and_then(|p| p.as_indirect().ok())
        .and_then(|num| page_map.get(&num).copied())
        .unwrap_or(0);

    // Bounding rect: /Rect array [x0, y0, x1, y1].
    let rect = node
        .get_dict("Rect")
        .ok()
        .flatten()
        .map(|r| {
            let x0 = r.get_array(0).ok().flatten().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            let y0 = r.get_array(1).ok().flatten().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            let x1 = r.get_array(2).ok().flatten().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            let y1 = r.get_array(3).ok().flatten().and_then(|v| v.as_float().ok()).unwrap_or(0.0);
            [x0, y0, x1, y1]
        })
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);

    out.push(FormField {
        name,
        field_type,
        value,
        page_index,
        rect,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Validates: Requirements 18**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn prop_crc32_hex_is_deterministic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let h1 = crc32_hex(&bytes);
            let h2 = crc32_hex(&bytes);
            prop_assert_eq!(h1, h2);
        }

        #[test]
        fn prop_crc32_hex_is_8_lowercase_hex_chars(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let h = crc32_hex(&bytes);
            prop_assert_eq!(h.len(), 8);
            prop_assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        }
    }

    // **Validates: Requirements 9.1**
    // Property 16: merging N copies of a 1-page PDF yields a document with N pages.

    /// Generate a valid 1-page PDF using MuPDF's own API so the output is
    /// guaranteed to be parseable by MuPDF without xref repair issues.
    fn make_valid_one_page_pdf() -> Vec<u8> {
        let mut pdf = PdfDocument::new();
        pdf.new_page(mupdf::Size { width: 612.0, height: 792.0 })
            .expect("create blank page");
        let mut buf: Vec<u8> = Vec::new();
        pdf.write_to(&mut buf).expect("serialize PDF");
        buf
    }

    // Validates the blank-render guard added to render_page: a zero-size or
    // empty pixmap must NEVER be returned as a successful render. The viewer
    // relies on this invariant — without it, the canvas paints a blank white
    // rectangle and the user has no idea anything went wrong.
    #[test]
    fn render_page_emits_a_non_empty_rgba_buffer_for_a_valid_pdf() {
        let pdf_bytes = make_valid_one_page_pdf();
        let document = open_document_from_bytes(&pdf_bytes).expect("open synthetic pdf");
        let (pages, _is_scanned) = load_page_infos(&document).expect("load page infos");
        let summary = DocumentSummary {
            page_count: pages.len(),
            object_count: 0,
            title: None,
            author: None,
            producer: None,
        };
        let doc = OpenedDocument {
            source_path: "memory://render-test".to_string(),
            document_hash: crc32_hex(&pdf_bytes),
            repaired: false,
            bytes: pdf_bytes,
            is_scanned: false,
            summary,
            pages,
            fonts: vec![],
            objects: vec![],
            recovery_report: None,
            parsed_document: None,
        };

        let engine = MuPdfEngine::new();
        let request = RenderRequest {
            session_id: "render-test".to_string(),
            page_index: 0,
            zoom: 1.0,
            viewport: BBox { x: 0.0, y: 0.0, width: 612.0, height: 792.0 },
            device_pixel_ratio: 1.0,
        };

        let response = engine.render_page(&doc, &request).expect("render must succeed");
        assert!(response.width_px > 0, "width_px must be > 0");
        assert!(response.height_px > 0, "height_px must be > 0");
        assert!(!response.pixels_rgba.is_empty(), "pixel buffer must not be empty");
        let expected = (response.width_px as usize) * (response.height_px as usize) * 4;
        assert_eq!(
            response.pixels_rgba.len(),
            expected,
            "pixel buffer length must equal width * height * 4",
        );
    }

    // Negative test: a render request with a page index past the end of the
    // document must surface PageOutOfRange instead of returning a successful
    // zero-byte response that the frontend would paint as blank.
    #[test]
    fn render_page_rejects_out_of_range_page_index() {
        let pdf_bytes = make_valid_one_page_pdf();
        let document = open_document_from_bytes(&pdf_bytes).expect("open synthetic pdf");
        let (pages, _is_scanned) = load_page_infos(&document).expect("load page infos");
        let summary = DocumentSummary {
            page_count: pages.len(),
            object_count: 0,
            title: None,
            author: None,
            producer: None,
        };
        let doc = OpenedDocument {
            source_path: "memory://render-test".to_string(),
            document_hash: crc32_hex(&pdf_bytes),
            repaired: false,
            bytes: pdf_bytes,
            is_scanned: false,
            summary,
            pages,
            fonts: vec![],
            objects: vec![],
            recovery_report: None,
            parsed_document: None,
        };

        let engine = MuPdfEngine::new();
        let request = RenderRequest {
            session_id: "render-test".to_string(),
            page_index: 99,
            zoom: 1.0,
            viewport: BBox { x: 0.0, y: 0.0, width: 612.0, height: 792.0 },
            device_pixel_ratio: 1.0,
        };

        let err = engine.render_page(&doc, &request).expect_err("out-of-range page must error");
        assert!(
            matches!(err, DocumentCoreError::PageOutOfRange { .. }),
            "expected PageOutOfRange, got {err:?}",
        );
    }


}
