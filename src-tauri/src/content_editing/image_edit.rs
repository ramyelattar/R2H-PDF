//! Native image editing: replace, delete, move, crop, and rotate image XObjects.

use std::time::{SystemTime, UNIX_EPOCH};
use crate::document_core::DocumentCoreState;
use super::types::*;
use super::analysis::extract_page_content_objects;

/// Replace an image XObject in the PDF with new image data.
pub fn replace_native_image(
    doc_state: &DocumentCoreState,
    request: &NativeImageReplaceRequest,
) -> Result<NativeImageEditResult, String> {
    let edit_id = format!("img-edit-{}", epoch_ms());

    // Verify the content object exists and is an image.
    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects.iter().find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;

    if target.object_type != ContentObjectType::ImageXobject {
        return Err(format!("Object {} is not an image (type: {:?})", request.content_object_id, target.object_type));
    }

    if target.editable_level == EditableLevel::ReadOnly {
        return Ok(NativeImageEditResult {
            edit_id,
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            content_object_id: request.content_object_id.clone(),
            method: EditMethod::Rejected,
            action: "replace".to_string(),
            success: false,
            warnings: vec!["Image object is read-only.".to_string()],
        });
    }

    // Decode the replacement image from base64 and save to temp file.
    let image_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &request.image_bytes_base64,
    ).map_err(|e| format!("Invalid base64 image data: {e}"))?;

    // Write to temp file for MuPDF to load.
    let temp_path = std::env::temp_dir().join(format!("r2h_img_replace_{}.png", epoch_ms()));
    std::fs::write(&temp_path, &image_bytes)
        .map_err(|e| format!("Failed to write temp image: {e}"))?;

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;

    // Load the PDF and replace the image.
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    // Create a new image from the temp file.
    let temp_path_str = temp_path.to_string_lossy().to_string();
    let new_image = mupdf::Image::from_file(&temp_path_str)
        .map_err(|e| format!("Failed to decode replacement image: {e}"))?;

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);

    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page)
        .map_err(|e| format!("PdfPage: {e}"))?;

    // Safe visual replacement: cover the old image bbox and draw the new
    // image into the same rect. This avoids claiming a native XObject swap
    // when the analysis layer cannot prove the original resource name.
    redact_rect_on_page(&mut pdf_page, target.bbox)?;
    let xobj_name = format!("R2HImg{}", epoch_ms());
    let image_obj = pdf.add_image(&new_image)
        .map_err(|e| format!("Failed to add image to PDF: {e}"))?;
    let page_obj = pdf_page.object();
    ensure_image_xobject_resource(&pdf, &page_obj, &xobj_name, image_obj)?;
    let draw_spec = build_replacement_draw_spec(
        target.bbox,
        request.layout_mode.as_str(),
        new_image.width() as f32,
        new_image.height() as f32,
    )?;
    append_image_draw_ops_to_page_contents(&mut pdf, request.page_index, &xobj_name, &draw_spec)?;

    // Serialize back.
    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes)
        .map_err(|e| format!("serialize PDF: {e}"))?;

    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeImageEditResult {
        edit_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        action: "replace".to_string(),
        success: true,
        warnings: vec![
            "Image replaced using safe visual replacement: original bbox covered, replacement image embedded and drawn on top. Native XObject swap was not claimed because the original resource name could not be verified.".to_string(),
        ],
    })
}

/// Delete an image from the page by removing its drawing operation.
pub fn delete_native_image(
    doc_state: &DocumentCoreState,
    request: &NativeImageDeleteRequest,
) -> Result<NativeImageEditResult, String> {
    let edit_id = format!("img-del-{}", epoch_ms());

    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects.iter().find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;

    if target.object_type != ContentObjectType::ImageXobject {
        return Err(format!("Object {} is not an image", request.content_object_id));
    }

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;

    // Strategy: Redact the image area with white fill to remove it visually.
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page)
        .map_err(|e| format!("PdfPage: {e}"))?;

    redact_rect_on_page(&mut pdf_page, target.bbox)?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;

    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeImageEditResult {
        edit_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        action: "delete".to_string(),
        success: true,
        warnings: vec![
            "Image deleted using visual cover: original image bbox redacted/covered with white.".to_string(),
        ],
    })
}

/// Move/resize an image by redacting the old position and redrawing at the new position.
/// True matrix editing in the content stream is complex; this uses the safe visual approach.
pub fn move_native_image(
    doc_state: &DocumentCoreState,
    request: &NativeImageMoveRequest,
) -> Result<NativeImageEditResult, String> {
    let edit_id = format!("img-move-{}", epoch_ms());

    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects.iter().find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;

    if target.object_type != ContentObjectType::ImageXobject {
        return Err(format!("Object {} is not an image", request.content_object_id));
    }

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;

    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let original_image = find_image_for_target(&pdf, request.page_index, target)
        .map_err(|e| format!("Cannot extract original image for move/resize: {e}"))?
        .ok_or_else(|| "Cannot move image safely: original image pixels could not be extracted for redraw.".to_string())?;

    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page)
        .map_err(|e| format!("PdfPage: {e}"))?;

    // Step 1: Redact the old image position.
    redact_rect_on_page(&mut pdf_page, target.bbox)?;

    // Step 2: Embed and draw the same image at the new position.
    let new_rect = request.new_rect;
    let xobj_name = format!("R2HImg{}", epoch_ms());
    let image_obj = pdf.add_image(&original_image)
        .map_err(|e| format!("Failed to add original image for redraw: {e}"))?;
    let page_obj = pdf_page.object();
    ensure_image_xobject_resource(&pdf, &page_obj, &xobj_name, image_obj)?;
    let draw_spec = ImageDrawSpec::simple(new_rect)?;
    append_image_draw_ops_to_page_contents(&mut pdf, request.page_index, &xobj_name, &draw_spec)?;

    // Serialize back.
    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;

    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeImageEditResult {
        edit_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        action: "move_resize".to_string(),
        success: true,
        warnings: vec![
            "Image moved using safe visual method: old position redacted, image redrawn at new position.".to_string(),
        ],
    })
}

/// Crop an image by redacting the old bbox and redrawing the original image
/// through a clipping rectangle. This is intentionally labeled visual crop:
/// it does not claim a native image mask rewrite.
pub fn crop_native_image(
    doc_state: &DocumentCoreState,
    request: &NativeImageCropRequest,
) -> Result<NativeImageEditResult, String> {
    let edit_id = format!("img-crop-{}", epoch_ms());
    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects.iter().find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;
    if target.object_type != ContentObjectType::ImageXobject {
        return Err(format!("Object {} is not an image", request.content_object_id));
    }
    let crop_rect = normalize_crop_rect(target.bbox, request.crop_rect)?;
    let target_rect = request.target_rect.unwrap_or(target.bbox);

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let original_image = find_image_for_target(&pdf, request.page_index, target)
        .map_err(|e| format!("Cannot extract original image for crop: {e}"))?
        .ok_or_else(|| "Cannot crop image safely: original image pixels could not be extracted for redraw.".to_string())?;

    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page)
        .map_err(|e| format!("PdfPage: {e}"))?;
    redact_rect_on_page(&mut pdf_page, target.bbox)?;

    let xobj_name = format!("R2HImg{}", epoch_ms());
    let image_obj = pdf.add_image(&original_image)
        .map_err(|e| format!("Failed to add original image for cropped redraw: {e}"))?;
    let page_obj = pdf_page.object();
    ensure_image_xobject_resource(&pdf, &page_obj, &xobj_name, image_obj)?;
    let draw_spec = build_crop_draw_spec(target.bbox, crop_rect, target_rect)?;
    append_image_draw_ops_to_page_contents(&mut pdf, request.page_index, &xobj_name, &draw_spec)?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;
    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeImageEditResult {
        edit_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        action: "crop".to_string(),
        success: true,
        warnings: vec![
            "Visual image crop: old image bbox covered, original image redrawn through a crop clipping path. Native image mask rewrite was not claimed.".to_string(),
        ],
    })
}

/// Rotate an image by redacting the old bbox and redrawing the original image
/// with a rotation matrix centered in the original bbox.
pub fn rotate_native_image(
    doc_state: &DocumentCoreState,
    request: &NativeImageRotateRequest,
) -> Result<NativeImageEditResult, String> {
    let edit_id = format!("img-rot-{}", epoch_ms());
    let normalized = normalize_rotation_degrees(request.degrees)?;
    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects.iter().find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;
    if target.object_type != ContentObjectType::ImageXobject {
        return Err(format!("Object {} is not an image", request.content_object_id));
    }

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let original_image = find_image_for_target(&pdf, request.page_index, target)
        .map_err(|e| format!("Cannot extract original image for rotation: {e}"))?
        .ok_or_else(|| "Cannot rotate image safely: original image pixels could not be extracted for redraw.".to_string())?;

    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page)
        .map_err(|e| format!("PdfPage: {e}"))?;
    redact_rect_on_page(&mut pdf_page, target.bbox)?;

    let xobj_name = format!("R2HImg{}", epoch_ms());
    let image_obj = pdf.add_image(&original_image)
        .map_err(|e| format!("Failed to add original image for rotated redraw: {e}"))?;
    let page_obj = pdf_page.object();
    ensure_image_xobject_resource(&pdf, &page_obj, &xobj_name, image_obj)?;
    let draw_spec = build_rotation_draw_spec(target.bbox, normalized)?;
    append_image_draw_ops_to_page_contents(&mut pdf, request.page_index, &xobj_name, &draw_spec)?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;
    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeImageEditResult {
        edit_id,
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        action: "rotate".to_string(),
        success: true,
        warnings: vec![
            format!("Visual image rotation: old image bbox covered, image redrawn with a {normalized} degree rotation matrix. Arbitrary-angle native matrix rewriting was not claimed."),
        ],
    })
}

fn redact_rect_on_page(pdf_page: &mut mupdf::pdf::PdfPage, bbox: [f32; 4]) -> Result<(), String> {
    let rect = mupdf::Rect::new(bbox[0], bbox[1], bbox[2], bbox[3]);
    let mut redact = pdf_page.create_annotation(mupdf::pdf::PdfAnnotationType::Redact)
        .map_err(|e| format!("create redact: {e}"))?;
    redact.set_rect(rect).map_err(|e| format!("set_rect: {e}"))?;
    redact.set_color(mupdf::color::AnnotationColor::Gray(1.0))
        .map_err(|e| format!("set_color: {e}"))?;
    drop(redact);
    pdf_page.redact().map_err(|e| format!("redact: {e}"))?;
    Ok(())
}

fn ensure_image_xobject_resource(
    pdf: &mupdf::pdf::PdfDocument,
    page_obj: &mupdf::pdf::PdfObject,
    name: &str,
    image_obj: mupdf::pdf::PdfObject,
) -> Result<(), String> {
    let mut page_obj_mut = page_obj.clone();
    let resources = match page_obj.get_dict("Resources") {
        Ok(Some(r)) => r,
        Ok(None) => {
            let new_resources = pdf.new_dict().map_err(|e| format!("new Resources dict: {e}"))?;
            page_obj_mut.dict_put("Resources", new_resources.clone())
                .map_err(|e| format!("dict_put Resources: {e}"))?;
            new_resources
        }
        Err(e) => return Err(format!("get Resources: {e}")),
    };
    let mut resources_mut = resources.clone();
    let mut xobjects = match resources.get_dict("XObject") {
        Ok(Some(x)) => x,
        Ok(None) => {
            let new_xobjects = pdf.new_dict().map_err(|e| format!("new XObject dict: {e}"))?;
            resources_mut.dict_put("XObject", new_xobjects.clone())
                .map_err(|e| format!("dict_put XObject: {e}"))?;
            new_xobjects
        }
        Err(e) => return Err(format!("get XObject: {e}")),
    };
    xobjects.dict_put(name, image_obj)
        .map_err(|e| format!("dict_put image XObject /{name}: {e}"))?;
    Ok(())
}

fn append_image_draw_ops_to_page_contents(
    pdf: &mut mupdf::pdf::PdfDocument,
    page_index: usize,
    xobject_name: &str,
    spec: &ImageDrawSpec,
) -> Result<(), String> {
    let ops = build_image_draw_ops(xobject_name, spec)?;
    let stream_dict = pdf.new_dict().map_err(|e| format!("new image stream dict: {e}"))?;
    let mut new_stream = pdf.add_object(&stream_dict).map_err(|e| format!("add image draw stream: {e}"))?;
    new_stream.write_stream_string(&ops)
        .map_err(|e| format!("write image draw stream: {e}"))?;

    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    let mut page_dict = pdf.find_page(page_no).map_err(|e| format!("find_page image append: {e}"))?;
    match page_dict.get_dict("Contents").map_err(|e| format!("get Contents image append: {e}"))? {
        Some(contents) if contents.is_array().unwrap_or(false) => {
            let mut arr = contents;
            arr.array_push(new_stream).map_err(|e| format!("array_push image stream: {e}"))?;
        }
        Some(contents) => {
            let mut arr = pdf.new_array().map_err(|e| format!("new Contents array: {e}"))?;
            arr.array_push(contents).map_err(|e| format!("array_push existing Contents: {e}"))?;
            arr.array_push(new_stream).map_err(|e| format!("array_push image stream: {e}"))?;
            page_dict.dict_put("Contents", arr).map_err(|e| format!("dict_put Contents array: {e}"))?;
        }
        None => {
            page_dict.dict_put("Contents", new_stream).map_err(|e| format!("dict_put image Contents: {e}"))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
struct ImageDrawSpec {
    matrix: [f32; 6],
    clip_rect: Option<[f32; 4]>,
}

impl ImageDrawSpec {
    fn simple(rect: [f32; 4]) -> Result<Self, String> {
        validate_rect(rect, "image draw rect")?;
        Ok(Self {
            matrix: [rect[2] - rect[0], 0.0, 0.0, rect[3] - rect[1], rect[0], rect[1]],
            clip_rect: None,
        })
    }
}

fn build_image_draw_ops(xobject_name: &str, spec: &ImageDrawSpec) -> Result<String, String> {
    if spec.matrix.iter().any(|v| !v.is_finite()) {
        return Err("image transform matrix is non-finite".to_string());
    }
    let mut ops = String::from("\nq\n");
    if let Some(rect) = spec.clip_rect {
        validate_rect(rect, "image clip rect")?;
        ops.push_str(&format!(
            "{} {} {} {} re\nW\nn\n",
            fmt_f32(rect[0]),
            fmt_f32(rect[1]),
            fmt_f32(rect[2] - rect[0]),
            fmt_f32(rect[3] - rect[1]),
        ));
    }
    ops.push_str(&format!(
        "{} {} {} {} {} {} cm\n/{} Do\nQ\n",
        fmt_f32(spec.matrix[0]),
        fmt_f32(spec.matrix[1]),
        fmt_f32(spec.matrix[2]),
        fmt_f32(spec.matrix[3]),
        fmt_f32(spec.matrix[4]),
        fmt_f32(spec.matrix[5]),
        xobject_name,
    ));
    Ok(ops)
}

fn build_replacement_draw_spec(
    bbox: [f32; 4],
    layout_mode: &str,
    image_width: f32,
    image_height: f32,
) -> Result<ImageDrawSpec, String> {
    validate_rect(bbox, "image replacement bbox")?;
    if image_width <= 0.0 || image_height <= 0.0 || !image_width.is_finite() || !image_height.is_finite() {
        return Err("replacement image dimensions are invalid".to_string());
    }
    let bbox_w = bbox[2] - bbox[0];
    let bbox_h = bbox[3] - bbox[1];
    let image_ratio = image_width / image_height;
    let bbox_ratio = bbox_w / bbox_h;
    let (draw_w, draw_h, clip) = match layout_mode {
        "stretch" => (bbox_w, bbox_h, None),
        "fit" => {
            if image_ratio > bbox_ratio {
                (bbox_w, bbox_w / image_ratio, None)
            } else {
                (bbox_h * image_ratio, bbox_h, None)
            }
        }
        "fill" => {
            if image_ratio > bbox_ratio {
                (bbox_h * image_ratio, bbox_h, Some(bbox))
            } else {
                (bbox_w, bbox_w / image_ratio, Some(bbox))
            }
        }
        other => return Err(format!("unsupported image layout mode: {other}")),
    };
    let x = bbox[0] + (bbox_w - draw_w) / 2.0;
    let y = bbox[1] + (bbox_h - draw_h) / 2.0;
    Ok(ImageDrawSpec {
        matrix: [draw_w, 0.0, 0.0, draw_h, x, y],
        clip_rect: clip,
    })
}

fn normalize_crop_rect(image_bbox: [f32; 4], crop_rect: [f32; 4]) -> Result<[f32; 4], String> {
    validate_rect(image_bbox, "image bbox")?;
    validate_rect(crop_rect, "crop rect")?;
    let clamped = [
        crop_rect[0].max(image_bbox[0]).min(image_bbox[2]),
        crop_rect[1].max(image_bbox[1]).min(image_bbox[3]),
        crop_rect[2].max(image_bbox[0]).min(image_bbox[2]),
        crop_rect[3].max(image_bbox[1]).min(image_bbox[3]),
    ];
    validate_rect(clamped, "crop rect")?;
    Ok(clamped)
}

fn build_crop_draw_spec(
    image_bbox: [f32; 4],
    crop_rect: [f32; 4],
    target_rect: [f32; 4],
) -> Result<ImageDrawSpec, String> {
    validate_rect(image_bbox, "image bbox")?;
    let crop = normalize_crop_rect(image_bbox, crop_rect)?;
    validate_rect(target_rect, "crop target rect")?;
    let image_w = image_bbox[2] - image_bbox[0];
    let image_h = image_bbox[3] - image_bbox[1];
    let crop_w = crop[2] - crop[0];
    let crop_h = crop[3] - crop[1];
    let target_w = target_rect[2] - target_rect[0];
    let target_h = target_rect[3] - target_rect[1];
    let draw_w = target_w * image_w / crop_w;
    let draw_h = target_h * image_h / crop_h;
    let draw_x = target_rect[0] - (crop[0] - image_bbox[0]) * draw_w / image_w;
    let draw_y = target_rect[1] - (crop[1] - image_bbox[1]) * draw_h / image_h;
    Ok(ImageDrawSpec {
        matrix: [draw_w, 0.0, 0.0, draw_h, draw_x, draw_y],
        clip_rect: Some(target_rect),
    })
}

fn normalize_rotation_degrees(degrees: i32) -> Result<i32, String> {
    let normalized = ((degrees % 360) + 360) % 360;
    match normalized {
        0 | 90 | 180 | 270 => Ok(normalized),
        _ => Err("Custom image rotation is disabled: only 90, 180, and 270 degree export-safe increments are supported.".to_string()),
    }
}

fn build_rotation_draw_spec(rect: [f32; 4], degrees: i32) -> Result<ImageDrawSpec, String> {
    validate_rect(rect, "image rotation rect")?;
    let normalized = normalize_rotation_degrees(degrees)?;
    let (cos, sin) = match normalized {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => unreachable!("normalize_rotation_degrees only returns 0/90/180/270"),
    };
    let w = rect[2] - rect[0];
    let h = rect[3] - rect[1];
    let a = w * cos;
    let b = w * sin;
    let c = -h * sin;
    let d = h * cos;
    let cx = rect[0] + w / 2.0;
    let cy = rect[1] + h / 2.0;
    let e = cx - (a + c) / 2.0;
    let f = cy - (b + d) / 2.0;
    Ok(ImageDrawSpec {
        matrix: [a, b, c, d, e, f],
        clip_rect: Some(rect),
    })
}

fn validate_rect(rect: [f32; 4], label: &str) -> Result<(), String> {
    if rect.iter().any(|v| !v.is_finite()) || rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Err(format!("{label} is degenerate"));
    }
    Ok(())
}

fn fmt_f32(v: f32) -> String {
    let s = format!("{:.4}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn find_image_for_target(
    pdf: &mupdf::pdf::PdfDocument,
    page_index: usize,
    target: &ContentObject,
) -> Result<Option<mupdf::Image>, String> {
    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    let page = pdf.load_page(page_no).map_err(|e| format!("load_page: {e}"))?;
    let text_page = page.to_text_page(mupdf::TextPageFlags::COLLECT_VECTORS)
        .map_err(|e| format!("to_text_page: {e}"))?;
    for block in text_page.blocks() {
        if block.r#type() != mupdf::text_page::TextBlockType::Image {
            continue;
        }
        let Some(transform) = block.ctm() else { continue; };
        let bbox = [
            transform.e,
            transform.f,
            transform.e + transform.a.abs(),
            transform.f + transform.d.abs(),
        ];
        if rects_close(bbox, target.bbox, 1.0) {
            return Ok(block.image());
        }
    }
    Ok(None)
}

fn rects_close(a: [f32; 4], b: [f32; 4], eps: f32) -> bool {
    (a[0] - b[0]).abs() <= eps
        && (a[1] - b[1]).abs() <= eps
        && (a[2] - b[2]).abs() <= eps
        && (a[3] - b[3]).abs() <= eps
}

fn epoch_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_layout_modes_compute_export_matrices() {
        let bbox = [10.0, 20.0, 110.0, 70.0];

        let stretch = build_replacement_draw_spec(bbox, "stretch", 400.0, 200.0).unwrap();
        assert_eq!(stretch.matrix, [100.0, 0.0, 0.0, 50.0, 10.0, 20.0]);
        assert_eq!(stretch.clip_rect, None);

        let fit = build_replacement_draw_spec(bbox, "fit", 400.0, 200.0).unwrap();
        assert_eq!(fit.matrix, [100.0, 0.0, 0.0, 50.0, 10.0, 20.0]);
        assert_eq!(fit.clip_rect, None);

        let tall_fit = build_replacement_draw_spec(bbox, "fit", 100.0, 200.0).unwrap();
        assert_eq!(tall_fit.matrix, [25.0, 0.0, 0.0, 50.0, 47.5, 20.0]);

        let fill = build_replacement_draw_spec(bbox, "fill", 100.0, 200.0).unwrap();
        assert_eq!(fill.matrix, [100.0, 0.0, 0.0, 200.0, 10.0, -55.0]);
        assert_eq!(fill.clip_rect, Some(bbox));
    }

    #[test]
    fn crop_draw_spec_clips_target_and_scales_original() {
        let image_bbox = [0.0, 0.0, 200.0, 100.0];
        let crop = [50.0, 25.0, 150.0, 75.0];
        let target = [10.0, 20.0, 210.0, 120.0];
        let spec = build_crop_draw_spec(image_bbox, crop, target).unwrap();
        assert_eq!(spec.clip_rect, Some(target));
        assert_eq!(spec.matrix, [400.0, 0.0, 0.0, 200.0, -90.0, -30.0]);
    }

    #[test]
    fn rotation_draw_spec_uses_centered_matrix_and_clip() {
        let rect = [10.0, 20.0, 110.0, 70.0];
        let spec = build_rotation_draw_spec(rect, 90).unwrap();
        assert_eq!(spec.clip_rect, Some(rect));
        assert_eq!(spec.matrix, [0.0, 100.0, -50.0, 0.0, 85.0, -5.0]);

        let half_turn = build_rotation_draw_spec(rect, 180).unwrap();
        assert_eq!(half_turn.matrix, [-100.0, 0.0, -0.0, -50.0, 110.0, 70.0]);
    }

    #[test]
    fn unsupported_custom_rotation_is_rejected() {
        assert!(normalize_rotation_degrees(45).is_err());
        assert_eq!(normalize_rotation_degrees(-90).unwrap(), 270);
    }

    #[test]
    fn image_draw_ops_include_clip_before_do() {
        let spec = ImageDrawSpec {
            matrix: [100.0, 0.0, 0.0, 50.0, 10.0, 20.0],
            clip_rect: Some([10.0, 20.0, 110.0, 70.0]),
        };
        let ops = build_image_draw_ops("R2HImg1", &spec).unwrap();
        assert!(ops.contains("10 20 100 50 re\nW\nn\n"));
        assert!(ops.contains("100 0 0 50 10 20 cm\n/R2HImg1 Do"));
    }
}
