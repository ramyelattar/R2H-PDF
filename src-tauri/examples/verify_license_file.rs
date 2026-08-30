// Standalone license verifier for support diagnostics.
//
// Usage: cargo run --example verify_license_file -- <path-to-license.json>
//
// Verifies a license document against the issuer public key embedded in the
// application. Contains no signing capability by design.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(path) => path.clone(),
        None => {
            eprintln!("usage: verify_license_file <path-to-license.json>");
            std::process::exit(2);
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(raw) => match r2h_pdf_lib::license::verify_license_document(&raw) {
            Ok(payload) => {
                println!("VALID");
                println!("  license_id:   {}", payload.license_id);
                println!("  subject:      {}", payload.subject);
                println!("  issued_unix:  {}", payload.issued_unix);
                println!("  expires_unix: {}", payload.expires_unix);
                println!("  features:     {}", payload.features.join(", "));
            }
            Err(reason) => {
                println!("INVALID: {reason}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(2);
        }
    }
}
