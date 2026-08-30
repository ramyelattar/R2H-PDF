//! Real end-to-end PP-OCRv5 fast-path validation.
//!
//! Exercises the production pipeline with NO mocks:
//!   SessionStore::open_document (MuPDF)
//!   → render_page at 200 DPI
//!   → production save_rgba_as_png (alpha flattened over white)
//!   → OcrEngine standard path (PP-OCRv5_mobile_det → arabic_PP-OCRv5_mobile_rec)
//!   → strict worker-response validation
//!   → PDF-point bbox conversion (the same converter the overlay acceptance
//!     command uses)
//!
//! Requires the local OCR pack (models + Python runtime with paddlepaddle);
//! ignored by default so CI machines without the pack stay green. Run with:
//!   cargo test --test ocr_fast_path_integration -- --ignored --nocapture

use std::path::PathBuf;
use std::time::Instant;

use r2h_pdf_lib::document_core::ipc::save_rgba_as_png;
use r2h_pdf_lib::document_core::ocr::{
    convert_ocr_bbox_to_pdf_points, OcrResultStatus, OcrRunContext, OcrState,
};
use r2h_pdf_lib::document_core::session::SessionStore;
use r2h_pdf_lib::document_core::types::{BBox, OpenDocumentRequest, RenderRequest};

const SCANNED_SAMPLE: &str = "../demo/input/scanned-ocr-sample.pdf";

#[test]
#[ignore = "requires the local PP-OCRv5 model pack and Python runtime (real inference)"]
fn ppocrv5_fast_path_end_to_end_real_inference() {
    let pdf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SCANNED_SAMPLE);
    assert!(pdf_path.is_file(), "scanned fixture missing: {pdf_path:?}");

    // 1. Open the scanned PDF through the real session store.
    let store = SessionStore::new();
    let opened = store
        .open_document(OpenDocumentRequest {
            path: pdf_path.to_string_lossy().to_string(),
            recover_if_damaged: true,
        })
        .expect("open scanned sample");
    let session_id = opened.session_id.clone();
    assert!(opened.summary.page_count >= 1);

    // Page geometry and document hash from the live session (production
    // ocr_run_page reads the exact same fields).
    let (page_width, page_height, document_id) = {
        let arc = store
            .get_session_arc_pub(&session_id)
            .expect("session present");
        let session = arc.lock().expect("session lock");
        let page = &session.document.pages[0];
        (
            page.width_points,
            page.height_points,
            session.document.document_hash.clone(),
        )
    };

    // 2. Render page 0 at 200 DPI exactly like ocr_run_page does.
    let render_started = Instant::now();
    let render = store
        .render_page(RenderRequest {
            session_id: session_id.clone(),
            page_index: 0,
            zoom: 200.0 / 72.0,
            viewport: BBox {
                x: 0.0,
                y: 0.0,
                width: page_width,
                height: page_height,
            },
            device_pixel_ratio: 1.0,
        })
        .expect("render page at 200dpi");
    let render_ms = render_started.elapsed().as_millis();
    assert!(render.width_px > 0 && render.height_px > 0);

    // 3. Production image writer (alpha flattened over white).
    let image_path = std::env::temp_dir().join(format!(
        "r2h-ocr-e2e-{}-{}.ppm.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    save_rgba_as_png(
        &render.pixels_rgba,
        render.width_px,
        render.height_px,
        &image_path,
    )
    .expect("save page image");

    // 4. Run the STANDARD fast path end to end (real worker, real models).
    let ocr_state = OcrState::new();
    let context = OcrRunContext {
        operation_id: "ocr-fast-path-e2e".to_string(),
        document_id,
        session_id: session_id.clone(),
        page_index: 0,
        page_width_points: page_width,
        page_height_points: page_height,
        rotation_degrees: 0,
        language: "auto".to_string(),
        model_id: "PP-OCRv5".to_string(),
        timeout_secs: 600,
        cancellation_id: None,
    };
    let started = Instant::now();
    let result = {
        let mut engine = ocr_state.engine.lock().expect("ocr lock");
        engine
            .run_ocr_on_image_with_context(
                &image_path,
                render.width_px,
                render.height_px,
                true,
                context,
                Some(&ocr_state.cancellations),
            )
            .expect("standard OCR run must succeed")
    };
    let total_ms = started.elapsed().as_millis();

    // 5. Assert the real recognition outcome.
    assert_eq!(
        result.engine, "PP-OCRv5",
        "result must come from the fast engine"
    );
    assert_eq!(result.status, OcrResultStatus::Completed);
    assert!(
        result
            .text
            .replace(' ', "")
            .to_uppercase()
            .contains("R2HOCRTEST123"),
        "expected fixture text, got: {:?}",
        result.text
    );
    assert!(!result.blocks.is_empty(), "geometry must be present");
    let confidence = result.confidence.expect("aggregate confidence");
    assert!((0.0..=1.0).contains(&confidence));

    // 6. Every block bbox must convert into valid PDF points (acceptance
    //    prerequisite) — same converter the overlay command uses.
    for block in &result.blocks {
        let conversion = convert_ocr_bbox_to_pdf_points(
            [
                block.bbox.x,
                block.bbox.y,
                block.bbox.width,
                block.bbox.height,
            ],
            result.image_width_px,
            result.image_height_px,
            page_width,
            page_height,
        );
        assert!(
            !conversion.degenerate,
            "block {} produced a degenerate overlay rect: {:?}",
            block.id, conversion.bbox
        );
        assert_eq!(conversion.coordinate_space, "image_px");
    }

    // 7. The result must be cached for the standard engine (overlay
    //    acceptance reads it from there).
    {
        let engine = ocr_state.engine.lock().expect("ocr lock");
        let cached = engine.cache.get(&session_id, 0).expect("cached result");
        assert_eq!(cached.engine, "PP-OCRv5");
    }
    let non_empty = result
        .blocks
        .iter()
        .filter(|block| !block.text.trim().is_empty())
        .count();

    eprintln!("OCR_FAST_PATH_E2E render_ms={render_ms} ocr_total_ms={total_ms}");
    eprintln!(
        "OCR_FAST_PATH_E2E regions={non_empty} confidence={confidence:.4} image={}x{}",
        result.image_width_px, result.image_height_px
    );
    eprintln!("OCR_FAST_PATH_E2E text={:?}", result.text);

    let _ = std::fs::remove_file(&image_path);
}
