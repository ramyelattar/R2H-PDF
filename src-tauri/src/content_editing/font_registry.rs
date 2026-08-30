//! Phase 29A — page-level font registry.
//!
//! Inspects `/Resources /Font` on a PDF page and produces a list of
//! `FontResourceInfo` records describing each font's subtype, encoding,
//! and editability characteristics. Pure data — no editing here — but
//! the registry is the foundation for the native-edit decision engine
//! in `text_edit::classify_text_edit_strategy`.
//!
//! The implementation is conservative: when we cannot prove a font is
//! safe for a given edit, we mark it unsafe. We never silently lie.

use serde::{Deserialize, Serialize};

use crate::document_core::DocumentCoreState;

/// Kind of font encoding detected on a resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingKind {
    WinAnsi,
    MacRoman,
    IdentityH,
    IdentityV,
    /// Simple base encoding + non-empty Differences array.
    CustomDifferences,
    /// Encoding could only be inferred via a ToUnicode CMap.
    ToUnicode,
    Unknown,
}

impl EncodingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EncodingKind::WinAnsi => "win_ansi",
            EncodingKind::MacRoman => "mac_roman",
            EncodingKind::IdentityH => "identity_h",
            EncodingKind::IdentityV => "identity_v",
            EncodingKind::CustomDifferences => "custom_differences",
            EncodingKind::ToUnicode => "to_unicode",
            EncodingKind::Unknown => "unknown",
        }
    }
}

/// Per-font diagnostic record. Booleans default to `false` and any flag
/// flipped to `true` must be backed by something the parser actually
/// observed — no guessing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontResourceInfo {
    pub resource_name: String,
    pub base_font_name: String,
    pub subtype: String,
    pub encoding_kind: EncodingKind,
    pub base_encoding: Option<String>,
    pub differences_count: usize,
    pub has_to_unicode: bool,
    pub is_subset: bool,
    pub is_embedded: bool,
    pub is_cid_font: bool,
    pub is_type0: bool,
    pub is_true_type: bool,
    pub is_type1: bool,
    pub is_type3: bool,
    pub can_native_edit_ascii: bool,
    pub can_native_edit_latin1: bool,
    pub can_native_edit_arabic: bool,
    pub can_native_edit_cjk: bool,
    pub unsupported_reasons: Vec<String>,
    /// Phase 31A — parsed `/Differences` entries (code → glyph name)
    /// when the font's encoding dict contains a non-empty `/Differences`
    /// array. Empty otherwise. The decision engine layers these on top
    /// of the base encoding via `encoding_map::build_with_differences`.
    #[serde(default)]
    pub differences: Vec<super::tounicode::DifferencesEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageFontRegistry {
    pub session_id: String,
    pub page_index: usize,
    pub fonts: Vec<FontResourceInfo>,
    pub warnings: Vec<String>,
}

/// Phase 29A — build a font registry for a single page.
///
/// The function loads the page via MuPDF, walks `/Resources /Font`,
/// resolves each indirect reference, and classifies every entry.
/// Pages with no font resources return an empty `fonts` list + a
/// warning so the caller knows the page is image- or annotation-only.
pub fn build_page_font_registry(
    doc_state: &DocumentCoreState,
    session_id: &str,
    page_index: usize,
) -> Result<PageFontRegistry, String> {
    let arc = doc_state.store.get_session_arc_pub(session_id)
        .map_err(|e| e.to_string())?;
    let session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;

    let pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let page_count = pdf.page_count().map_err(|e| format!("page_count: {e}"))?;
    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    if page_no >= page_count {
        return Err(format!(
            "Page index {page_index} out of range (total: {page_count})"
        ));
    }
    let page = pdf.load_page(page_no).map_err(|e| format!("load_page: {e}"))?;
    let pdf_page = mupdf::pdf::PdfPage::try_from(page).map_err(|e| format!("PdfPage: {e}"))?;
    let page_obj = pdf_page.object();

    let mut warnings: Vec<String> = Vec::new();
    let mut fonts: Vec<FontResourceInfo> = Vec::new();

    let resources = match page_obj.get_dict("Resources").map_err(|e| format!("Resources: {e}"))? {
        Some(r) => r.resolve().ok().flatten().unwrap_or(r),
        None => {
            warnings.push("Page has no /Resources dictionary; no fonts available.".to_string());
            return Ok(PageFontRegistry {
                session_id: session_id.to_string(),
                page_index,
                fonts,
                warnings,
            });
        }
    };
    let font_dict = match resources.get_dict("Font").map_err(|e| format!("Font: {e}"))? {
        Some(f) => f.resolve().ok().flatten().unwrap_or(f),
        None => {
            warnings.push("Page /Resources has no /Font dictionary.".to_string());
            return Ok(PageFontRegistry {
                session_id: session_id.to_string(),
                page_index,
                fonts,
                warnings,
            });
        }
    };

    let entries = font_dict.len().unwrap_or(0);
    for i in 0..entries as i32 {
        let key_obj = match font_dict.get_dict_key(i) {
            Ok(Some(k)) => k,
            _ => continue,
        };
        let val_obj = match font_dict.get_dict_val(i) {
            Ok(Some(v)) => v,
            _ => continue,
        };
        let resolved_val = val_obj.resolve().ok().flatten().unwrap_or(val_obj);
        let resource_name = key_obj.as_name()
            .ok()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_else(|| format!("font-{i}"));
        match classify_font_object(&resolved_val, &resource_name) {
            Ok(info) => fonts.push(info),
            Err(e) => warnings.push(format!("Font {resource_name}: {e}")),
        }
    }

    Ok(PageFontRegistry {
        session_id: session_id.to_string(),
        page_index,
        fonts,
        warnings,
    })
}

/// Classify a single font dict into a `FontResourceInfo`. Pure function
/// over the PdfObject so we can unit-test slices of the logic via
/// `classify_font_info_from_parts`.
fn classify_font_object(
    font_obj: &mupdf::pdf::PdfObject,
    resource_name: &str,
) -> Result<FontResourceInfo, String> {
    let subtype = name_of(font_obj, "Subtype").unwrap_or_else(|| "Unknown".to_string());
    let base_font = name_of(font_obj, "BaseFont").unwrap_or_else(|| "Unknown".to_string());

    // For Type0 fonts the meaningful encoding info often lives on the
    // descendant CIDFont, but the top-level /Encoding still tells us
    // Identity-H vs Identity-V.
    let encoding_field = font_obj.get_dict("Encoding").ok().flatten();
    let analysis = match encoding_field {
        Some(enc) => analyze_encoding(&enc),
        None => EncodingAnalysis {
            kind: EncodingKind::Unknown,
            base_encoding: None,
            differences_count: 0,
            differences: vec![],
        },
    };
    let EncodingAnalysis {
        kind: encoding_kind,
        base_encoding,
        differences_count,
        differences,
    } = analysis;

    let has_to_unicode = font_obj.get_dict("ToUnicode").ok().flatten().is_some();

    // Subset detection: BaseFont prefixed with 6 uppercase letters + '+'.
    let is_subset = base_font.len() > 7
        && base_font.as_bytes().get(6) == Some(&b'+')
        && base_font.as_bytes()[..6].iter().all(|b| (b'A'..=b'Z').contains(b));

    // Embedded detection: look for /FontDescriptor /FontFile* indirect.
    let is_embedded = font_obj
        .get_dict("FontDescriptor")
        .ok()
        .flatten()
        .map(|fd| {
            let resolved = fd.resolve().ok().flatten().unwrap_or(fd);
            resolved.get_dict("FontFile").ok().flatten().is_some()
                || resolved.get_dict("FontFile2").ok().flatten().is_some()
                || resolved.get_dict("FontFile3").ok().flatten().is_some()
        })
        .unwrap_or(false);

    let mut info = classify_font_info_from_parts(
        resource_name,
        &base_font,
        &subtype,
        encoding_kind,
        base_encoding,
        differences_count,
        has_to_unicode,
        is_subset,
        is_embedded,
    );
    info.differences = differences;
    Ok(info)
}

/// Pure logic — given the inspected parts, return a `FontResourceInfo`.
/// Exposed so unit tests can cover combinations without a live PDF.
pub fn classify_font_info_from_parts(
    resource_name: &str,
    base_font: &str,
    subtype: &str,
    encoding_kind: EncodingKind,
    base_encoding: Option<String>,
    differences_count: usize,
    has_to_unicode: bool,
    is_subset: bool,
    is_embedded: bool,
) -> FontResourceInfo {
    let is_type0 = subtype == "Type0";
    let is_cid_font = subtype == "CIDFontType0" || subtype == "CIDFontType2";
    let is_true_type = subtype == "TrueType";
    let is_type1 = subtype == "Type1" || subtype == "MMType1";
    let is_type3 = subtype == "Type3";

    let mut unsupported_reasons: Vec<String> = Vec::new();
    if is_type3 {
        unsupported_reasons.push(
            "Type3 font detected — native text edit not supported in this build.".to_string(),
        );
    }
    if is_type0 && encoding_kind == EncodingKind::IdentityH {
        unsupported_reasons.push(
            "Type0/Identity-H font — native edit requires CID reverse mapping which is not implemented.".to_string(),
        );
    }
    if is_subset {
        unsupported_reasons.push(
            "Subset font detected — replacement glyphs may not be embedded; native edit may corrupt output.".to_string(),
        );
    }
    if matches!(encoding_kind, EncodingKind::CustomDifferences) {
        unsupported_reasons.push(
            "Custom encoding (Differences) detected — native edit limited to glyph names present in the table.".to_string(),
        );
    }
    if matches!(encoding_kind, EncodingKind::ToUnicode) {
        unsupported_reasons.push(
            "Only ToUnicode reverse mapping available — native edit not implemented for this font.".to_string(),
        );
    }
    if matches!(encoding_kind, EncodingKind::Unknown) && !is_type0 {
        unsupported_reasons.push(
            "Encoding unknown — native edit will refuse non-ASCII replacements.".to_string(),
        );
    }

    // Eligibility rules:
    //  * ASCII safe when encoding is WinAnsi / MacRoman / simple
    //    Differences-on-top-of-WinAnsi-or-MacRoman AND not a subset font
    //    AND not Type3.
    //  * Latin-1 safe when encoding is WinAnsi or MacRoman (their tables
    //    cover U+0000–U+00FF) AND not subset / Type3.
    //  * Arabic / CJK never native-safe in this phase.
    let simple_8bit = matches!(
        encoding_kind,
        EncodingKind::WinAnsi | EncodingKind::MacRoman
    );
    let can_native_edit_ascii =
        !is_type3 && !is_subset && !is_type0 && (simple_8bit || matches!(encoding_kind, EncodingKind::CustomDifferences));
    let can_native_edit_latin1 = !is_type3 && !is_subset && !is_type0 && simple_8bit;
    let can_native_edit_arabic = false;
    let can_native_edit_cjk = false;

    FontResourceInfo {
        resource_name: resource_name.to_string(),
        base_font_name: base_font.to_string(),
        subtype: subtype.to_string(),
        encoding_kind,
        base_encoding,
        differences_count,
        has_to_unicode,
        is_subset,
        is_embedded,
        is_cid_font,
        is_type0,
        is_true_type,
        is_type1,
        is_type3,
        can_native_edit_ascii,
        can_native_edit_latin1,
        can_native_edit_arabic,
        can_native_edit_cjk,
        unsupported_reasons,
        differences: vec![], // populated by `classify_font_object`.
    }
}

/// Phase 31A — return value of `analyze_encoding`. Carries the parsed
/// Differences entries alongside the classification.
struct EncodingAnalysis {
    kind: EncodingKind,
    base_encoding: Option<String>,
    differences_count: usize,
    differences: Vec<super::tounicode::DifferencesEntry>,
}

/// Inspect `/Encoding` (either a name like `/WinAnsiEncoding` or a dict
/// with `/BaseEncoding` + `/Differences`) and classify it.
fn analyze_encoding(enc: &mupdf::pdf::PdfObject) -> EncodingAnalysis {
    let resolved = enc.resolve().ok().flatten().unwrap_or(enc.clone());
    // Case 1: name form (e.g. /WinAnsiEncoding, /Identity-H).
    if resolved.is_name().unwrap_or(false) {
        if let Ok(bytes) = resolved.as_name() {
            let name = String::from_utf8_lossy(bytes).into_owned();
            let kind = classify_encoding_name(&name);
            return EncodingAnalysis {
                kind,
                base_encoding: Some(name),
                differences_count: 0,
                differences: vec![],
            };
        }
    }
    // Case 2: dict form.
    let base_encoding = resolved
        .get_dict("BaseEncoding")
        .ok()
        .flatten()
        .and_then(|b| b.as_name().ok().map(|bytes| String::from_utf8_lossy(bytes).into_owned()));
    let (differences_count, differences) = match resolved.get_dict("Differences").ok().flatten() {
        Some(arr) => {
            let arr_resolved = arr.resolve().ok().flatten().unwrap_or(arr);
            let len = arr_resolved.len().unwrap_or(0);
            // Read tokens as either int or /name and feed parse_differences_tokens.
            let mut tokens: Vec<String> = Vec::with_capacity(len);
            for i in 0..len as i32 {
                let Ok(Some(entry)) = arr_resolved.get_array(i) else { continue };
                let entry_r = entry.resolve().ok().flatten().unwrap_or(entry);
                // Either an integer code or a name like /eacute.
                if entry_r.is_name().unwrap_or(false) {
                    if let Ok(bytes) = entry_r.as_name() {
                        tokens.push(format!("/{}", String::from_utf8_lossy(bytes)));
                    }
                } else {
                    // Try integer.
                    if let Ok(n) = entry_r.as_int() {
                        tokens.push(n.to_string());
                    }
                }
            }
            let parsed = super::tounicode::parse_differences_tokens(&tokens);
            (parsed.len(), parsed)
        }
        None => (0, vec![]),
    };
    let kind = if differences_count > 0 {
        EncodingKind::CustomDifferences
    } else if let Some(name) = base_encoding.as_deref() {
        classify_encoding_name(name)
    } else {
        EncodingKind::Unknown
    };
    EncodingAnalysis {
        kind,
        base_encoding,
        differences_count,
        differences,
    }
}

/// Pure helper — classify an encoding name string.
pub fn classify_encoding_name(name: &str) -> EncodingKind {
    match name {
        "WinAnsiEncoding" | "/WinAnsiEncoding" => EncodingKind::WinAnsi,
        "MacRomanEncoding" | "/MacRomanEncoding" => EncodingKind::MacRoman,
        "Identity-H" | "/Identity-H" => EncodingKind::IdentityH,
        "Identity-V" | "/Identity-V" => EncodingKind::IdentityV,
        _ => EncodingKind::Unknown,
    }
}

fn name_of(font_obj: &mupdf::pdf::PdfObject, key: &str) -> Option<String> {
    let val = font_obj.get_dict(key).ok().flatten()?;
    let resolved = val.resolve().ok().flatten().unwrap_or(val);
    resolved
        .as_name()
        .ok()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type1_winansi_is_native_safe_for_ascii_and_latin1() {
        let info = classify_font_info_from_parts(
            "F1", "Helvetica", "Type1", EncodingKind::WinAnsi,
            Some("WinAnsiEncoding".to_string()), 0,
            false, false, false,
        );
        assert!(info.is_type1);
        assert!(info.can_native_edit_ascii);
        assert!(info.can_native_edit_latin1);
        assert!(!info.can_native_edit_cjk);
        assert!(!info.can_native_edit_arabic);
        assert!(info.unsupported_reasons.is_empty());
    }

    #[test]
    fn truetype_winansi_is_native_safe() {
        let info = classify_font_info_from_parts(
            "F2", "Arial", "TrueType", EncodingKind::WinAnsi,
            Some("WinAnsiEncoding".to_string()), 0,
            false, false, true,
        );
        assert!(info.is_true_type);
        assert!(info.can_native_edit_ascii);
        assert!(info.can_native_edit_latin1);
    }

    #[test]
    fn subset_prefix_is_detected_and_marked_unsafe() {
        let info = classify_font_info_from_parts(
            "F3", "ABCDEF+Helvetica", "Type1", EncodingKind::WinAnsi,
            Some("WinAnsiEncoding".to_string()), 0,
            false, true, true,
        );
        assert!(info.is_subset);
        assert!(!info.can_native_edit_ascii);
        assert!(info.unsupported_reasons.iter().any(|r| r.to_lowercase().contains("subset")));
    }

    #[test]
    fn to_unicode_presence_is_recorded() {
        let info = classify_font_info_from_parts(
            "F4", "Arial", "TrueType", EncodingKind::WinAnsi,
            Some("WinAnsiEncoding".to_string()), 0,
            true, false, true,
        );
        assert!(info.has_to_unicode);
    }

    #[test]
    fn identity_h_type0_is_marked_unsafe() {
        let info = classify_font_info_from_parts(
            "F5", "STHeiti", "Type0", EncodingKind::IdentityH,
            Some("Identity-H".to_string()), 0,
            true, true, true,
        );
        assert!(info.is_type0);
        assert!(!info.can_native_edit_ascii);
        assert!(info.unsupported_reasons.iter().any(|r| r.to_lowercase().contains("identity-h")));
    }

    #[test]
    fn differences_array_is_counted() {
        let info = classify_font_info_from_parts(
            "F6", "Custom", "Type1", EncodingKind::CustomDifferences,
            Some("WinAnsiEncoding".to_string()), 5,
            false, false, true,
        );
        assert_eq!(info.differences_count, 5);
        assert_eq!(info.encoding_kind, EncodingKind::CustomDifferences);
        assert!(info.unsupported_reasons.iter().any(|r| r.to_lowercase().contains("differences")));
    }

    #[test]
    fn type3_font_is_unsafe() {
        let info = classify_font_info_from_parts(
            "F7", "MyType3", "Type3", EncodingKind::Unknown,
            None, 0,
            false, false, true,
        );
        assert!(info.is_type3);
        assert!(!info.can_native_edit_ascii);
        assert!(info.unsupported_reasons.iter().any(|r| r.to_lowercase().contains("type3")));
    }

    #[test]
    fn classify_encoding_name_recognizes_standards() {
        assert_eq!(classify_encoding_name("WinAnsiEncoding"), EncodingKind::WinAnsi);
        assert_eq!(classify_encoding_name("MacRomanEncoding"), EncodingKind::MacRoman);
        assert_eq!(classify_encoding_name("Identity-H"), EncodingKind::IdentityH);
        assert_eq!(classify_encoding_name("Identity-V"), EncodingKind::IdentityV);
        assert_eq!(classify_encoding_name("SomeRandom"), EncodingKind::Unknown);
    }

    #[test]
    fn unknown_encoding_is_marked_unsafe_for_non_type0_font() {
        let info = classify_font_info_from_parts(
            "F8", "Custom", "Type1", EncodingKind::Unknown,
            None, 0,
            false, false, true,
        );
        assert!(!info.can_native_edit_ascii);
        assert!(info.unsupported_reasons.iter().any(|r| r.to_lowercase().contains("encoding unknown")));
    }
}
