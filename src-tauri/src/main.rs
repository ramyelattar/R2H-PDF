// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--release-smoke") {
        let workflow = args.get(2).map(String::as_str).unwrap_or("all");
        std::process::exit(r2h_pdf_lib::run_release_smoke_cli(workflow));
    }
    if args.get(1).map(String::as_str) == Some("--license-smoke") {
        let mode = args.get(2).map(String::as_str).unwrap_or("offline");
        std::process::exit(r2h_pdf_lib::run_license_smoke_cli(mode));
    }
    r2h_pdf_lib::run()
}
