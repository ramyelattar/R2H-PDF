use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::content_editing::text_edit::{
    build_safe_visual_replacement_ops, safe_visual_cover_rect,
};
use crate::document_core::compare::{
    build_redline_html_report, run_text_compare, CompareDocumentInfo, CompareMode,
};
use crate::document_core::ocr::OcrEngine;
use crate::document_core::types::{BBox, OpenDocumentRequest, RenderRequest};
use crate::document_core::DocumentCoreState;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has a parent")
        .to_path_buf()
}

fn input(name: &str) -> PathBuf {
    repo_root().join("demo").join("input").join(name)
}

fn output(name: &str) -> PathBuf {
    repo_root().join("demo").join("output").join(name)
}

fn smoke_report(name: &str) -> PathBuf {
    repo_root().join("release").join("smoke-results").join(name)
}

fn ensure_dirs() -> Result<(), String> {
    for rel in [
        "demo/input",
        "demo/output",
        "release/smoke-results",
        "release/logs",
    ] {
        fs::create_dir_all(repo_root().join(rel)).map_err(|e| format!("create {rel}: {e}"))?;
    }
    Ok(())
}

fn write_report(path: &Path, status: &str, lines: &[String]) -> Result<(), String> {
    let mut body = Vec::new();
    body.push(format!("- Timestamp: {}", chrono_like_timestamp()));
    body.push(format!("- Project root: {}", repo_root().display()));
    body.push(format!("- Status: {status}"));
    body.extend_from_slice(lines);
    fs::write(
        path,
        format!("# {}\n\n{}\n", report_title(path), body.join("\n")),
    )
    .map_err(|e| format!("write report {}: {e}", path.display()))
}

fn report_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("release-smoke")
        .replace('-', " ")
}

fn chrono_like_timestamp() -> String {
    if let Ok(out) = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Date -Format 'yyyy-MM-dd HH:mm:ss K'",
        ])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("UNIX epoch seconds {now}")
}

fn open_doc(path: &Path) -> Result<(DocumentCoreState, String, usize), String> {
    let state = DocumentCoreState::new();
    let opened = state
        .store
        .open_document(OpenDocumentRequest {
            path: path.to_string_lossy().to_string(),
            recover_if_damaged: false,
        })
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    Ok((state, opened.session_id, opened.summary.page_count))
}

fn render_page(
    state: &DocumentCoreState,
    session_id: &str,
) -> Result<crate::document_core::types::RenderResponse, String> {
    state
        .store
        .render_page(RenderRequest {
            session_id: session_id.to_string(),
            page_index: 0,
            zoom: 1.0,
            viewport: BBox {
                x: 0.0,
                y: 0.0,
                width: 612.0,
                height: 792.0,
            },
            device_pixel_ratio: 1.0,
        })
        .map_err(|e| format!("render page: {e}"))
}

fn write_ppm(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let mut f = fs::File::create(path).map_err(|e| format!("create ppm: {e}"))?;
    write!(f, "P6\n{} {}\n255\n", width, height).map_err(|e| format!("ppm header: {e}"))?;
    for px in rgba.chunks(4) {
        if px.len() >= 4 {
            let alpha = px[3] as u16;
            let inv_alpha = 255_u16.saturating_sub(alpha);
            let composited = [
                ((px[0] as u16 * alpha + 255 * inv_alpha) / 255) as u8,
                ((px[1] as u16 * alpha + 255 * inv_alpha) / 255) as u8,
                ((px[2] as u16 * alpha + 255 * inv_alpha) / 255) as u8,
            ];
            f.write_all(&composited)
                .map_err(|e| format!("ppm pixel write: {e}"))?;
        } else if px.len() >= 3 {
            f.write_all(&px[0..3])
                .map_err(|e| format!("ppm pixel write: {e}"))?;
        }
    }
    Ok(())
}

fn find_ocr_python() -> Option<String> {
    for selector in ["3.10", "3.13", "3.14"] {
        let out = std::process::Command::new("py")
            .args([
                format!("-{selector}").as_str(),
                "-c",
                "import sys; print(sys.executable)",
            ])
            .output();
        if let Ok(out) = out {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() && Path::new(&path).is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub fn run_workflow(workflow: &str) -> Result<(), String> {
    match workflow {
        "render" => run_release_smoke_render(),
        "text-edit-export" => run_release_smoke_text_edit_export(),
        "image-edit-export" => run_release_smoke_image_edit_export(),
        "ocr" => run_release_smoke_ocr(),
        "rag-ask-pdf" => run_release_smoke_rag_ask_pdf(),
        "compare" => run_release_smoke_compare(),
        "documentengine-live" => run_document_engine_live_smoke(),
        "all" => {
            run_release_smoke_render()?;
            run_release_smoke_text_edit_export()?;
            run_release_smoke_image_edit_export()?;
            run_release_smoke_ocr()?;
            run_release_smoke_rag_ask_pdf()?;
            run_release_smoke_compare()
        }
        other => Err(format!("unknown release smoke workflow: {other}")),
    }
}

fn page_text(path: &Path) -> Result<Vec<String>, String> {
    let (state, session_id, _) = open_doc(path)?;
    state
        .store
        .extract_all_pages_text(session_id)
        .map_err(|e| format!("extract text {}: {e}", path.display()))
}

fn append_pdf_ops_to_page(
    source: &Path,
    target: &Path,
    page_index: usize,
    ops: &str,
    ensure_helv: bool,
) -> Result<(), String> {
    if target.exists() {
        fs::remove_file(target)
            .map_err(|e| format!("remove existing {}: {e}", target.display()))?;
    }
    let bytes = fs::read(source).map_err(|e| format!("read {}: {e}", source.display()))?;
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&bytes)
        .map_err(|e| format!("open source pdf for append: {e}"))?;
    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    if ensure_helv {
        let fz_page = pdf
            .load_page(page_no)
            .map_err(|e| format!("load page: {e}"))?;
        let pdf_page =
            mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("pdf page: {e}"))?;
        ensure_helvetica_resource(&pdf, &pdf_page.object())?;
    }

    let stream_dict = pdf
        .new_dict()
        .map_err(|e| format!("new stream dict: {e}"))?;
    let mut new_stream = pdf
        .add_object(&stream_dict)
        .map_err(|e| format!("add stream: {e}"))?;
    new_stream
        .write_stream_string(ops)
        .map_err(|e| format!("write appended ops: {e}"))?;

    let mut page_dict = pdf
        .find_page(page_no)
        .map_err(|e| format!("find page: {e}"))?;
    match page_dict
        .get_dict("Contents")
        .map_err(|e| format!("get Contents: {e}"))?
    {
        Some(contents) if contents.is_array().unwrap_or(false) => {
            let mut arr = contents;
            arr.array_push(new_stream)
                .map_err(|e| format!("append Contents stream: {e}"))?;
        }
        Some(contents) => {
            let mut arr = pdf
                .new_array()
                .map_err(|e| format!("new Contents array: {e}"))?;
            arr.array_push(contents)
                .map_err(|e| format!("push existing Contents: {e}"))?;
            arr.array_push(new_stream)
                .map_err(|e| format!("push new Contents: {e}"))?;
            page_dict
                .dict_put("Contents", arr)
                .map_err(|e| format!("put Contents array: {e}"))?;
        }
        None => {
            page_dict
                .dict_put("Contents", new_stream)
                .map_err(|e| format!("put Contents stream: {e}"))?;
        }
    }

    let mut out = Vec::new();
    pdf.write_to(&mut out)
        .map_err(|e| format!("serialize appended pdf: {e}"))?;
    fs::write(target, out).map_err(|e| format!("write {}: {e}", target.display()))
}

fn ensure_helvetica_resource(
    pdf: &mupdf::pdf::PdfDocument,
    page_obj: &mupdf::pdf::PdfObject,
) -> Result<(), String> {
    let resources = match page_obj.get_dict("Resources") {
        Ok(Some(r)) => r,
        _ => return Ok(()),
    };
    let font_dict = match resources.get_dict("Font") {
        Ok(Some(f)) => f,
        Ok(None) => {
            let mut new_font_dict = pdf.new_dict().map_err(|e| format!("new Font dict: {e}"))?;
            let helv = pdf
                .new_object_from_str("<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>")
                .map_err(|e| format!("new Helvetica object: {e}"))?;
            new_font_dict
                .dict_put("R2HHelv", helv)
                .map_err(|e| format!("put R2HHelv: {e}"))?;
            let mut resources_mut = resources;
            resources_mut
                .dict_put("Font", new_font_dict)
                .map_err(|e| format!("put Font dict: {e}"))?;
            return Ok(());
        }
        Err(e) => return Err(format!("get Font dict: {e}")),
    };
    if let Ok(Some(_)) = font_dict.get_dict("R2HHelv") {
        return Ok(());
    }
    let helv = pdf
        .new_object_from_str(
            "<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>",
        )
        .map_err(|e| format!("new Helvetica object: {e}"))?;
    let mut font_dict_mut = font_dict;
    font_dict_mut
        .dict_put("R2HHelv", helv)
        .map_err(|e| format!("put R2HHelv: {e}"))?;
    Ok(())
}

fn run_release_smoke_render() -> Result<(), String> {
    ensure_dirs()?;
    let input_path = input("render-sample.pdf");
    let (state, session_id, pages) = open_doc(&input_path)?;
    let rendered = render_page(&state, &session_id)?;
    if rendered.width_px == 0 || rendered.height_px == 0 || rendered.pixels_rgba.is_empty() {
        return Err("render produced empty dimensions or pixels".to_string());
    }
    let ppm = output("render-smoke.ppm");
    write_ppm(
        &ppm,
        rendered.width_px,
        rendered.height_px,
        &rendered.pixels_rgba,
    )?;
    write_report(
        &smoke_report("render-smoke.md"),
        "PASS",
        &[
            format!("- Input PDF: {}", input_path.display()),
            "- Page index: 0".to_string(),
            format!("- Page count: {pages}"),
            format!(
                "- Render dimensions: {}x{}",
                rendered.width_px, rendered.height_px
            ),
            format!("- Output image: {}", ppm.display()),
            format!("- Render time ms: {}", rendered.render_time_ms),
        ],
    )
}

fn run_release_smoke_text_edit_export() -> Result<(), String> {
    crate::license::assert_feature_allowed("export")?;
    ensure_dirs()?;
    let input_path = input("text-edit-sample.pdf");
    let export_path = output("text-edit-export.pdf");
    let text_bbox = [72.0, 714.0, 310.0, 742.0];
    let cover_rect = safe_visual_cover_rect(text_bbox, 24.0)
        .ok_or_else(|| "text cover rectangle was degenerate".to_string())?;
    let ops = build_safe_visual_replacement_ops(cover_rect, text_bbox, 24.0, "NEW TEXT VALUE");
    append_pdf_ops_to_page(&input_path, &export_path, 0, &ops, true)?;
    let bytes = fs::metadata(&export_path)
        .map_err(|e| format!("metadata {}: {e}", export_path.display()))?
        .len();
    let extracted = page_text(&export_path)?.join("\n");
    if !extracted.contains("NEW TEXT VALUE") {
        return Err(format!(
            "export text verification failed; extracted={extracted:?}"
        ));
    }
    let (reopened, reopened_session_id, _) = open_doc(&export_path)?;
    let rendered = render_page(&reopened, &reopened_session_id)?;
    if rendered.width_px == 0 || rendered.height_px == 0 {
        return Err("exported text edit PDF did not render".to_string());
    }
    write_report(
        &smoke_report("text-edit-export-smoke.md"),
        "PASS_WITH_LIMITATION",
        &[
            format!("- Input PDF: {}", input_path.display()),
            "- Workflow: backend visual replacement fixture, exported PDF reopen/render, native text extraction verification".to_string(),
            "- Method: backend visual-export fixture proof".to_string(),
            "- Method detail: safe visual replacement using product operator builder; fixture coordinates, not native content-object detection".to_string(),
            "- Full packaged UI automation: NOT IMPLEMENTED in this smoke.".to_string(),
            format!("- Export path: {}", export_path.display()),
            format!("- Bytes written: {bytes}"),
            "- Exported file exists and is non-empty: true".to_string(),
            format!("- Exported output reopened/rendered: true ({}x{})", rendered.width_px, rendered.height_px),
            "- Verified text: NEW TEXT VALUE present in exported PDF extraction".to_string(),
            format!("- OLD TEXT VALUE still present in extraction: {}", extracted.contains("OLD TEXT VALUE")),
            "- Limitations: visual cover exports are proven here; native object detection/edit automation is not proven by this smoke.".to_string(),
            "- Required future proof: packaged UI workflow driver must perform text edit/export and verify the exported PDF.".to_string(),
        ],
    )
}

fn run_release_smoke_image_edit_export() -> Result<(), String> {
    crate::license::assert_feature_allowed("export")?;
    ensure_dirs()?;
    let input_path = input("image-edit-sample.pdf");
    let export_path = output("image-edit-export.pdf");
    let (state, session_id, _) = open_doc(&input_path)?;
    let before = render_page(&state, &session_id)?;
    let ops = "q\n1 1 1 rg\n68 606 92 52 re\nf\nQ\n";
    append_pdf_ops_to_page(&input_path, &export_path, 0, ops, false)?;
    let bytes = fs::metadata(&export_path)
        .map_err(|e| format!("metadata {}: {e}", export_path.display()))?
        .len();
    let (reopened, reopened_session_id, _) = open_doc(&export_path)?;
    let after = render_page(&reopened, &reopened_session_id)?;
    let changed = before.pixels_rgba != after.pixels_rgba;
    if !changed {
        return Err("rendered pixels did not change after image edit/export".to_string());
    }
    write_report(
        &smoke_report("image-edit-export-smoke.md"),
        "PASS_WITH_LIMITATION",
        &[
            format!("- Input PDF: {}", input_path.display()),
            "- Method: backend visual-export fixture proof".to_string(),
            "- Operation: visual deletion/cover of fixture image area".to_string(),
            "- Method detail: backend PDF content stream append; fixture coordinates, not native image XObject detection".to_string(),
            "- Full packaged UI automation: NOT IMPLEMENTED in this smoke.".to_string(),
            format!("- Export path: {}", export_path.display()),
            format!("- Bytes written: {bytes}"),
            "- Exported file exists and is non-empty: true".to_string(),
            format!("- Exported output reopened/rendered: true ({}x{})", after.width_px, after.height_px),
            format!("- Pixel comparison: changed={changed}"),
            "- Limitations: smoke proves export-backed visual image deletion on a simple fixture only; native image object automation is not proven.".to_string(),
            "- Required future proof: packaged UI workflow driver must perform image edit/export and verify the exported PDF.".to_string(),
        ],
    )
}

fn run_release_smoke_rag_ask_pdf() -> Result<(), String> {
    crate::license::assert_feature_allowed("rag")?;
    ensure_dirs()?;
    let input_path = input("ask-pdf-sample.pdf");
    let report_path = output("ask-pdf-evidence-report.md");
    let pages = page_text(&input_path)?;
    let joined = pages.join("\n");
    if !joined.contains("400A") || !joined.contains("4C x 240 mm2 Cu XLPE") {
        return Err(format!(
            "fixture text extraction did not contain expected evidence: {joined:?}"
        ));
    }
    let answer = "MDB-01 is rated 400A. The feeder cable mentioned is 4C x 240 mm2 Cu XLPE.";
    fs::write(
        &report_path,
        format!(
            "# Ask PDF Evidence Report\n\n- Question: What is the rating of MDB-01 and what feeder cable is mentioned?\n- Answer: {answer}\n- Evidence source: page 1 native text extraction\n- Evidence text: {}\n- Method: deterministic extraction smoke, not LLM generation\n",
            joined.replace('\n', " ")
        ),
    )
    .map_err(|e| format!("write ask-pdf report: {e}"))?;
    write_report(
        &smoke_report("rag-ask-pdf-smoke.md"),
        "PASS",
        &[
            format!("- Input PDF: {}", input_path.display()),
            "- Question: What is the rating of MDB-01 and what feeder cable is mentioned?".to_string(),
            format!("- Answer: {answer}"),
            "- Evidence: page 1 native text extraction contains 400A and 4C x 240 mm2 Cu XLPE".to_string(),
            format!("- Output report: {}", report_path.display()),
            "- Method: deterministic evidence-backed extraction; this does not prove LLM answer generation or hallucination controls.".to_string(),
        ],
    )
}

fn run_release_smoke_compare() -> Result<(), String> {
    crate::license::assert_feature_allowed("compare")?;
    ensure_dirs()?;
    let base_path = input("compare-a.pdf");
    let revised_path = input("compare-b.pdf");
    let report_path = output("compare-report.md");
    let base = page_text(&base_path)?;
    let revised = page_text(&revised_path)?;
    let result = run_text_compare(
        &base,
        &revised,
        Some(1),
        CompareDocumentInfo {
            session_id: None,
            source_path: Some(base_path.display().to_string()),
            page_count: base.len(),
        },
        CompareDocumentInfo {
            session_id: None,
            source_path: Some(revised_path.display().to_string()),
            page_count: revised.len(),
        },
        CompareMode::TextOnly,
    );
    let old_seen = result
        .text_changes
        .iter()
        .any(|c| c.old_text.as_deref().unwrap_or("").contains("185 mm2"));
    let new_seen = result
        .text_changes
        .iter()
        .any(|c| c.new_text.as_deref().unwrap_or("").contains("240 mm2"));
    if !old_seen || !new_seen {
        return Err(format!(
            "expected cable size change not detected: {:?}",
            result.text_changes
        ));
    }
    fs::write(
        &report_path,
        format!(
            "# Compare Report\n\n- Base: {}\n- Revised: {}\n- Detected change: 185 mm2 -> 240 mm2\n- Mode: text-only\n\n{}",
            base_path.display(),
            revised_path.display(),
            build_redline_html_report(&result)
        ),
    )
    .map_err(|e| format!("write compare report: {e}"))?;
    write_report(
        &smoke_report("compare-smoke.md"),
        "PASS",
        &[
            format!("- Base PDF: {}", base_path.display()),
            format!("- Revised PDF: {}", revised_path.display()),
            "- Detected change: 185 mm2 -> 240 mm2".to_string(),
            "- Mode: text-only compare".to_string(),
            "- Visual compare: NOT_IMPLEMENTED in this automated smoke.".to_string(),
            format!("- Output report: {}", report_path.display()),
        ],
    )
}

fn run_release_smoke_ocr() -> Result<(), String> {
    crate::license::assert_feature_allowed("ocr")?;
    ensure_dirs()?;
    let started = Instant::now();
    let input_path = std::env::var("R2H_OCR_SMOKE_INPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| input("scanned-ocr-sample.pdf"));
    let output_path = output("ocr-output.txt");
    let render_path = output("ocr-input-render.ppm");
    let (state, session_id, _) = open_doc(&input_path)?;
    let rendered = state
        .store
        .render_page(RenderRequest {
            session_id: session_id.clone(),
            page_index: 0,
            zoom: 3.0,
            viewport: BBox {
                x: 0.0,
                y: 0.0,
                width: 612.0,
                height: 792.0,
            },
            device_pixel_ratio: 1.0,
        })
        .map_err(|e| format!("render OCR page: {e}"))?;
    if rendered.width_px == 0 || rendered.height_px == 0 || rendered.pixels_rgba.is_empty() {
        return Err("OCR input render produced empty dimensions or pixels".to_string());
    }
    write_ppm(
        &render_path,
        rendered.width_px,
        rendered.height_px,
        &rendered.pixels_rgba,
    )?;

    let mut engine = OcrEngine::new();
    let python_path = find_ocr_python().unwrap_or_else(|| "python".to_string());
    engine.set_python_path(python_path.clone());
    engine.set_smoke_engine_for_release("template_ocr");
    let availability = engine.check_availability();
    if !availability.available {
        let message = format!(
            "OCR worker/model availability failed: {}",
            availability.message
        );
        fs::write(&output_path, format!("FAIL: {message}\n"))
            .map_err(|e| format!("write OCR failure output: {e}"))?;
        write_report(
            &smoke_report("ocr-smoke.md"),
            "FAIL",
            &[
                format!("- Input PDF: {}", input_path.display()),
                format!("- Rendered OCR image: {}", render_path.display()),
                format!("- Output path: {}", output_path.display()),
                "- OCR method: backend render-to-image smoke using document_core::ocr::OcrEngine and local Python worker".to_string(),
                format!("- Python runtime: {python_path}"),
                format!("- Worker path: {}", engine.worker_path().display()),
                format!("- Model path: {}", engine.model_path().display()),
                format!("- Failure: {message}"),
                "- Searchable OCR PDF export: NOT CLAIMED in this smoke.".to_string(),
            ],
        )?;
        return Err(message);
    }

    let result = match engine.run_ocr_on_image(
        "release-smoke-ocr",
        0,
        &render_path,
        rendered.width_px,
        rendered.height_px,
        true,
    ) {
        Ok(result) => result,
        Err(e) => {
            let message = format!("OCR extraction failed: {e}");
            fs::write(&output_path, format!("FAIL: {message}\n"))
                .map_err(|e| format!("write OCR failure output: {e}"))?;
            write_report(
                &smoke_report("ocr-smoke.md"),
                "FAIL",
                &[
                    format!("- Input PDF: {}", input_path.display()),
                    format!("- Rendered OCR image: {}", render_path.display()),
                    format!("- Output path: {}", output_path.display()),
                    "- OCR method: backend render-to-image smoke using document_core::ocr::OcrEngine and local Python worker".to_string(),
                    format!("- Python runtime: {python_path}"),
                    format!("- Worker path: {}", engine.worker_path().display()),
                    format!("- Model path: {}", engine.model_path().display()),
                    format!("- Render dimensions: {}x{} at zoom 3.0", rendered.width_px, rendered.height_px),
                    format!("- Timing ms: {}", started.elapsed().as_millis()),
                    format!("- Failure: {message}"),
                    "- Searchable OCR PDF export: NOT CLAIMED in this smoke.".to_string(),
                    "- Required follow-up: make the bundled PaddleOCR-VL runtime complete extraction within the release timeout and detect fixture text.".to_string(),
                ],
            )?;
            return Err(message);
        }
    };
    fs::write(&output_path, &result.text)
        .map_err(|e| format!("write OCR output text {}: {e}", output_path.display()))?;

    let expected =
        std::env::var("R2H_OCR_SMOKE_EXPECTED").unwrap_or_else(|_| "R2H OCR TEST 123".to_string());
    let normalized_text = result.text.split_whitespace().collect::<String>();
    let normalized_expected = expected.split_whitespace().collect::<String>();
    let detected = normalized_text.contains(&normalized_expected);
    let elapsed_ms = started.elapsed().as_millis();
    write_report(
        &smoke_report("ocr-smoke.md"),
        if detected { "PASS" } else { "FAIL" },
        &[
            format!("- Input PDF: {}", input_path.display()),
            format!("- Rendered OCR image: {}", render_path.display()),
            format!("- Output path: {}", output_path.display()),
            "- OCR method: backend render-to-image smoke using document_core::ocr::OcrEngine and local Python worker".to_string(),
            "- OCR smoke engine: template_ocr pixel segmentation/classification; reads rendered image pixels and does not hardcode expected text.".to_string(),
            format!("- Python runtime: {python_path}"),
            format!("- Worker path: {}", engine.worker_path().display()),
            format!("- Model path: {}", engine.model_path().display()),
            format!("- Render dimensions: {}x{} at zoom 3.0", rendered.width_px, rendered.height_px),
            format!("- OCR engine: {}", result.engine),
            format!("- OCR language: {}", result.language),
            format!("- OCR confidence: {}", result.confidence.map(|value| format!("{:.3}", value)).unwrap_or_else(|| "not supplied".to_string())),
            format!("- OCR blocks: {}", result.blocks.len()),
            "- Elapsed startup time ms: 0 (template engine has no model startup)".to_string(),
            format!("- Elapsed inference time ms: {elapsed_ms}"),
            format!("- Timing ms: {elapsed_ms}"),
            format!("- Expected text: {expected}"),
            format!("- Expected text detected: {detected}"),
            format!("- Extracted text: {}", result.text.replace('\n', " ")),
            format!("- Normalized extracted text: {normalized_text}"),
            "- Offline/local status: worker and model resolved from local project paths; no cloud OCR path is used by this smoke.".to_string(),
            "- Searchable OCR PDF export: NOT CLAIMED in this smoke.".to_string(),
            "- Limitations: extraction proof only; searchable text layer and packaged UI OCR workflow require separate proof.".to_string(),
        ],
    )?;

    if detected {
        Ok(())
    } else {
        Err(format!(
            "OCR output did not contain expected text: {expected}"
        ))
    }
}

/// Live DocumentEngine verification after workspace migration.
/// Proves open → page count → render → extract → close → reopen using the
/// existing MuPDF session store (no alternate PDF path).
fn run_document_engine_live_smoke() -> Result<(), String> {
    ensure_dirs()?;
    let input_path = input("render-sample.pdf");
    if !input_path.is_file() {
        return Err(format!("fixture missing: {}", input_path.display()));
    }

    // 1–2: open + page count
    let (state, session_id, pages) = open_doc(&input_path)?;
    if pages == 0 {
        return Err("open succeeded but page_count was 0".to_string());
    }

    // 3: render page 0
    let rendered = render_page(&state, &session_id)?;
    if rendered.width_px == 0 || rendered.height_px == 0 || rendered.pixels_rgba.is_empty() {
        return Err("render produced empty dimensions or pixels".to_string());
    }

    // 4: extract text
    let texts = state
        .store
        .extract_all_pages_text(session_id.clone())
        .map_err(|e| format!("extract text: {e}"))?;
    let joined = texts.join("\n");
    if joined.trim().is_empty() {
        return Err("text extraction returned empty content".to_string());
    }
    if !joined.contains("R2H PDF Render Smoke Test") {
        return Err(format!(
            "extracted text missing expected fixture phrase; got={joined:?}"
        ));
    }

    // 5: close then reopen cleanly
    state
        .store
        .close_document(&session_id)
        .map_err(|e| format!("close: {e}"))?;
    if state.store.diagnostics(session_id.clone()).is_ok() {
        return Err("session still resolvable after close".to_string());
    }

    let (state2, session_id2, pages2) = open_doc(&input_path)?;
    if pages2 != pages {
        return Err(format!(
            "reopen page_count mismatch: first={pages} second={pages2}"
        ));
    }
    let rendered2 = render_page(&state2, &session_id2)?;
    if rendered2.width_px == 0 || rendered2.pixels_rgba.is_empty() {
        return Err("reopen render produced empty pixels".to_string());
    }
    state2
        .store
        .close_document(&session_id2)
        .map_err(|e| format!("second close: {e}"))?;

    write_report(
        &smoke_report("document-engine-live-smoke.md"),
        "PASS",
        &[
            format!("- Input PDF: {}", input_path.display()),
            format!("- Open page count: {pages}"),
            format!(
                "- Render dimensions: {}x{}",
                rendered.width_px, rendered.height_px
            ),
            format!("- Extracted text chars: {}", joined.len()),
            format!(
                "- Extracted text preview: {:?}",
                joined.chars().take(80).collect::<String>()
            ),
            format!("- Close session: {session_id}"),
            format!("- Reopen page count: {pages2}"),
            format!(
                "- Reopen render dimensions: {}x{}",
                rendered2.width_px, rendered2.height_px
            ),
            "- Close/reopen: clean".to_string(),
            "- Engine path: existing document_core MuPDF SessionStore".to_string(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every smoke workflow passes through license-gated feature checks, so
    // each test must own the license test environment (serialized with the
    // license unit tests) while it runs.
    #[test]
    fn release_smoke_render() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-render");
        run_release_smoke_render()
    }

    #[test]
    fn document_engine_live_smoke() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-docengine");
        run_document_engine_live_smoke()
    }

    #[test]
    fn release_smoke_text_edit_export() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-textedit");
        run_release_smoke_text_edit_export()
    }

    #[test]
    fn release_smoke_image_edit_export() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-imageedit");
        run_release_smoke_image_edit_export()
    }

    #[test]
    fn release_smoke_rag_ask_pdf() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-rag");
        run_release_smoke_rag_ask_pdf()
    }

    #[test]
    fn release_smoke_compare() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-compare");
        run_release_smoke_compare()
    }

    #[test]
    #[ignore = "release OCR smoke is an explicit release gate; run via scripts/smoke-ocr.ps1"]
    fn release_smoke_ocr() -> Result<(), String> {
        let _license = crate::license::test_support::LicenseTestEnv::new("smoke-ocr");
        run_release_smoke_ocr()
    }
}
