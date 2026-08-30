//! Content stream analysis: extracts text spans, image XObjects, and paths
//! from a PDF page's content stream using MuPDF's structured text extraction.

use mupdf::text_page::TextBlockType;
use crate::document_core::DocumentCoreState;
use super::types::*;

/// Threshold above which a decoded text line is considered "garbled" by the
/// quality heuristic — i.e. so many U+FFFD replacement characters or other
/// unreadable glyphs that the text identity is unreliable and we must not
/// label the object with the decoded string.
const GARBLED_REPLACEMENT_RATIO: f32 = 0.20;

/// Classification of how reliable the decoded text is. The frontend uses
/// this to decide whether to label a row with the decoded string or with
/// a neutral fallback ("Text object"), and the editing engine treats
/// `Garbled` text identity as un-safe for native edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodingQuality {
    Ok,
    Partial,
    Garbled,
}

impl DecodingQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            DecodingQuality::Ok => "ok",
            DecodingQuality::Partial => "partial",
            DecodingQuality::Garbled => "garbled",
        }
    }
}

/// Honest assessment of how usable the decoded text is. Counts U+FFFD
/// replacement characters and other non-printable codepoints. A short
/// string is more sensitive than a long one (one bad glyph in five vs
/// ten in fifty).
pub fn assess_decoding_quality(text: &str) -> DecodingQuality {
    let total = text.chars().count();
    if total == 0 {
        return DecodingQuality::Ok;
    }
    let mut bad = 0usize;
    for c in text.chars() {
        let cp = c as u32;
        // U+FFFD REPLACEMENT CHARACTER is the smoking gun for a broken
        // ToUnicode / encoding decode. The C1 control range and Private
        // Use Area also count: those are what shows up when a font uses
        // unmapped glyph indices.
        if c == '\u{FFFD}'
            || (0x0080..=0x009F).contains(&cp)
            || (0xE000..=0xF8FF).contains(&cp)
        {
            bad += 1;
        }
    }
    let ratio = bad as f32 / total as f32;
    if ratio >= GARBLED_REPLACEMENT_RATIO {
        DecodingQuality::Garbled
    } else if bad > 0 {
        DecodingQuality::Partial
    } else {
        DecodingQuality::Ok
    }
}

/// Extract content objects from a page using MuPDF's text extraction device.
pub fn extract_page_content_objects(
    doc_state: &DocumentCoreState,
    session_id: &str,
    page_index: usize,
) -> Result<Vec<ContentObject>, String> {
    let arc = doc_state.store.get_session_arc_pub(session_id)
        .map_err(|e| e.to_string())?;
    let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;

    let pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let page_count = pdf.page_count().map_err(|e| format!("page_count: {e}"))?;
    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    if page_no >= page_count {
        return Err(format!("Page index {} out of range (total: {})", page_index, page_count));
    }

    let page = pdf.load_page(page_no).map_err(|e| format!("load_page: {e}"))?;
    // Capture page bounds in PDF points so we can flip Y coordinates from
    // MuPDF's text-page convention (origin top-left, Y down) to the PDF
    // user-space convention used by the rest of the app (origin bottom-left,
    // Y up). Without this flip the canvas hit-test overlay and inline text
    // editor place boxes at completely the wrong vertical position.
    let page_bounds = page.bounds().map_err(|e| format!("page bounds: {e}"))?;
    let page_h = page_bounds.height().abs();
    // Phase 25E: request vector block collection so simple paths
    // (rectangles, lines, filled/stroked shapes) appear alongside text and
    // image blocks. Without this flag MuPDF discards vector geometry.
    let text_page = page.to_text_page(mupdf::TextPageFlags::COLLECT_VECTORS)
        .map_err(|e| format!("to_text_page: {e}"))?;

    let mut objects: Vec<ContentObject> = Vec::new();
    let mut z_index = 0;
    let mut text_occurrence_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for block in text_page.blocks() {
        match block.r#type() {
            TextBlockType::Text => {
                for line in block.lines() {
                    let mut line_text = String::new();
                    let mut font_size: f32 = 0.0;
                    let mut glyph_count = 0;

                    for ch in line.chars() {
                        if let Some(c) = ch.char() {
                            line_text.push(c);
                            glyph_count += 1;
                        }
                        if font_size == 0.0 {
                            font_size = ch.size();
                        }
                    }

                    let text = line_text.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }

                    let bounds = line.bounds();
                    // MuPDF's text-page extraction returns bounds with the
                    // origin at the top-left (Y down). Every other system in
                    // this app — search, the inline editor, the canvas
                    // hit-test overlay — expects PDF user-space (Y up,
                    // origin bottom-left). Flip Y here so all downstream
                    // consumers can use the same coordinate system.
                    let bbox = [
                        bounds.x0,
                        page_h - bounds.y1,
                        bounds.x1,
                        page_h - bounds.y0,
                    ];

                    // Track occurrence index for repeated text.
                    let occurrence = text_occurrence_counts.entry(text.clone()).or_insert(0);
                    let current_occurrence = *occurrence;
                    *occurrence += 1;

                    // Determine editability based on text characteristics.
                    let (mut editable_level, mut diagnostics) =
                        classify_text_editability(&text, font_size);

                    // Honest text-decoding assessment. If we cannot trust the
                    // decoded glyph string (e.g. lots of U+FFFD replacement
                    // characters from a font with no usable ToUnicode and a
                    // non-standard encoding), we MUST NOT label this object
                    // with the garbled string and we MUST NOT allow a native
                    // text edit — we don't know the source characters.
                    let quality = assess_decoding_quality(&text);
                    if quality == DecodingQuality::Garbled {
                        diagnostics.push(
                            "Text encoding could not be decoded safely — original glyphs are unreadable, so this object cannot be edited natively.".to_string()
                        );
                        editable_level = EditableLevel::ReadOnly;
                    } else if quality == DecodingQuality::Partial {
                        diagnostics.push(
                            "Some glyphs in this text object could not be decoded — labels may be imprecise.".to_string()
                        );
                    }

                    let id = format!("co-{}-{}-text-{}", &session_id[..session_id.len().min(8)], page_index, z_index);

                    // Phase 28A — pick a granular editability strategy
                    // matching the EditableLevel classifier output.
                    let editable_strategy = match editable_level {
                        EditableLevel::NativeEditable => "native_in_place".to_string(),
                        EditableLevel::NativeReplaceable => "native_rebuild_span".to_string(),
                        EditableLevel::VisualPatchOnly => "safe_visual_replacement".to_string(),
                        EditableLevel::ReadOnly => "read_only".to_string(),
                    };
                    let mut unsupported_reason = if editable_level == EditableLevel::VisualPatchOnly {
                        diagnostics.clone()
                    } else {
                        Vec::new()
                    };
                    if quality == DecodingQuality::Garbled {
                        unsupported_reason.push(
                            "Decoded text identity unreliable; native edit refused.".to_string()
                        );
                    }

                    objects.push(ContentObject {
                        id,
                        session_id: session_id.to_string(),
                        page_index,
                        object_type: ContentObjectType::TextSpan,
                        bbox,
                        z_index,
                        editable_level,
                        text_info: Some(TextInfo {
                            raw_text: text.clone(),
                            decoded_text: text,
                            glyph_count,
                            font_name: "Unknown".to_string(),
                            font_size,
                            fill_color: None,
                            stroke_color: None,
                            writing_mode: "horizontal".to_string(),
                            is_subset_font: false,
                            encoding_safe: quality != DecodingQuality::Garbled,
                            operator_offset: None,
                            operator_length: None,
                            occurrence_index: current_occurrence,
                            // Phase 28A — MuPDF's text-page extraction does
                            // not expose operator identity; the
                            // stream_parser fills these in when the
                            // edit pipeline actually walks the content
                            // stream. We default to "unknown" here so the
                            // UI shows the right method badge.
                            operator_type: "unknown".to_string(),
                            content_stream_index: None,
                            operator_index: None,
                            editable_strategy,
                            unsupported_reason,
                            font_encoding: None,
                            decoding_quality: quality.as_str().to_string(),
                        }),
                        image_info: None,
                        style_info: Some(StyleInfo {
                            opacity: 1.0,
                            rotation: 0.0,
                            scale_x: 1.0,
                            scale_y: 1.0,
                        }),
                        diagnostics,
                    });

                    z_index += 1;
                }
            }
            TextBlockType::Image => {
                if let Some(transform) = block.ctm() {
                    // MuPDF CTM is in PDF user space (Y up), so the X/Y
                    // origin maps cleanly to PDF user-space bbox.
                    let x0 = transform.e;
                    let x1 = transform.e + transform.a.abs();
                    let y0 = transform.f;
                    let y1 = transform.f + transform.d.abs();
                    let bbox = [x0, y0, x1, y1];

                    let (width, height) = if let Some(img) = block.image() {
                        (img.width() as u32, img.height() as u32)
                    } else {
                        (0, 0)
                    };

                    let id = format!("co-{}-{}-img-{}", &session_id[..session_id.len().min(8)], page_index, z_index);

                    objects.push(ContentObject {
                        id,
                        session_id: session_id.to_string(),
                        page_index,
                        object_type: ContentObjectType::ImageXobject,
                        bbox,
                        z_index,
                        editable_level: EditableLevel::NativeReplaceable,
                        text_info: None,
                        image_info: Some(ImageInfo {
                            xobject_name: format!("Im{}", z_index),
                            width,
                            height,
                            color_space: "DeviceRGB".to_string(),
                            bits_per_component: 8,
                            transform_matrix: [
                                transform.a, transform.b, transform.c,
                                transform.d, transform.e, transform.f,
                            ],
                        }),
                        style_info: Some(StyleInfo {
                            opacity: 1.0,
                            rotation: 0.0,
                            scale_x: transform.a,
                            scale_y: transform.d,
                        }),
                        diagnostics: vec![],
                    });

                    z_index += 1;
                }
            }
            TextBlockType::Vector => {
                // Phase 25E: surface vector/path geometry as a content object
                // with honest editability. We only get the bbox from MuPDF —
                // not individual path points, stroke style, or fill colour.
                // Frontend will offer safe-visual-removal where supported.
                //
                // Same Y-flip as text spans: MuPDF reports text-page bounds
                // with Y down, the rest of the app uses PDF user-space Y up.
                let bounds = block.bounds();
                let bbox = [
                    bounds.x0,
                    page_h - bounds.y1,
                    bounds.x1,
                    page_h - bounds.y0,
                ];
                let width = (bbox[2] - bbox[0]).abs();
                let height = (bbox[3] - bbox[1]).abs();
                // Heuristic shape classification — fully honest, just based
                // on the bbox aspect ratio.
                let shape_hint = if width < 1.0 || height < 1.0 {
                    "line_or_thin_path"
                } else if (width / height - 1.0).abs() < 0.05 {
                    "square_path"
                } else {
                    "rectangle_path"
                };
                let id = format!(
                    "co-{}-{}-path-{}",
                    &session_id[..session_id.len().min(8)],
                    page_index,
                    z_index,
                );
                let mut diagnostics = vec![
                    format!("Detected vector path (shape hint: {shape_hint})."),
                    "Vector point editing is not implemented in this phase — read-only or safe visual removal only.".to_string(),
                ];
                if width < 0.5 && height < 0.5 {
                    diagnostics.push("Degenerate path bbox; may be noise.".to_string());
                }
                objects.push(ContentObject {
                    id,
                    session_id: session_id.to_string(),
                    page_index,
                    object_type: ContentObjectType::Path,
                    bbox,
                    z_index,
                    editable_level: EditableLevel::ReadOnly,
                    text_info: None,
                    image_info: None,
                    style_info: Some(StyleInfo {
                        opacity: 1.0,
                        rotation: 0.0,
                        scale_x: 1.0,
                        scale_y: 1.0,
                    }),
                    diagnostics,
                });
                z_index += 1;
            }
            _ => {
                // Skip struct/grid blocks.
            }
        }
    }

    Ok(objects)
}

/// Phase 25E: light classification used by tests and the frontend to render
/// the right disabled-action message. Returned diagnostics list is purely
/// informational and never claims point-editing is possible.
pub fn classify_path_object(width: f32, height: f32) -> (&'static str, Vec<String>) {
    let mut diag = Vec::new();
    let hint = if width < 1.0 || height < 1.0 {
        "line_or_thin_path"
    } else if (width / height - 1.0).abs() < 0.05 {
        "square_path"
    } else {
        "rectangle_path"
    };
    diag.push(format!("Detected vector path (shape hint: {hint})."));
    diag.push(
        "Vector point editing is not implemented in this phase — read-only or safe visual removal only.".to_string(),
    );
    if width < 0.5 && height < 0.5 {
        diag.push("Degenerate path bbox; may be noise.".to_string());
    }
    (hint, diag)
}

/// Classify text editability.
fn classify_text_editability(text: &str, _font_size: f32) -> (EditableLevel, Vec<String>) {
    let mut diagnostics = Vec::new();
    let has_non_ascii = text.chars().any(|c| !c.is_ascii());

    if has_non_ascii {
        diagnostics.push("Contains non-ASCII characters. Native editing may require font verification.".to_string());
        // Still allow editing for common extended Latin, but flag CJK as visual-patch-only.
        let has_cjk = text.chars().any(|c| {
            let cp = c as u32;
            (0x4E00..=0x9FFF).contains(&cp) || (0x3040..=0x30FF).contains(&cp) || (0xAC00..=0xD7AF).contains(&cp)
        });
        if has_cjk {
            diagnostics.push("CJK text detected. Native editing not safe without font embedding verification.".to_string());
            return (EditableLevel::VisualPatchOnly, diagnostics);
        }
    }

    (EditableLevel::NativeEditable, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_ascii_as_editable() {
        let (level, diag) = classify_text_editability("Hello World", 12.0);
        assert_eq!(level, EditableLevel::NativeEditable);
        assert!(diag.is_empty());
    }

    #[test]
    fn classify_cjk_as_visual_patch() {
        let (level, _) = classify_text_editability("日本語テスト", 10.0);
        assert_eq!(level, EditableLevel::VisualPatchOnly);
    }

    #[test]
    fn classify_extended_latin_as_editable() {
        let (level, diag) = classify_text_editability("café résumé", 12.0);
        assert_eq!(level, EditableLevel::NativeEditable);
        assert!(!diag.is_empty()); // Has non-ASCII warning
    }

    // -------- Phase 25E: path classification --------

    #[test]
    fn classify_thin_path_as_line() {
        let (hint, diag) = classify_path_object(100.0, 0.2);
        assert_eq!(hint, "line_or_thin_path");
        assert!(diag.iter().any(|d| d.contains("Vector point editing is not implemented")),
            "expected honest editability disclaimer, got {:?}", diag);
    }

    #[test]
    fn classify_square_path() {
        let (hint, _) = classify_path_object(40.0, 40.0);
        assert_eq!(hint, "square_path");
    }

    #[test]
    fn classify_rectangle_path() {
        let (hint, _) = classify_path_object(200.0, 50.0);
        assert_eq!(hint, "rectangle_path");
    }

    // -------- Decoding-quality assessment (Acrobat-UX fix) --------

    #[test]
    fn decoding_quality_ok_for_clean_ascii() {
        assert_eq!(assess_decoding_quality("Hello World"), DecodingQuality::Ok);
    }

    #[test]
    fn decoding_quality_ok_for_clean_extended_latin() {
        assert_eq!(assess_decoding_quality("café résumé"), DecodingQuality::Ok);
    }

    #[test]
    fn decoding_quality_garbled_for_replacement_chars() {
        // String of 12 U+FFFD characters — entirely garbage.
        let s: String = std::iter::repeat('\u{FFFD}').take(12).collect();
        assert_eq!(assess_decoding_quality(&s), DecodingQuality::Garbled);
    }

    #[test]
    fn decoding_quality_garbled_for_pua_dump() {
        // Private Use Area is what shows up when MuPDF maps an unmapped
        // glyph index 1:1 — looks like a real character but isn't.
        let s: String = std::iter::repeat('\u{E000}').take(10).collect();
        assert_eq!(assess_decoding_quality(&s), DecodingQuality::Garbled);
    }

    #[test]
    fn decoding_quality_partial_for_one_bad_char_in_long_string() {
        // Long enough that 1 bad char is below the 20% threshold.
        let s = format!("Hello world from the encoder \u{FFFD}");
        assert_eq!(assess_decoding_quality(&s), DecodingQuality::Partial);
    }

    #[test]
    fn decoding_quality_empty_string_is_ok() {
        assert_eq!(assess_decoding_quality(""), DecodingQuality::Ok);
    }

    #[test]
    fn classify_path_flags_degenerate_bbox() {
        let (_, diag) = classify_path_object(0.1, 0.1);
        assert!(diag.iter().any(|d| d.contains("Degenerate")));
    }
}
