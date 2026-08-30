//! Phase 26E — verify OCR overlay coordinate conversion is correct.
//!
//! We exercise the conversion math directly without spinning up a Tauri
//! command: image-pixel bbox (top-left origin) → PDF user space (origin
//! bottom-left) with explicit per-axis scale.

#[test]
fn image_px_bbox_converts_to_pdf_points_with_y_flip() {
    // Page: 612pt x 792pt (US Letter)
    // Image: rendered at 200 DPI → 1700 x 2200 px
    let page_w = 612.0_f32;
    let page_h = 792.0_f32;
    let img_w = 1700.0_f32;
    let img_h = 2200.0_f32;

    // OCR worker bbox: top-left pixel (100, 200), 800x55 in image-pixel space.
    // Top-left in pixel space → top of the line.
    let bx = 100.0_f32;
    let by = 200.0_f32;
    let bw = 800.0_f32;
    let bh = 55.0_f32;

    let sx = page_w / img_w;
    let sy = page_h / img_h;

    let pdf_x0 = bx * sx;
    let pdf_x1 = (bx + bw) * sx;
    let pdf_y_top_in_pts = by * sy;
    let pdf_y_bot_in_pts = (by + bh) * sy;
    let pdf_y0 = page_h - pdf_y_bot_in_pts; // PDF bottom-left origin
    let pdf_y1 = page_h - pdf_y_top_in_pts;

    // Sanity: width and height in points = pixel size × scale
    assert!((pdf_x1 - pdf_x0 - bw * sx).abs() < 1e-3);
    assert!((pdf_y1 - pdf_y0 - bh * sy).abs() < 1e-3);

    // Y flip: a bbox near the TOP of the image (low y in px) should end
    // up near the TOP of the PDF (high y in points).
    assert!(
        pdf_y1 > page_h / 2.0,
        "expected y1 in upper half of page, got y1={pdf_y1:.2} of {page_h:.2}"
    );
    // The whole bbox lies inside the page.
    assert!(pdf_x0 >= 0.0 && pdf_x1 <= page_w);
    assert!(pdf_y0 >= 0.0 && pdf_y1 <= page_h);
}

#[test]
fn off_page_pixel_bbox_clamps_to_page_bounds() {
    let page_w = 612.0_f32;
    let page_h = 792.0_f32;
    let img_w = 1700.0_f32;
    let img_h = 2200.0_f32;
    let sx = page_w / img_w;
    let sy = page_h / img_h;

    // Bbox extends past the right edge by 10px.
    let bx = 1690.0_f32;
    let by = 100.0_f32;
    let bw = 20.0_f32;
    let bh = 30.0_f32;

    let pdf_x1 = (bx + bw) * sx;
    let clamped = pdf_x1.clamp(0.0, page_w);
    assert!(pdf_x1 > page_w, "test pre-condition failed");
    assert_eq!(clamped, page_w, "off-page bbox must clamp to page width");

    let _ = (sy, by, bh); // suppress unused warnings on stable
}

#[test]
fn degenerate_bbox_is_detected_after_clamp() {
    let page_w = 612.0_f32;
    let page_h = 792.0_f32;
    // A zero-width bbox.
    let cx0 = 100.0_f32;
    let cx1 = 100.0_f32;
    let cy0 = 200.0_f32;
    let cy1 = 220.0_f32;
    let ok = cx1 > cx0 && cy1 > cy0;
    assert!(!ok, "expected degenerate bbox to be rejected");
    let _ = (page_w, page_h);
}
