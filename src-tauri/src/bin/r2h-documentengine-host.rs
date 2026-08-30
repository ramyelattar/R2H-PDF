fn main() {
    if let Err(error) = r2h_pdf_lib::document_engine_host::run_stdio() {
        eprintln!("documentengine host stdin/stdout failure: {}", error.kind());
        std::process::exit(1);
    }
}
