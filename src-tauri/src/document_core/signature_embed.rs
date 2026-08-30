//! Phase 26A — real signature/image overlay embedding into PDF bytes.
//!
//! Strategy: append a fresh content stream to the target page that draws an
//! image XObject inside the overlay rect, and register the image in the
//! page's `/Resources /XObject` dict. The image XObject itself is added at
//! the document level via `pdf_doc.add_image`.
//!
//! mupdf-rs only exposes `Image::from_file` (no `from_bytes`), so we
//! write the decoded bytes to a unique temp file per call, load the image,
//! then unlink the temp file. Failures are reported individually so the
//! export pipeline can record per-signature success/failure counts and
//! warnings without aborting the entire export.

use std::path::PathBuf;

use base64::{Engine as _, engine::general_purpose};
use mupdf::pdf::PdfDocument;

#[derive(Debug, Clone)]
pub struct SignatureEmbedSpec {
    pub id: String,
    pub page_index: usize,
    /// Rect in PDF user space, [x0, y0, x1, y1] (origin bottom-left).
    pub rect: [f32; 4],
    /// `data:image/...;base64,...` URL.
    pub image_data_url: String,
    /// Phase 27A — if true, the image is letterboxed/pillarboxed inside
    /// `rect`. If false, it is stretched to fill the rect.
    pub preserve_aspect: bool,
    /// Phase 27A — optional caller-supplied image dimensions in pixels.
    /// Used as a fallback when MuPDF cannot decode the embedded image
    /// dimensions directly.
    pub image_natural_width: Option<u32>,
    pub image_natural_height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SignatureEmbedReport {
    pub embedded: usize,
    pub failed: usize,
    pub warnings: Vec<String>,
    pub embedded_ids: Vec<String>,
    pub failed_ids: Vec<String>,
    /// Phase 27A — how many embeds preserved the source aspect ratio.
    pub aspect_preserved: usize,
}

impl SignatureEmbedReport {
    pub fn new() -> Self {
        Self {
            embedded: 0,
            failed: 0,
            warnings: Vec::new(),
            embedded_ids: Vec::new(),
            failed_ids: Vec::new(),
            aspect_preserved: 0,
        }
    }
}

/// Phase 27A — fit an image of (img_w, img_h) inside a target rect
/// `[x0, y0, x1, y1]`. Returns a new rect with the same aspect ratio as
/// the image, centered inside the target. Letterboxes (image wider than
/// rect) or pillarboxes (image taller than rect) as needed.
pub fn fit_rect_preserving_aspect(target: [f32; 4], img_w: f32, img_h: f32) -> [f32; 4] {
    let [x0, y0, x1, y1] = target;
    let tw = (x1 - x0).max(1e-3);
    let th = (y1 - y0).max(1e-3);
    if img_w <= 0.0 || img_h <= 0.0 {
        return target;
    }
    let target_aspect = tw / th;
    let img_aspect = img_w / img_h;
    if (target_aspect - img_aspect).abs() < 1e-3 {
        return target;
    }
    if img_aspect > target_aspect {
        // wider than target → fit width, center vertically (letterbox)
        let new_w = tw;
        let new_h = new_w / img_aspect;
        let pad = (th - new_h) / 2.0;
        [x0, y0 + pad, x1, y0 + pad + new_h]
    } else {
        // taller than target → fit height, center horizontally (pillarbox)
        let new_h = th;
        let new_w = new_h * img_aspect;
        let pad = (tw - new_w) / 2.0;
        [x0 + pad, y0, x0 + pad + new_w, y1]
    }
}

/// Parse a `data:[<mime>][;base64],<data>` URL into (mime, raw bytes).
/// Only base64 data URLs are supported (which is what our frontend
/// always emits via `FileReader.readAsDataURL`).
pub fn decode_image_data_url(data_url: &str) -> Result<(String, Vec<u8>), String> {
    let trimmed = data_url.trim();
    if !trimmed.starts_with("data:") {
        return Err("not a data: URL".to_string());
    }
    let comma = trimmed.find(',').ok_or_else(|| "data URL missing comma".to_string())?;
    let meta = &trimmed[5..comma]; // strip "data:"
    let payload = &trimmed[comma + 1..];

    if !meta.contains(";base64") {
        return Err("data URL must be base64-encoded".to_string());
    }
    let mime = meta
        .split(';')
        .next()
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = general_purpose::STANDARD
        .decode(payload.as_bytes())
        .map_err(|e| format!("base64 decode failed: {e}"))?;
    if bytes.is_empty() {
        return Err("data URL payload decoded to zero bytes".to_string());
    }
    Ok((mime, bytes))
}

/// Pick a file extension for a temp file based on the data URL mime hint.
fn extension_for_mime(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "img",
    }
}

/// Build the PDF content-stream snippet that draws the named image XObject
/// at the requested rect. Numbers are formatted with enough precision to
/// avoid sub-pixel placement drift.
pub fn build_image_draw_snippet(name: &str, rect: [f32; 4]) -> String {
    let [x0, y0, x1, y1] = rect;
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    // q ... Q saves/restores graphics state. CM applies `[w 0 0 h x0 y0]`
    // so the 1×1 image gets scaled to (w, h) and translated to (x0, y0).
    format!("q\n{w:.3} 0 0 {h:.3} {x0:.3} {y0:.3} cm\n/{name} Do\nQ\n")
}

/// Validate that an overlay rect is finite, non-degenerate, and lies
/// (mostly) within a page of `page_w_pts × page_h_pts`. Returns Err with a
/// human-readable reason for failed rects.
pub fn validate_rect(rect: [f32; 4], page_w_pts: f32, page_h_pts: f32) -> Result<(), String> {
    let [x0, y0, x1, y1] = rect;
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return Err(format!("rect has non-finite values: {rect:?}"));
    }
    if x1 <= x0 || y1 <= y0 {
        return Err(format!("rect must satisfy x1>x0 and y1>y0; got {rect:?}"));
    }
    let margin = 2.0;
    if x0 < -margin || y0 < -margin || x1 > page_w_pts + margin || y1 > page_h_pts + margin {
        return Err(format!(
            "rect {rect:?} extends outside page bounds {page_w_pts:.1}x{page_h_pts:.1}"
        ));
    }
    Ok(())
}

/// Insert the image XObject reference into page's `/Resources /XObject`,
/// preserving any existing resource entries. We always re-fetch the
/// containing dict after each `dict_put` so we operate on the
/// document-stored dict, not a stale local handle.
fn merge_into_resources(
    pdf_doc: &mut PdfDocument,
    page_index: usize,
    resource_name: &str,
    image_ref: mupdf::pdf::PdfObject,
) -> Result<(), String> {
    // Step 1: ensure /Resources exists on the page. We re-fetch the page
    // each step so we never operate on a stale clone — try_clone gives us
    // back a handle that mupdf-rs does not always treat as a live dict
    // ref for dict_put.
    {
        let mut page_dict = pdf_doc
            .find_page(page_index as i32)
            .map_err(|e| format!("find_page step1: {e}"))?;
        if !page_dict.is_dict().unwrap_or(false) {
            return Err(format!("page {page_index} is not a dict"));
        }
        let existing = page_dict
            .get_dict("Resources")
            .map_err(|e| format!("get Resources step1: {e}"))?;
        if existing.is_none() {
            let new_res = pdf_doc.new_dict().map_err(|e| format!("new_dict Resources: {e}"))?;
            page_dict
                .dict_put("Resources", new_res)
                .map_err(|e| format!("dict_put Resources: {e}"))?;
        }
    }

    // Step 2: ensure /Resources/XObject exists.
    {
        let page_dict = pdf_doc
            .find_page(page_index as i32)
            .map_err(|e| format!("find_page step2: {e}"))?;
        let mut resources = page_dict
            .get_dict("Resources")
            .map_err(|e| format!("get Resources step2: {e}"))?
            .ok_or_else(|| "Resources missing after creation".to_string())?;
        if let Some(resolved) = resources.resolve().map_err(|e| format!("resolve Resources: {e}"))? {
            resources = resolved;
        }
        if !resources.is_dict().unwrap_or(false) {
            return Err("Resources is not a dict".to_string());
        }
        let existing_xo = resources
            .get_dict("XObject")
            .map_err(|e| format!("get XObject step2: {e}"))?;
        if existing_xo.is_none() {
            let new_xo = pdf_doc.new_dict().map_err(|e| format!("new_dict XObject: {e}"))?;
            resources
                .dict_put("XObject", new_xo)
                .map_err(|e| format!("dict_put XObject: {e}"))?;
        }
    }

    // Step 3: put the image ref into /Resources/XObject/<name>.
    let page_dict = pdf_doc
        .find_page(page_index as i32)
        .map_err(|e| format!("find_page step3: {e}"))?;
    let mut resources = page_dict
        .get_dict("Resources")
        .map_err(|e| format!("get Resources step3: {e}"))?
        .ok_or_else(|| "Resources missing for put".to_string())?;
    if let Some(resolved) = resources.resolve().map_err(|e| format!("resolve Resources final: {e}"))? {
        resources = resolved;
    }
    let mut xobject = resources
        .get_dict("XObject")
        .map_err(|e| format!("get XObject step3: {e}"))?
        .ok_or_else(|| "XObject missing for put".to_string())?;
    if let Some(resolved) = xobject.resolve().map_err(|e| format!("resolve XObject: {e}"))? {
        xobject = resolved;
    }
    xobject
        .dict_put(resource_name, image_ref)
        .map_err(|e| format!("dict_put image ref: {e}"))
}

/// Embed every signature in `specs` into the corresponding page of
/// `pdf_doc`. The caller is responsible for serializing the document
/// afterwards via `pdf_doc.write_to(...)`.
pub fn embed_signatures(
    pdf_doc: &mut PdfDocument,
    page_w_pts_for: impl Fn(usize) -> Option<f32>,
    page_h_pts_for: impl Fn(usize) -> Option<f32>,
    specs: &[SignatureEmbedSpec],
) -> SignatureEmbedReport {
    let mut report = SignatureEmbedReport::new();
    if specs.is_empty() {
        return report;
    }

    let temp_dir = std::env::temp_dir();

    for (idx, spec) in specs.iter().enumerate() {
        match embed_one_signature(pdf_doc, &temp_dir, idx, &page_w_pts_for, &page_h_pts_for, spec) {
            Ok(EmbedDetail { aspect_preserved, warning }) => {
                report.embedded += 1;
                report.embedded_ids.push(spec.id.clone());
                if aspect_preserved {
                    report.aspect_preserved += 1;
                }
                if let Some(w) = warning {
                    report.warnings.push(w);
                }
            }
            Err(e) => {
                report.failed += 1;
                report.failed_ids.push(spec.id.clone());
                report.warnings.push(format!(
                    "Failed to embed signature {} on page {}: {e}",
                    spec.id, spec.page_index + 1
                ));
            }
        }
    }

    report
}

#[derive(Debug, Clone)]
pub struct EmbedDetail {
    pub aspect_preserved: bool,
    /// Non-fatal warning surfaced even though the embed succeeded
    /// (e.g. fell back to caller dims because MuPDF could not decode).
    pub warning: Option<String>,
}

fn embed_one_signature(
    pdf_doc: &mut PdfDocument,
    temp_dir: &std::path::Path,
    idx: usize,
    page_w_pts_for: &impl Fn(usize) -> Option<f32>,
    page_h_pts_for: &impl Fn(usize) -> Option<f32>,
    spec: &SignatureEmbedSpec,
) -> Result<EmbedDetail, String> {
    let _ = |e: mupdf::Error| e.to_string();

    let page_w = page_w_pts_for(spec.page_index)
        .ok_or_else(|| format!("page {} not found", spec.page_index))?;
    let page_h = page_h_pts_for(spec.page_index)
        .ok_or_else(|| format!("page {} not found", spec.page_index))?;
    validate_rect(spec.rect, page_w, page_h)?;

    // 1. Decode data URL → image bytes.
    let (mime, bytes) = decode_image_data_url(&spec.image_data_url)?;

    // 2. Write to a unique temp file (mupdf::Image::from_bytes is not exposed).
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ext = extension_for_mime(&mime);
    let tmp_path: PathBuf = temp_dir.join(format!("r2h_sig_{nanos}_{idx}.{ext}"));
    std::fs::write(&tmp_path, &bytes).map_err(|e| format!("write temp image: {e}"))?;

    let load_result: Result<EmbedDetail, String> = (|| {
        let image = mupdf::Image::from_file(
            tmp_path.to_str().ok_or_else(|| "temp path not utf-8".to_string())?,
        )
        .map_err(|e| format!("Image::from_file: {e}"))?;

        // Phase 27A — discover image natural dimensions for aspect-ratio
        // calculation. mupdf::Image exposes width()/height() returning u32;
        // if those are zero (rare), fall back to caller-supplied dims.
        let (img_w_px, img_h_px, dims_source) = {
            let w = image.width();
            let h = image.height();
            if w > 0 && h > 0 {
                (w as f32, h as f32, "mupdf")
            } else if let (Some(w), Some(h)) = (spec.image_natural_width, spec.image_natural_height) {
                (w as f32, h as f32, "caller_dims")
            } else {
                (0.0, 0.0, "unknown")
            }
        };

        // Phase 27A — fit the rect when preserve_aspect is requested and
        // we can decode dimensions. Otherwise draw at the full target rect.
        let mut warning: Option<String> = None;
        let mut aspect_preserved = false;
        let draw_rect = if spec.preserve_aspect {
            if img_w_px > 0.0 && img_h_px > 0.0 {
                aspect_preserved = true;
                if dims_source == "caller_dims" {
                    warning = Some(format!(
                        "Signature {}: used caller-supplied image dimensions for aspect ratio because backend could not decode them.",
                        spec.id
                    ));
                }
                fit_rect_preserving_aspect(spec.rect, img_w_px, img_h_px)
            } else {
                warning = Some(format!(
                    "Signature {}: preserve-aspect requested but image dimensions are unknown; falling back to stretched draw.",
                    spec.id
                ));
                spec.rect
            }
        } else {
            spec.rect
        };

        // 3. Add image XObject to document → indirect reference.
        let image_ref = pdf_doc.add_image(&image).map_err(|e| format!("add_image: {e}"))?;

        // 4. Insert the image XObject reference into /Resources/XObject.
        let resource_name = format!("R2HSig{idx}");
        merge_into_resources(pdf_doc, spec.page_index, &resource_name, image_ref)?;

        // 5. Append a new content stream that draws the image.
        let snippet = build_image_draw_snippet(&resource_name, draw_rect);
        let stream_dict_skel = pdf_doc
            .new_dict()
            .map_err(|e| format!("new_dict (stream): {e}"))?;
        let mut new_stream = pdf_doc
            .add_object(&stream_dict_skel)
            .map_err(|e| format!("add_object (stream): {e}"))?;
        new_stream
            .write_stream_string(&snippet)
            .map_err(|e| format!("write_stream_string: {e}"))?;

        let mut page_dict = pdf_doc
            .find_page(spec.page_index as i32)
            .map_err(|e| format!("find_page-contents: {e}"))?;

        let contents = page_dict
            .get_dict("Contents")
            .map_err(|e| format!("get Contents: {e}"))?;
        match contents {
            Some(c) => {
                if c.is_array().unwrap_or(false) {
                    let mut arr = c;
                    arr.array_push(new_stream)
                        .map_err(|e| format!("array_push Contents: {e}"))?;
                } else {
                    let mut arr = pdf_doc.new_array().map_err(|e| format!("new_array: {e}"))?;
                    arr.array_push(c).map_err(|e| format!("array_push original: {e}"))?;
                    arr.array_push(new_stream)
                        .map_err(|e| format!("array_push new: {e}"))?;
                    page_dict
                        .dict_put("Contents", arr)
                        .map_err(|e| format!("dict_put Contents arr: {e}"))?;
                }
            }
            None => {
                page_dict
                    .dict_put("Contents", new_stream)
                    .map_err(|e| format!("dict_put Contents: {e}"))?;
            }
        }

        Ok(EmbedDetail { aspect_preserved, warning })
    })();

    // 6. Always remove the temp file.
    let _ = std::fs::remove_file(&tmp_path);

    load_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_simple_png_data_url() {
        // 1×1 transparent PNG, base64-encoded.
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let (mime, bytes) = decode_image_data_url(url).unwrap();
        assert_eq!(mime, "image/png");
        assert!(bytes.len() > 4);
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn decode_rejects_non_data_url() {
        let err = decode_image_data_url("http://example.com/foo.png").err().unwrap();
        assert!(err.contains("not a data: URL"));
    }

    #[test]
    fn decode_rejects_missing_comma() {
        let err = decode_image_data_url("data:image/png;base64").err().unwrap();
        assert!(err.contains("missing comma"));
    }

    #[test]
    fn decode_rejects_non_base64_url() {
        let err = decode_image_data_url("data:image/png,abc%20def").err().unwrap();
        assert!(err.contains("base64-encoded"));
    }

    #[test]
    fn decode_rejects_bad_base64() {
        let err = decode_image_data_url("data:image/png;base64,@@@@").err().unwrap();
        assert!(err.contains("base64 decode failed"));
    }

    #[test]
    fn extension_for_mime_maps_common_types() {
        assert_eq!(extension_for_mime("image/png"), "png");
        assert_eq!(extension_for_mime("image/jpeg"), "jpg");
        assert_eq!(extension_for_mime("image/jpg"), "jpg");
        assert_eq!(extension_for_mime("image/gif"), "gif");
        assert_eq!(extension_for_mime("image/webp"), "webp");
        assert_eq!(extension_for_mime("application/pdf"), "img");
    }

    #[test]
    fn draw_snippet_uses_translate_and_scale() {
        let s = build_image_draw_snippet("R2HSig0", [100.0, 50.0, 300.0, 110.0]);
        assert!(s.contains("/R2HSig0 Do"));
        // Width = 200, Height = 60 — should appear in the cm matrix.
        assert!(s.contains("200.000 0 0 60.000 100.000 50.000 cm"));
        // q / Q must wrap to preserve graphics state.
        assert!(s.starts_with("q\n"));
        assert!(s.trim_end().ends_with("Q"));
    }

    #[test]
    fn validate_rect_passes_normal_rect() {
        assert!(validate_rect([10.0, 10.0, 100.0, 50.0], 612.0, 792.0).is_ok());
    }

    #[test]
    fn validate_rect_rejects_degenerate() {
        assert!(validate_rect([10.0, 10.0, 10.0, 50.0], 612.0, 792.0).is_err());
        assert!(validate_rect([10.0, 50.0, 50.0, 50.0], 612.0, 792.0).is_err());
    }

    #[test]
    fn validate_rect_rejects_off_page() {
        assert!(validate_rect([1000.0, 0.0, 1100.0, 50.0], 612.0, 792.0).is_err());
    }

    #[test]
    fn validate_rect_rejects_non_finite() {
        assert!(validate_rect([f32::NAN, 0.0, 10.0, 10.0], 612.0, 792.0).is_err());
        assert!(validate_rect([0.0, 0.0, f32::INFINITY, 10.0], 612.0, 792.0).is_err());
    }

    // End-to-end: embed a 1x1 PNG into a synthetic single-page PDF and
    // confirm the resulting bytes carry the image XObject reference + draw
    // operator. We don't require external rendering to validate.
    #[test]
    fn embed_signature_changes_pdf_bytes_and_references_image() {
        // Build a minimal valid PDF using mupdf's own API.
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size { width: 612.0, height: 792.0 }).unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();

        let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes).unwrap();
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let specs = vec![SignatureEmbedSpec {
            id: "sig-1".into(),
            page_index: 0,
            rect: [100.0, 100.0, 300.0, 160.0],
            image_data_url: url.into(),
            preserve_aspect: false,
            image_natural_width: None,
            image_natural_height: None,
        }];
        let report = embed_signatures(
            &mut pdf,
            |_p| Some(612.0),
            |_p| Some(792.0),
            &specs,
        );
        assert_eq!(report.failed, 0, "warnings: {:?}", report.warnings);
        assert_eq!(report.embedded, 1);
        assert!(report.embedded_ids.contains(&"sig-1".to_string()));

        // Serialise and look for the draw operator we appended.
        let mut out: Vec<u8> = Vec::new();
        pdf.write_to(&mut out).unwrap();
        assert!(out.len() > bytes.len(), "expected pdf to grow after image embed");
        let stringy: String = String::from_utf8_lossy(&out).to_string();
        assert!(stringy.contains("/R2HSig0"), "expected /R2HSig0 resource in output");
    }

    // ─── Phase 27A: fit_rect_preserving_aspect ───────────────────────

    #[test]
    fn fit_rect_aspect_equal_returns_target() {
        let r = fit_rect_preserving_aspect([0.0, 0.0, 200.0, 100.0], 200.0, 100.0);
        assert!((r[0] - 0.0).abs() < 1e-3);
        assert!((r[1] - 0.0).abs() < 1e-3);
        assert!((r[2] - 200.0).abs() < 1e-3);
        assert!((r[3] - 100.0).abs() < 1e-3);
    }

    #[test]
    fn fit_rect_image_wider_than_target_letterboxes() {
        // Image 400×100 (aspect 4), target 200×200 (aspect 1).
        // Fit width → 200×50, vertically centered inside 200 tall → pad 75.
        let r = fit_rect_preserving_aspect([0.0, 0.0, 200.0, 200.0], 400.0, 100.0);
        assert!((r[0] - 0.0).abs() < 1e-3, "x0={}", r[0]);
        assert!((r[1] - 75.0).abs() < 1e-3, "y0={}", r[1]);
        assert!((r[2] - 200.0).abs() < 1e-3, "x1={}", r[2]);
        assert!((r[3] - 125.0).abs() < 1e-3, "y1={}", r[3]);
    }

    #[test]
    fn fit_rect_image_taller_than_target_pillarboxes() {
        // Image 100×400 (aspect 0.25), target 200×200.
        // Fit height → 50×200, horizontally centered → pad 75.
        let r = fit_rect_preserving_aspect([0.0, 0.0, 200.0, 200.0], 100.0, 400.0);
        assert!((r[0] - 75.0).abs() < 1e-3);
        assert!((r[1] - 0.0).abs() < 1e-3);
        assert!((r[2] - 125.0).abs() < 1e-3);
        assert!((r[3] - 200.0).abs() < 1e-3);
    }

    #[test]
    fn fit_rect_returns_target_for_zero_image() {
        let r = fit_rect_preserving_aspect([10.0, 10.0, 50.0, 30.0], 0.0, 0.0);
        assert_eq!(r, [10.0, 10.0, 50.0, 30.0]);
    }

    #[test]
    fn fit_rect_landscape_image_in_portrait_box_letterboxes() {
        // Landscape 200×50 (aspect 4), portrait box 100×400 (aspect 0.25).
        // Image is wider → fit width 100, h=100/4=25 → pad (400-25)/2=187.5.
        let r = fit_rect_preserving_aspect([0.0, 0.0, 100.0, 400.0], 200.0, 50.0);
        assert!((r[0] - 0.0).abs() < 1e-3);
        assert!((r[1] - 187.5).abs() < 1e-3);
        assert!((r[2] - 100.0).abs() < 1e-3);
        assert!((r[3] - 212.5).abs() < 1e-3);
    }

    #[test]
    fn fit_rect_portrait_image_in_landscape_box_pillarboxes() {
        // Portrait 50×200 (aspect 0.25), landscape box 400×100 (aspect 4).
        // Image is taller → fit height 100, w=100*0.25=25 → pad (400-25)/2=187.5.
        let r = fit_rect_preserving_aspect([0.0, 0.0, 400.0, 100.0], 50.0, 200.0);
        assert!((r[0] - 187.5).abs() < 1e-3);
        assert!((r[1] - 0.0).abs() < 1e-3);
        assert!((r[2] - 212.5).abs() < 1e-3);
        assert!((r[3] - 100.0).abs() < 1e-3);
    }

    #[test]
    fn embed_signature_with_preserve_aspect_reports_aspect_preserved() {
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size { width: 612.0, height: 792.0 }).unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();
        let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes).unwrap();

        // 1×1 PNG — aspect 1.0.
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let specs = vec![SignatureEmbedSpec {
            id: "sig-aspect".into(),
            page_index: 0,
            rect: [100.0, 100.0, 300.0, 160.0],
            image_data_url: url.into(),
            preserve_aspect: true,
            image_natural_width: None,
            image_natural_height: None,
        }];
        let report = embed_signatures(
            &mut pdf,
            |_| Some(612.0),
            |_| Some(792.0),
            &specs,
        );
        assert_eq!(report.embedded, 1, "warnings: {:?}", report.warnings);
        assert_eq!(report.aspect_preserved, 1);
    }

    #[test]
    fn embed_signature_without_preserve_aspect_does_not_count_aspect_preserved() {
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size { width: 612.0, height: 792.0 }).unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();
        let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes).unwrap();

        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
        let specs = vec![SignatureEmbedSpec {
            id: "sig-stretch".into(),
            page_index: 0,
            rect: [100.0, 100.0, 300.0, 160.0],
            image_data_url: url.into(),
            preserve_aspect: false,
            image_natural_width: None,
            image_natural_height: None,
        }];
        let report = embed_signatures(
            &mut pdf,
            |_| Some(612.0),
            |_| Some(792.0),
            &specs,
        );
        assert_eq!(report.embedded, 1);
        assert_eq!(report.aspect_preserved, 0);
    }

    #[test]
    fn embed_signature_reports_failure_for_bad_data_url() {
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size { width: 612.0, height: 792.0 }).unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();
        let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes).unwrap();
        let specs = vec![SignatureEmbedSpec {
            id: "bad".into(),
            page_index: 0,
            rect: [10.0, 10.0, 50.0, 50.0],
            image_data_url: "not a data url".into(),
            preserve_aspect: false,
            image_natural_width: None,
            image_natural_height: None,
        }];
        let report = embed_signatures(&mut pdf, |_| Some(612.0), |_| Some(792.0), &specs);
        assert_eq!(report.embedded, 0);
        assert_eq!(report.failed, 1);
        assert!(report.warnings.iter().any(|w| w.contains("Failed to embed signature bad")));
    }
}
