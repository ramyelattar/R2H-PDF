//! Form-field creation, deletion, and property updates.
//!
//! Phase 23A: extends the AcroForm support beyond read-only listing to
//! true creation of new widget annotations and AcroForm field entries.
//!
//! Strategy:
//! * Build a merged widget/field PdfObject from a dictionary string.
//! * `add_object` it to get an indirect reference.
//! * Append the indirect to `/Catalog/AcroForm/Fields` (creating
//!   `/AcroForm` if absent).
//! * Append the indirect to the page's `/Annots` array.
//! * Set `/AcroForm/NeedAppearances true` so any conforming viewer
//!   regenerates appearance streams on open. This avoids the complexity of
//!   hand-rolling /AP streams while still producing a valid, editable field.

use serde::{Deserialize, Serialize};

use mupdf::pdf::PdfDocument;

use super::engine::OpenedDocument;
use super::errors::DocumentCoreError;
use super::types::FormField;

/// Field type for creation. Mirrors the subset of PDF /FT we expose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateFieldType {
    Text,
    Checkbox,
    Radio,
    Combo,
    List,
    Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFormFieldRequest {
    pub session_id: String,
    pub page_index: usize,
    pub field_type: CreateFieldType,
    pub name: String,
    pub value: Option<String>,
    pub default_value: Option<String>,
    pub options: Option<Vec<String>>,
    pub rect: [f32; 4],
    pub required: Option<bool>,
    pub read_only: Option<bool>,
    pub font_size: Option<f32>,
    pub border_color: Option<[f32; 3]>,
    pub fill_color: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFormFieldRequest {
    pub session_id: String,
    pub field_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFormFieldPropertiesRequest {
    pub session_id: String,
    pub field_name: String,
    pub value: Option<String>,
    pub required: Option<bool>,
    pub read_only: Option<bool>,
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFieldOperationResult {
    pub success: bool,
    pub field_name: String,
    pub page_index: usize,
    pub action: String,
    pub warnings: Vec<String>,
}

// -------- Validation helpers --------

const MAX_NAME_LEN: usize = 127;

/// Validate a field name against a conservative subset of PDF naming rules.
/// PDF technically allows almost anything but we restrict to printable
/// ASCII without `.` `/` `(` `)` `<` `>` `[` `]` `%` `#` to keep names
/// safe for AcroForm fully-qualified paths.
fn validate_field_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("field name cannot be empty".to_string());
    }
    if name.len() > MAX_NAME_LEN {
        return Err(format!(
            "field name length {} exceeds max {}",
            name.len(),
            MAX_NAME_LEN
        ));
    }
    for ch in name.chars() {
        if !ch.is_ascii() || ch.is_ascii_control() {
            return Err(format!(
                "field name contains non-ASCII or control character: {:?}",
                ch
            ));
        }
        if matches!(
            ch,
            '.' | '/' | '(' | ')' | '<' | '>' | '[' | ']' | '%' | '#'
        ) {
            return Err(format!(
                "field name contains reserved PDF character: {:?}",
                ch
            ));
        }
    }
    Ok(())
}

/// Validate the rect is non-degenerate.
fn validate_rect(rect: &[f32; 4]) -> Result<(), String> {
    let [x0, y0, x1, y1] = *rect;
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return Err(format!("rect has non-finite values: {rect:?}"));
    }
    if x1 <= x0 || y1 <= y0 {
        return Err(format!("rect must satisfy x1>x0 and y1>y0; got {rect:?}"));
    }
    Ok(())
}

/// Escape `(` `)` `\` in a PDF literal string.
fn escape_pdf_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Disambiguate `name` by appending `_2`, `_3`, ... until it doesn't appear
/// in `existing`. Returns the (possibly modified) name.
fn unique_name(name: &str, existing: &[FormField]) -> String {
    if !existing.iter().any(|f| f.name == name) {
        return name.to_string();
    }
    for suffix in 2..1000u32 {
        let candidate = format!("{name}_{suffix}");
        if !existing.iter().any(|f| f.name == candidate) {
            return candidate;
        }
    }
    // Extremely unlikely fallthrough.
    format!("{name}_{}", chrono_like_nanos())
}

fn chrono_like_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

// -------- Public entry points --------

/// Create a new AcroForm field + widget annotation on the requested page.
///
/// Returns the (possibly disambiguated) field name on success along with the
/// updated form-field listing.
pub fn create_form_field(
    doc: &mut OpenedDocument,
    request: &CreateFormFieldRequest,
    existing: &[FormField],
) -> Result<(FormFieldOperationResult, String), DocumentCoreError> {
    validate_field_name(&request.name).map_err(DocumentCoreError::InvalidPdf)?;
    validate_rect(&request.rect).map_err(DocumentCoreError::InvalidPdf)?;

    let page_count = doc.pages.len();
    if request.page_index >= page_count {
        return Err(DocumentCoreError::PageOutOfRange {
            requested: request.page_index,
            total: page_count,
        });
    }

    // Disambiguate.
    let mut warnings = Vec::new();
    let final_name = unique_name(&request.name, existing);
    if final_name != request.name {
        warnings.push(format!(
            "duplicate field name '{}' was auto-suffixed to '{}'",
            request.name, final_name
        ));
    }

    let mut pdf_doc = PdfDocument::from_bytes(&doc.bytes)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("failed to open PDF: {e}")))?;

    let dict_str = build_field_dict_str(request, &final_name);

    let field_obj = pdf_doc.new_object_from_str(&dict_str).map_err(|e| {
        DocumentCoreError::InvalidPdf(format!("invalid field dict: {e}; dict={dict_str}"))
    })?;

    let field_indirect = pdf_doc
        .add_object(&field_obj)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("add_object failed: {e}")))?;

    // Append to AcroForm /Fields.
    append_to_acroform_fields(&mut pdf_doc, &field_indirect)?;

    // Append to page /Annots.
    append_to_page_annots(&mut pdf_doc, request.page_index, &field_indirect)?;

    serialize_back(&mut pdf_doc, doc)?;

    Ok((
        FormFieldOperationResult {
            success: true,
            field_name: final_name.clone(),
            page_index: request.page_index,
            action: "create".to_string(),
            warnings,
        },
        final_name,
    ))
}

/// Delete a field by name. Removes the field from `/AcroForm/Fields` and
/// from every page's `/Annots` array.
pub fn delete_form_field(
    doc: &mut OpenedDocument,
    request: &DeleteFormFieldRequest,
) -> Result<FormFieldOperationResult, DocumentCoreError> {
    if request.field_name.is_empty() {
        return Err(DocumentCoreError::InvalidPdf(
            "field name cannot be empty".to_string(),
        ));
    }

    let mut pdf_doc = PdfDocument::from_bytes(&doc.bytes)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("failed to open PDF: {e}")))?;

    let trailer = pdf_doc.trailer().map_err(map_err)?;
    let root = trailer
        .get_dict("Root")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no /Root in trailer".to_string()))?;

    let acro_form = root.get_dict("AcroForm").map_err(map_err)?;
    let Some(acro_form) = acro_form else {
        return Err(DocumentCoreError::InvalidPdf(
            "no AcroForm in document".to_string(),
        ));
    };

    let mut fields_arr = acro_form
        .get_dict("Fields")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no /Fields array".to_string()))?;

    let mut removed_indirect: Option<i32> = None;
    let len = fields_arr.len().unwrap_or(0);
    let mut idx_to_remove: Option<i32> = None;
    for i in 0..len {
        let elem = match fields_arr.get_array(i as i32).map_err(map_err)? {
            Some(e) => e,
            None => continue,
        };
        let resolved = elem
            .resolve()
            .map_err(map_err)?
            .unwrap_or(elem.try_clone().map_err(map_err)?);
        let name = resolved
            .get_dict("T")
            .map_err(map_err)?
            .and_then(|t| t.as_string().ok().map(|s| s.to_string()));
        if name.as_deref() == Some(request.field_name.as_str()) {
            if let Ok(num) = elem.as_indirect() {
                removed_indirect = Some(num);
            }
            idx_to_remove = Some(i as i32);
            break;
        }
    }

    let Some(idx) = idx_to_remove else {
        return Err(DocumentCoreError::InvalidPdf(format!(
            "field '{}' not found",
            request.field_name
        )));
    };

    fields_arr.array_delete(idx).map_err(map_err)?;

    // Strip from any page /Annots whose entry resolves to the same indirect.
    let mut removed_from_pages: usize = 0;
    if let Some(num) = removed_indirect {
        let page_count = doc.pages.len();
        for page_idx in 0..page_count {
            if let Ok(page_obj) = pdf_doc.find_page(page_idx as i32) {
                let resolved = page_obj.resolve().map_err(map_err)?;
                let page_dict = resolved.unwrap_or(page_obj);
                if let Some(mut annots) = page_dict.get_dict("Annots").map_err(map_err)? {
                    let annots_len = annots.len().unwrap_or(0);
                    // Iterate in reverse so deletions don't shift remaining indices.
                    for j in (0..annots_len).rev() {
                        let elem = match annots.get_array(j as i32).map_err(map_err)? {
                            Some(e) => e,
                            None => continue,
                        };
                        if elem.as_indirect().ok() == Some(num) {
                            annots.array_delete(j as i32).map_err(map_err)?;
                            removed_from_pages += 1;
                        }
                    }
                }
            }
        }
    }

    if let Some(num) = removed_indirect {
        // Best-effort: delete the indirect object itself.
        let _ = pdf_doc.delete_object(num);
    }

    serialize_back(&mut pdf_doc, doc)?;

    let mut warnings = Vec::new();
    if removed_from_pages == 0 {
        warnings.push("field removed from AcroForm but no widget found on any page".to_string());
    }

    Ok(FormFieldOperationResult {
        success: true,
        field_name: request.field_name.clone(),
        page_index: 0,
        action: "delete".to_string(),
        warnings,
    })
}

/// Update mutable properties of an existing field by name.
pub fn update_form_field_properties(
    doc: &mut OpenedDocument,
    request: &UpdateFormFieldPropertiesRequest,
) -> Result<FormFieldOperationResult, DocumentCoreError> {
    if request.field_name.is_empty() {
        return Err(DocumentCoreError::InvalidPdf(
            "field name cannot be empty".to_string(),
        ));
    }

    let mut pdf_doc = PdfDocument::from_bytes(&doc.bytes)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("failed to open PDF: {e}")))?;

    let trailer = pdf_doc.trailer().map_err(map_err)?;
    let root = trailer
        .get_dict("Root")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no /Root in trailer".to_string()))?;
    let acro_form = root
        .get_dict("AcroForm")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no AcroForm".to_string()))?;
    let fields_arr = acro_form
        .get_dict("Fields")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no /Fields".to_string()))?;

    let len = fields_arr.len().unwrap_or(0);
    let mut target: Option<mupdf::pdf::PdfObject> = None;
    for i in 0..len {
        let elem = match fields_arr.get_array(i as i32).map_err(map_err)? {
            Some(e) => e,
            None => continue,
        };
        let resolved = elem.resolve().map_err(map_err)?.unwrap_or(elem);
        let name = resolved
            .get_dict("T")
            .map_err(map_err)?
            .and_then(|t| t.as_string().ok().map(|s| s.to_string()));
        if name.as_deref() == Some(request.field_name.as_str()) {
            target = Some(resolved);
            break;
        }
    }

    let mut field = target.ok_or_else(|| {
        DocumentCoreError::InvalidPdf(format!("field '{}' not found", request.field_name))
    })?;

    // Value.
    if let Some(v) = &request.value {
        let v_obj = mupdf::pdf::PdfObject::new_string(v).map_err(map_err)?;
        field.dict_put("V", v_obj).map_err(map_err)?;
    }

    // Flags (/Ff). Read-only = bit 1 (value 1); Required = bit 2 (value 2).
    if request.required.is_some() || request.read_only.is_some() {
        let mut ff: i32 = field
            .get_dict_inheritable("Ff")
            .map_err(map_err)?
            .and_then(|f| f.as_int().ok())
            .unwrap_or(0);
        if let Some(ro) = request.read_only {
            if ro {
                ff |= 1;
            } else {
                ff &= !1;
            }
        }
        if let Some(req) = request.required {
            if req {
                ff |= 2;
            } else {
                ff &= !2;
            }
        }
        let ff_obj = mupdf::pdf::PdfObject::new_int(ff).map_err(map_err)?;
        field.dict_put("Ff", ff_obj).map_err(map_err)?;
    }

    if let Some(fs) = request.font_size {
        let fs_clamped = fs.clamp(1.0, 72.0);
        let da_obj = mupdf::pdf::PdfObject::new_string(&format!("/Helv {fs_clamped} Tf 0 0 0 rg"))
            .map_err(map_err)?;
        field.dict_put("DA", da_obj).map_err(map_err)?;
    }

    // Force appearance regen.
    ensure_need_appearances(&mut pdf_doc)?;

    serialize_back(&mut pdf_doc, doc)?;

    Ok(FormFieldOperationResult {
        success: true,
        field_name: request.field_name.clone(),
        page_index: 0,
        action: "update_properties".to_string(),
        warnings: Vec::new(),
    })
}

// -------- Internal helpers --------

fn map_err(e: mupdf::Error) -> DocumentCoreError {
    DocumentCoreError::InvalidPdf(e.to_string())
}

/// Build the dictionary literal that represents the merged widget/field
/// for the requested field type.
fn build_field_dict_str(req: &CreateFormFieldRequest, name: &str) -> String {
    let [x0, y0, x1, y1] = req.rect;
    let font_size = req.font_size.unwrap_or(12.0).clamp(1.0, 72.0);
    let da = format!("/Helv {font_size} Tf 0 0 0 rg");
    let name_escaped = escape_pdf_string(name);

    // Field flags: read-only bit 1, required bit 2.
    let mut ff = 0;
    if req.read_only.unwrap_or(false) {
        ff |= 1;
    }
    if req.required.unwrap_or(false) {
        ff |= 2;
    }

    // Border + fill color appearance characteristics — populated when provided.
    let mk = build_mk_dict(req.border_color, req.fill_color);

    match req.field_type {
        CreateFieldType::Text => {
            let value = req.value.as_deref().unwrap_or("");
            let default = req.default_value.as_deref().unwrap_or("");
            let v_esc = escape_pdf_string(value);
            let dv_esc = escape_pdf_string(default);
            format!(
                "<</Type /Annot /Subtype /Widget /FT /Tx /T ({name}) \
                 /V ({v}) /DV ({dv}) \
                 /Rect [{x0} {y0} {x1} {y1}] \
                 /Ff {ff} /F 4 /DA ({da}) /Q 0 {mk}>>",
                name = name_escaped,
                v = v_esc,
                dv = dv_esc,
                x0 = x0,
                y0 = y0,
                x1 = x1,
                y1 = y1,
                ff = ff,
                da = da,
                mk = mk,
            )
        }
        CreateFieldType::Checkbox => {
            // Checkboxes use /Btn with /Ff bit 16 NOT set.
            let initial = req.value.as_deref().unwrap_or("Off");
            // Map common truthy values to /Yes.
            let v_name = if matches!(initial, "Yes" | "On" | "true" | "1" | "checked") {
                "Yes"
            } else {
                "Off"
            };
            format!(
                "<</Type /Annot /Subtype /Widget /FT /Btn /T ({name}) \
                 /V /{v_name} /AS /{v_name} /DV /Off \
                 /Rect [{x0} {y0} {x1} {y1}] \
                 /Ff {ff} /F 4 {mk}>>",
                name = name_escaped,
                v_name = v_name,
                x0 = x0,
                y0 = y0,
                x1 = x1,
                y1 = y1,
                ff = ff,
                mk = mk,
            )
        }
        CreateFieldType::Radio => {
            // Radio = /Btn with /Ff bit 16 set (15536 = 0x8000 + read-only/required as needed).
            let radio_ff = ff | (1 << 15);
            format!(
                "<</Type /Annot /Subtype /Widget /FT /Btn /T ({name}) \
                 /V /Off /AS /Off /DV /Off \
                 /Rect [{x0} {y0} {x1} {y1}] \
                 /Ff {ff} /F 4 {mk}>>",
                name = name_escaped,
                ff = radio_ff,
                x0 = x0,
                y0 = y0,
                x1 = x1,
                y1 = y1,
                mk = mk,
            )
        }
        CreateFieldType::Combo | CreateFieldType::List => {
            let combo = matches!(req.field_type, CreateFieldType::Combo);
            // Combo = /Ch with /Ff bit 18 set. List = /Ch without that bit.
            let ch_ff = if combo { ff | (1 << 17) } else { ff };
            let opt_arr = match &req.options {
                Some(opts) if !opts.is_empty() => {
                    let inner: Vec<String> = opts
                        .iter()
                        .map(|o| format!("({})", escape_pdf_string(o)))
                        .collect();
                    format!("/Opt [{}]", inner.join(" "))
                }
                _ => String::new(),
            };
            let value = req.value.as_deref().unwrap_or("");
            let v_esc = escape_pdf_string(value);
            format!(
                "<</Type /Annot /Subtype /Widget /FT /Ch /T ({name}) \
                 /V ({v}) {opt} \
                 /Rect [{x0} {y0} {x1} {y1}] \
                 /Ff {ff} /F 4 /DA ({da}) {mk}>>",
                name = name_escaped,
                v = v_esc,
                opt = opt_arr,
                x0 = x0,
                y0 = y0,
                x1 = x1,
                y1 = y1,
                ff = ch_ff,
                da = da,
                mk = mk,
            )
        }
        CreateFieldType::Signature => {
            format!(
                "<</Type /Annot /Subtype /Widget /FT /Sig /T ({name}) \
                 /Rect [{x0} {y0} {x1} {y1}] \
                 /Ff {ff} /F 4 {mk}>>",
                name = name_escaped,
                x0 = x0,
                y0 = y0,
                x1 = x1,
                y1 = y1,
                ff = ff,
                mk = mk,
            )
        }
    }
}

fn build_mk_dict(border: Option<[f32; 3]>, fill: Option<[f32; 3]>) -> String {
    if border.is_none() && fill.is_none() {
        return String::new();
    }
    let mut parts = Vec::new();
    if let Some([r, g, b]) = border {
        parts.push(format!("/BC [{r} {g} {b}]"));
    }
    if let Some([r, g, b]) = fill {
        parts.push(format!("/BG [{r} {g} {b}]"));
    }
    format!("/MK <<{}>>", parts.join(" "))
}

fn append_to_acroform_fields(
    pdf_doc: &mut PdfDocument,
    field_indirect: &mupdf::pdf::PdfObject,
) -> Result<(), DocumentCoreError> {
    let trailer = pdf_doc.trailer().map_err(map_err)?;
    let root = trailer
        .get_dict("Root")
        .map_err(map_err)?
        .ok_or_else(|| DocumentCoreError::InvalidPdf("no /Root in trailer".to_string()))?;
    let resolved_root = root.resolve().map_err(map_err)?;
    let mut root_dict = resolved_root.unwrap_or(root);

    let existing_acroform = root_dict.get_dict("AcroForm").map_err(map_err)?;
    match existing_acroform {
        Some(acro) => {
            let resolved_af = acro.resolve().map_err(map_err)?;
            let mut acro_dict = resolved_af.unwrap_or(acro);
            // Ensure /Fields exists.
            let mut fields_arr = match acro_dict.get_dict("Fields").map_err(map_err)? {
                Some(f) => f,
                None => {
                    let new_arr = pdf_doc.new_array().map_err(map_err)?;
                    acro_dict
                        .dict_put("Fields", new_arr.try_clone().map_err(map_err)?)
                        .map_err(map_err)?;
                    new_arr
                }
            };
            fields_arr
                .array_push(field_indirect.try_clone().map_err(map_err)?)
                .map_err(map_err)?;
            // NeedAppearances so viewers regenerate /AP.
            let true_obj = mupdf::pdf::PdfObject::new_bool(true);
            acro_dict
                .dict_put("NeedAppearances", true_obj)
                .map_err(map_err)?;
        }
        None => {
            let mut new_acro = pdf_doc.new_dict().map_err(map_err)?;
            let mut new_fields = pdf_doc.new_array().map_err(map_err)?;
            new_fields
                .array_push(field_indirect.try_clone().map_err(map_err)?)
                .map_err(map_err)?;
            new_acro.dict_put("Fields", new_fields).map_err(map_err)?;
            let true_obj = mupdf::pdf::PdfObject::new_bool(true);
            new_acro
                .dict_put("NeedAppearances", true_obj)
                .map_err(map_err)?;
            // Store the AcroForm dict as an indirect object and reference it.
            let acro_indirect = pdf_doc.add_object(&new_acro).map_err(map_err)?;
            root_dict
                .dict_put("AcroForm", acro_indirect)
                .map_err(map_err)?;
        }
    }

    Ok(())
}

fn append_to_page_annots(
    pdf_doc: &mut PdfDocument,
    page_index: usize,
    annot_indirect: &mupdf::pdf::PdfObject,
) -> Result<(), DocumentCoreError> {
    let page_obj = pdf_doc
        .find_page(page_index as i32)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("find_page({page_index}): {e}")))?;
    let resolved = page_obj.resolve().map_err(map_err)?;
    let mut page_dict = resolved.unwrap_or(page_obj);

    let existing = page_dict.get_dict("Annots").map_err(map_err)?;
    match existing {
        Some(mut arr) => {
            arr.array_push(annot_indirect.try_clone().map_err(map_err)?)
                .map_err(map_err)?;
        }
        None => {
            let mut new_arr = pdf_doc.new_array().map_err(map_err)?;
            new_arr
                .array_push(annot_indirect.try_clone().map_err(map_err)?)
                .map_err(map_err)?;
            page_dict.dict_put("Annots", new_arr).map_err(map_err)?;
        }
    }
    Ok(())
}

fn ensure_need_appearances(pdf_doc: &mut PdfDocument) -> Result<(), DocumentCoreError> {
    let trailer = pdf_doc.trailer().map_err(map_err)?;
    let root = trailer.get_dict("Root").map_err(map_err)?;
    let Some(root) = root else {
        return Ok(());
    };
    let resolved = root.resolve().map_err(map_err)?;
    let root_dict = resolved.unwrap_or(root);
    if let Some(acro) = root_dict.get_dict("AcroForm").map_err(map_err)? {
        let resolved_af = acro.resolve().map_err(map_err)?;
        let mut acro_dict = resolved_af.unwrap_or(acro);
        let true_obj = mupdf::pdf::PdfObject::new_bool(true);
        acro_dict
            .dict_put("NeedAppearances", true_obj)
            .map_err(map_err)?;
    }
    Ok(())
}

fn serialize_back(
    pdf_doc: &mut PdfDocument,
    doc: &mut OpenedDocument,
) -> Result<(), DocumentCoreError> {
    let mut new_bytes: Vec<u8> = Vec::new();
    pdf_doc
        .write_to(&mut new_bytes)
        .map_err(|e| DocumentCoreError::InvalidPdf(format!("failed to serialize PDF: {e}")))?;
    doc.bytes = new_bytes;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_field_name_rejects_empty() {
        assert!(validate_field_name("").is_err());
    }

    #[test]
    fn validate_field_name_rejects_reserved_chars() {
        for c in ['.', '/', '(', ')', '<', '>', '[', ']', '%', '#'] {
            let s = format!("name{c}");
            assert!(
                validate_field_name(&s).is_err(),
                "expected {s:?} to be rejected"
            );
        }
    }

    #[test]
    fn validate_field_name_accepts_simple() {
        assert!(validate_field_name("name").is_ok());
        assert!(validate_field_name("Customer_Name").is_ok());
        assert!(validate_field_name("a-b_c").is_ok());
    }

    #[test]
    fn validate_field_name_rejects_too_long() {
        let s = "a".repeat(128);
        assert!(validate_field_name(&s).is_err());
    }

    #[test]
    fn validate_rect_rejects_degenerate() {
        assert!(validate_rect(&[10.0, 10.0, 10.0, 50.0]).is_err());
        assert!(validate_rect(&[10.0, 10.0, 50.0, 10.0]).is_err());
        assert!(validate_rect(&[50.0, 10.0, 10.0, 50.0]).is_err());
        assert!(validate_rect(&[10.0, 50.0, 50.0, 10.0]).is_err());
        assert!(validate_rect(&[10.0, 10.0, 50.0, 50.0]).is_ok());
    }

    #[test]
    fn validate_rect_rejects_non_finite() {
        assert!(validate_rect(&[f32::NAN, 0.0, 10.0, 10.0]).is_err());
        assert!(validate_rect(&[0.0, 0.0, f32::INFINITY, 10.0]).is_err());
    }

    #[test]
    fn unique_name_passthrough_when_no_collision() {
        let existing = vec![FormField {
            name: "other".to_string(),
            field_type: "text".to_string(),
            value: String::new(),
            page_index: 0,
            rect: [0.0; 4],
        }];
        assert_eq!(unique_name("Customer", &existing), "Customer");
    }

    #[test]
    fn unique_name_suffixes_when_collision() {
        let existing = vec![FormField {
            name: "Customer".to_string(),
            field_type: "text".to_string(),
            value: String::new(),
            page_index: 0,
            rect: [0.0; 4],
        }];
        assert_eq!(unique_name("Customer", &existing), "Customer_2");
    }

    #[test]
    fn unique_name_increments_until_free() {
        let existing = vec![
            FormField {
                name: "x".into(),
                field_type: "text".into(),
                value: String::new(),
                page_index: 0,
                rect: [0.0; 4],
            },
            FormField {
                name: "x_2".into(),
                field_type: "text".into(),
                value: String::new(),
                page_index: 0,
                rect: [0.0; 4],
            },
            FormField {
                name: "x_3".into(),
                field_type: "text".into(),
                value: String::new(),
                page_index: 0,
                rect: [0.0; 4],
            },
        ];
        assert_eq!(unique_name("x", &existing), "x_4");
    }

    #[test]
    fn build_text_field_dict_contains_required_keys() {
        let req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Text,
            name: "Foo".into(),
            value: Some("Bar".into()),
            default_value: None,
            options: None,
            rect: [10.0, 20.0, 100.0, 50.0],
            required: Some(true),
            read_only: None,
            font_size: Some(14.0),
            border_color: None,
            fill_color: None,
        };
        let s = build_field_dict_str(&req, "Foo");
        assert!(s.contains("/FT /Tx"), "missing /FT /Tx in {s}");
        assert!(s.contains("/T (Foo)"));
        assert!(s.contains("/V (Bar)"));
        assert!(s.contains("/Subtype /Widget"));
        assert!(s.contains("/Ff 2"), "expected Required bit only: {s}");
    }

    #[test]
    fn build_checkbox_field_handles_initial_value() {
        let mut req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Checkbox,
            name: "Agree".into(),
            value: Some("Yes".into()),
            default_value: None,
            options: None,
            rect: [10.0, 20.0, 30.0, 40.0],
            required: None,
            read_only: None,
            font_size: None,
            border_color: None,
            fill_color: None,
        };
        let s_yes = build_field_dict_str(&req, "Agree");
        assert!(s_yes.contains("/V /Yes"), "{s_yes}");
        assert!(s_yes.contains("/AS /Yes"), "{s_yes}");

        req.value = Some("Off".into());
        let s_off = build_field_dict_str(&req, "Agree");
        assert!(s_off.contains("/V /Off"), "{s_off}");
    }

    #[test]
    fn build_signature_field_dict() {
        let req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Signature,
            name: "Sig1".into(),
            value: None,
            default_value: None,
            options: None,
            rect: [100.0, 100.0, 300.0, 150.0],
            required: None,
            read_only: None,
            font_size: None,
            border_color: None,
            fill_color: None,
        };
        let s = build_field_dict_str(&req, "Sig1");
        assert!(s.contains("/FT /Sig"));
        assert!(s.contains("/T (Sig1)"));
    }

    #[test]
    fn escape_pdf_string_handles_parens() {
        assert_eq!(escape_pdf_string("a(b)c"), "a\\(b\\)c");
        assert_eq!(escape_pdf_string("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn create_and_round_trip_text_field() {
        // Build a tiny synthetic PDF using mupdf itself so we have valid bytes.
        let mut pdf = PdfDocument::new();
        pdf.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();
        let mut bytes = Vec::new();
        pdf.write_to(&mut bytes).unwrap();

        let mut doc = OpenedDocument {
            source_path: "test.pdf".into(),
            document_hash: crate::document_core::session::document_hash(&bytes),
            repaired: false,
            bytes,
            is_scanned: false,
            summary: crate::document_core::types::DocumentSummary {
                page_count: 1,
                object_count: 0,
                title: None,
                author: None,
                producer: None,
            },
            pages: vec![crate::document_core::types::PageInfo {
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

        let req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Text,
            name: "Greeting".into(),
            value: Some("Hello".into()),
            default_value: None,
            options: None,
            rect: [50.0, 50.0, 250.0, 80.0],
            required: None,
            read_only: None,
            font_size: Some(12.0),
            border_color: None,
            fill_color: None,
        };

        let before_len = doc.bytes.len();
        let (result, _) = create_form_field(&mut doc, &req, &[]).unwrap();
        assert!(result.success);
        assert_eq!(result.field_name, "Greeting");
        assert!(
            doc.bytes.len() > before_len,
            "bytes should grow after field creation"
        );

        // Verify the AcroForm now contains the field.
        let pdf_doc = PdfDocument::from_bytes(&doc.bytes).unwrap();
        assert!(pdf_doc.has_acro_form().unwrap());
    }

    #[test]
    fn duplicate_field_name_is_auto_suffixed() {
        let mut pdf = PdfDocument::new();
        pdf.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();
        let mut bytes = Vec::new();
        pdf.write_to(&mut bytes).unwrap();

        let mut doc = OpenedDocument {
            source_path: "t.pdf".into(),
            document_hash: crate::document_core::session::document_hash(&bytes),
            repaired: false,
            bytes,
            is_scanned: false,
            summary: crate::document_core::types::DocumentSummary {
                page_count: 1,
                object_count: 0,
                title: None,
                author: None,
                producer: None,
            },
            pages: vec![crate::document_core::types::PageInfo {
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

        let existing = vec![FormField {
            name: "Foo".into(),
            field_type: "text".into(),
            value: String::new(),
            page_index: 0,
            rect: [0.0; 4],
        }];

        let req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Text,
            name: "Foo".into(),
            value: None,
            default_value: None,
            options: None,
            rect: [10.0, 10.0, 100.0, 30.0],
            required: None,
            read_only: None,
            font_size: None,
            border_color: None,
            fill_color: None,
        };

        let (result, name) = create_form_field(&mut doc, &req, &existing).unwrap();
        assert_eq!(name, "Foo_2");
        assert_eq!(result.field_name, "Foo_2");
        assert!(result.warnings.iter().any(|w| w.contains("auto-suffixed")));
    }

    #[test]
    fn invalid_rect_rejected_by_create() {
        let mut doc = OpenedDocument {
            source_path: "t.pdf".into(),
            document_hash: crate::document_core::session::document_hash(&[0xFF]),
            repaired: false,
            bytes: vec![0xFF],
            is_scanned: false,
            summary: crate::document_core::types::DocumentSummary {
                page_count: 1,
                object_count: 0,
                title: None,
                author: None,
                producer: None,
            },
            pages: vec![crate::document_core::types::PageInfo {
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
        let req = CreateFormFieldRequest {
            session_id: "s".into(),
            page_index: 0,
            field_type: CreateFieldType::Text,
            name: "Foo".into(),
            value: None,
            default_value: None,
            options: None,
            rect: [100.0, 100.0, 50.0, 50.0], // inverted
            required: None,
            read_only: None,
            font_size: None,
            border_color: None,
            fill_color: None,
        };
        let err = create_form_field(&mut doc, &req, &[]).err().unwrap();
        assert!(format!("{err}").contains("rect"));
    }
}
