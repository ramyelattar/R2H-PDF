//! Debug test to inspect the actual content stream of dummy.pdf

use std::path::Path;

#[test]
fn debug_content_stream() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("vendor")
        .join("mupdf-0.6.0")
        .join("tests")
        .join("files")
        .join("dummy.pdf");
    let bytes = std::fs::read(&path).expect("read");

    let doc = mupdf::pdf::PdfDocument::from_bytes(&bytes).expect("open");
    let page = doc.load_page(0).expect("load page");
    let pdf_page = mupdf::pdf::PdfPage::try_from(page).expect("pdf page");
    let page_obj = pdf_page.object();

    // Check what Contents looks like.
    match page_obj.get_dict("Contents") {
        Ok(Some(contents)) => {
            let is_stream = contents.is_stream().unwrap_or(false);
            let is_array = contents.is_array().unwrap_or(false);
            let is_indirect = contents.is_indirect().unwrap_or(false);
            eprintln!(
                "Contents: is_stream={}, is_array={}, is_indirect={}",
                is_stream, is_array, is_indirect
            );
            eprintln!("Contents object: {}", contents);

            if is_stream {
                match contents.read_stream() {
                    Ok(stream) => {
                        eprintln!("Stream length: {} bytes", stream.len());
                        eprintln!(
                            "Stream hex (first 200): {:?}",
                            &stream[..stream.len().min(200)]
                        );
                        let preview = String::from_utf8_lossy(&stream);
                        eprintln!("Stream as text:\n---\n{}\n---", preview);
                    }
                    Err(e) => eprintln!("Failed to read stream: {}", e),
                }
            } else if is_indirect {
                // Try resolving the indirect reference.
                match contents.resolve() {
                    Ok(Some(resolved)) => {
                        let is_stream2 = resolved.is_stream().unwrap_or(false);
                        let is_array2 = resolved.is_array().unwrap_or(false);
                        eprintln!("Resolved: is_stream={}, is_array={}", is_stream2, is_array2);
                        if is_stream2 {
                            match resolved.read_stream() {
                                Ok(stream) => {
                                    let preview =
                                        String::from_utf8_lossy(&stream[..stream.len().min(500)]);
                                    eprintln!("Resolved stream (first 500 bytes):\n{}", preview);
                                }
                                Err(e) => eprintln!("Failed to read resolved stream: {}", e),
                            }
                        }
                    }
                    Ok(None) => eprintln!("Resolved to None"),
                    Err(e) => eprintln!("Resolve error: {}", e),
                }
            }
        }
        Ok(None) => eprintln!("No Contents dict entry"),
        Err(e) => eprintln!("Error getting Contents: {}", e),
    }
}
