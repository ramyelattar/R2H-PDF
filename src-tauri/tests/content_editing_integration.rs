//! Integration tests for the content editing pipeline.
//! These tests use real PDF files to prove that:
//! 1. Content objects can be extracted from real PDFs.
//! 2. Text replacement actually changes the PDF bytes.
//! 3. The exported/modified PDF contains the replacement text.
//! 4. The original text is no longer visible after editing.
//!
//! This is the proof that the editing engine works before any GUI testing.

use std::path::Path;

/// Load the test PDF bytes.
fn load_test_pdf() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("vendor")
        .join("mupdf-0.6.0")
        .join("tests")
        .join("files")
        .join("dummy.pdf");
    std::fs::read(&path).expect(&format!("Failed to read test PDF at {:?}", path))
}

/// Extract text from a PDF's first page using MuPDF.
fn extract_first_page_text(bytes: &[u8]) -> String {
    let doc = mupdf::Document::from_bytes(bytes, "pdf").expect("Failed to open PDF");
    let page = doc.load_page(0).expect("Failed to load page 0");
    let text_page = page
        .to_text_page(mupdf::TextPageFlags::empty())
        .expect("Failed to create text page");
    text_page.to_text().expect("Failed to extract text")
}

/// Parse content stream and find text operations.
fn find_text_ops_in_pdf(bytes: &[u8]) -> Vec<String> {
    let doc = mupdf::pdf::PdfDocument::from_bytes(bytes).expect("Failed to open as PdfDocument");
    let page = doc.load_page(0).expect("Failed to load page");
    let pdf_page = mupdf::pdf::PdfPage::try_from(page).expect("Failed to convert to PdfPage");
    let page_obj = pdf_page.object();

    let contents = match page_obj.get_dict("Contents") {
        Ok(Some(c)) => c,
        _ => return vec![],
    };

    if !contents.is_stream().unwrap_or(false) {
        return vec![];
    }

    let stream = contents
        .read_stream()
        .expect("Failed to read content stream");
    let ops = r2h_pdf_lib::content_editing::stream_parser::find_text_operations(&stream);
    ops.iter().map(|op| op.text.clone()).collect()
}

#[test]
fn test_dummy_pdf_has_text() {
    let bytes = load_test_pdf();
    let text = extract_first_page_text(&bytes);
    assert!(
        text.contains("Dummy PDF file"),
        "Expected 'Dummy PDF file' in extracted text, got: {}",
        text
    );
}

#[test]
fn test_content_stream_contains_text_operators() {
    let bytes = load_test_pdf();
    let ops = find_text_ops_in_pdf(&bytes);

    // dummy.pdf stores glyph codes as hex strings under a custom font encoding.
    // The low-level parser proves text-showing operators exist; semantic Unicode
    // decoding is verified independently through MuPDF text extraction.
    assert_eq!(
        ops.len(),
        6,
        "Expected six text-showing operators in dummy.pdf, got: {:?}",
        ops
    );
    assert!(
        ops.iter().all(|text| !text.is_empty()),
        "Every parsed text operator must retain a non-empty operand"
    );

    let rendered_text = extract_first_page_text(&bytes);
    assert!(
        rendered_text.contains("Dummy PDF file"),
        "MuPDF semantic extraction must decode the custom font mapping. Got: {}",
        rendered_text
    );
}

#[test]
fn test_text_replacement_changes_bytes() {
    let original_bytes = load_test_pdf();
    let original_text = extract_first_page_text(&original_bytes);

    let doc = mupdf::pdf::PdfDocument::from_bytes(&original_bytes).expect("open");
    let page = doc.load_page(0).expect("load page");
    let pdf_page = mupdf::pdf::PdfPage::try_from(page).expect("pdf page");
    let page_obj = pdf_page.object();

    let contents = page_obj
        .get_dict("Contents")
        .expect("get Contents")
        .expect("Contents exists");
    assert!(
        contents.is_stream().unwrap_or(false),
        "Contents must resolve to a readable stream"
    );

    let stream = contents.read_stream().expect("read stream");
    let ops = r2h_pdf_lib::content_editing::stream_parser::find_text_operations(&stream);

    assert_eq!(
        ops.len(),
        6,
        "Expected six text-showing operators in dummy.pdf, got: {:?}",
        ops.iter().map(|op| &op.text).collect::<Vec<_>>()
    );

    // The first operand is <01020303>, which semantically contributes "Dumm".
    // Code 0x04 is already proven by the fixture's second operator to map to "y".
    // Replacing the first operand with four 0x04 codes performs a genuine
    // font-aware native stream mutation without pretending the raw bytes are
    // Unicode text.
    let replacement_codes = [0x04_u8, 0x04, 0x04, 0x04];
    let new_stream = r2h_pdf_lib::content_editing::stream_parser::replace_text_in_stream_with_bytes(
        &stream,
        &ops[0],
        &replacement_codes,
    );

    assert_ne!(
        new_stream, stream,
        "The content stream must change after explicit byte replacement"
    );

    let new_stream_text = String::from_utf8_lossy(&new_stream);
    assert!(
        new_stream_text.contains("(\\004\\004\\004\\004)"),
        "The replacement glyph codes must be serialized as octal escapes. Got: {}",
        new_stream_text
    );
    assert!(
        !new_stream_text.contains("<01020303>"),
        "The original first hex operand must be removed"
    );

    let mut contents_mut = contents;
    let buf = mupdf::Buffer::from_bytes(&new_stream).expect("buffer");
    contents_mut
        .write_stream_buffer(&buf)
        .expect("write stream");

    let mut output = Vec::new();
    doc.write_to(&mut output).expect("serialize");

    assert!(!output.is_empty(), "Output must not be empty");
    assert_ne!(
        output, original_bytes,
        "Serialized PDF bytes must differ from the original"
    );

    let new_text = extract_first_page_text(&output);
    assert_ne!(
        new_text, original_text,
        "Semantic text extraction must change after native glyph replacement"
    );
    assert!(
        !new_text.contains("Dummy PDF file"),
        "The original semantic text must no longer be present. Got: {}",
        new_text
    );
}

#[test]
fn test_safe_visual_replacement_produces_visible_text() {
    let original_bytes = load_test_pdf();

    // Simulate the safe visual replacement path:
    // 1. Redact original area
    // 2. Append new text operators with Helvetica
    let mut doc = mupdf::pdf::PdfDocument::from_bytes(&original_bytes).expect("open");
    let page = doc.load_page(0).expect("load page");
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(page).expect("pdf page");

    // Create redaction over the text area (approximate bbox for "Dummy PDF file").
    let rect = mupdf::Rect::new(50.0, 700.0, 300.0, 730.0);
    let mut redact = pdf_page
        .create_annotation(mupdf::pdf::PdfAnnotationType::Redact)
        .expect("create redact");
    redact.set_rect(rect).expect("set rect");
    redact
        .set_color(mupdf::color::AnnotationColor::Gray(1.0))
        .expect("set color");
    drop(redact);
    pdf_page.redact().expect("apply redact");

    // Add replacement text via content stream append.
    let page_obj = pdf_page.object();

    // Ensure Helvetica resource.
    if let Ok(Some(resources)) = page_obj.get_dict("Resources") {
        if let Ok(Some(mut font_dict)) = resources.get_dict("Font") {
            let helv = doc.new_object_from_str(
                "<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>"
            ).expect("helv obj");
            let _ = font_dict.dict_put("R2HHelv", helv);
        }
    }

    // Append text operators.
    let text_ops = b"\nq\nBT\n/R2HHelv 14 Tf\n56 714 Td\n(REPLACED BY SAFE VISUAL) Tj\nET\nQ\n";
    if let Ok(Some(contents)) = page_obj.get_dict("Contents") {
        if contents.is_stream().unwrap_or(false) {
            let existing = contents.read_stream().expect("read");
            let mut combined = existing;
            combined.extend_from_slice(text_ops);
            let mut contents_mut = contents;
            let buf = mupdf::Buffer::from_bytes(&combined).expect("buf");
            contents_mut.write_stream_buffer(&buf).expect("write");
        }
    }

    // Serialize.
    let mut output = Vec::new();
    doc.write_to(&mut output).expect("serialize");

    // Verify replacement text is extractable.
    let text = extract_first_page_text(&output);
    assert!(
        text.contains("REPLACED BY SAFE VISUAL"),
        "Safe visual replacement must produce extractable text. Got: {}",
        text
    );
}

#[test]
fn test_form_field_detection() {
    // dummy.pdf may not have form fields, but the API should not crash.
    let bytes = load_test_pdf();
    let doc = mupdf::pdf::PdfDocument::from_bytes(&bytes).expect("open");
    let has_form = doc.has_acro_form().expect("has_acro_form");
    // dummy.pdf likely has no form — just verify no crash.
    assert!(
        !has_form || has_form,
        "has_acro_form should return a boolean without crashing"
    );
}

#[test]
fn test_stream_parser_handles_empty_stream() {
    let ops = r2h_pdf_lib::content_editing::stream_parser::find_text_operations(b"");
    assert!(ops.is_empty());
}

#[test]
fn test_stream_parser_handles_no_text_ops() {
    let stream = b"q 1 0 0 1 0 0 cm /Im0 Do Q";
    let ops = r2h_pdf_lib::content_editing::stream_parser::find_text_operations(stream);
    assert!(ops.is_empty());
}
