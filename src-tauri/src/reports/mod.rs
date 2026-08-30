#![allow(dead_code)]

pub mod html_report;
pub mod types;

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use types::*;
use html_report::generate_html_report;

// ---------------------------------------------------------------------------
// Report Builder
// ---------------------------------------------------------------------------

pub fn build_report_bundle(
    request: &ReportExportRequest,
    review_result: Option<&crate::ai_core::review::DocumentReviewResult>,
    eng_findings: &[crate::engineering::types::EngineeringFinding],
    eng_traces: &[crate::engineering::types::CalculationTrace],
) -> Result<ReportBundle, String> {
    if review_result.is_none() && eng_findings.is_empty() {
        return Err("No review or engineering findings are available to export.".to_string());
    }

    let warnings = Vec::new();
    let mut all_citations: Vec<ReportCitation> = Vec::new();

    // Review data.
    let (summary, findings, risks, missing_info) = if let Some(rev) = review_result {
        let findings = rev.findings.iter().map(|f| ReportFinding {
            id: f.finding_id.clone(), severity: f.severity.clone(), category: f.category.clone(),
            title: f.title.clone(), description: f.description.clone(),
            recommendation: f.recommendation.clone(), page_refs: f.page_refs.clone(), confidence: f.confidence,
        }).collect();
        let risks = rev.risks.iter().map(|f| ReportFinding {
            id: f.finding_id.clone(), severity: f.severity.clone(), category: "risk".into(),
            title: f.title.clone(), description: f.description.clone(),
            recommendation: f.recommendation.clone(), page_refs: f.page_refs.clone(), confidence: f.confidence,
        }).collect();
        let missing = rev.missing_information.iter().map(|f| ReportFinding {
            id: f.finding_id.clone(), severity: f.severity.clone(), category: "missing_information".into(),
            title: f.title.clone(), description: f.description.clone(),
            recommendation: f.recommendation.clone(), page_refs: f.page_refs.clone(), confidence: f.confidence,
        }).collect();

        for c in &rev.citations {
            all_citations.push(ReportCitation { id: c.citation_id.clone(), page: c.page_index, source: c.source.clone(), snippet: c.snippet.clone(), score: c.score });
        }

        (rev.summary.clone(), findings, risks, missing)
    } else {
        (String::new(), Vec::new(), Vec::new(), Vec::new())
    };

    // Engineering findings.
    let eng = eng_findings.iter().map(|f| ReportFinding {
        id: f.finding_id.clone(), severity: match f.severity { crate::engineering::types::FindingSeverity::Info => "info", crate::engineering::types::FindingSeverity::Warning => "warning", crate::engineering::types::FindingSeverity::Major => "major", crate::engineering::types::FindingSeverity::Critical => "critical" }.into(),
        category: "engineering".into(), title: f.title.clone(), description: f.description.clone(),
        recommendation: f.recommendation.clone(), page_refs: f.page_refs.clone(), confidence: f.confidence,
    }).collect();

    // Traces.
    let traces: Vec<ReportTrace> = eng_traces.iter().map(|t| {
        let inputs: Vec<String> = t.steps.iter().flat_map(|s| s.inputs.iter().map(|i| format!("{}: {} {:?} ({})", i.label, i.value, i.unit, i.source_citation.as_deref().unwrap_or("")))).collect();
        let formula = t.steps.first().map(|s| s.formula.clone()).unwrap_or_default();
        ReportTrace { id: t.calculation_id.clone(), title: t.title.clone(), calculation_type: t.calculation_type.clone(), formula, inputs, final_value: format!("{:.2} {:?}", t.final_value, t.final_unit), warnings: t.warnings.clone() }
    }).collect();

    // Deduplicate citations.
    let mut seen_cites = std::collections::HashSet::new();
    all_citations.retain(|c| seen_cites.insert(c.id.clone()));
    all_citations.sort_by_key(|c| c.page);

    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let generated_at = format!("{}", chrono_lite(now));

    Ok(ReportBundle {
        session_id: request.session_id.clone(),
        document_path: request.output_path.clone(),
        report_title: request.report_title.clone(),
        review_summary: summary,
        findings, risks, missing_information: missing_info,
        engineering_findings: eng,
        calculation_traces: traces,
        ai_actions: Vec::new(),
        audit_entries: Vec::new(),
        citations: all_citations,
        warnings,
        metadata: ReportMetadata { generated_at, page_count: 0, retrieval_mode: "hybrid".into(), ocr_included: false },
    })
}

fn chrono_lite(epoch_secs: u64) -> String {
    // Simple date formatting without chrono crate.
    let s = epoch_secs % 60; let m = (epoch_secs / 60) % 60; let h = (epoch_secs / 3600) % 24;
    let days = epoch_secs / 86400;
    let mut y = 1970u32; let mut rem = days;
    loop { let dy = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 }; if rem < dy { break; } rem -= dy; y += 1; }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let md = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32; for &d in &md { if rem < d { break; } rem -= d; mo += 1; }
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, rem + 1, h, m, s)
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn report_export_review(
    eng_state: State<'_, crate::engineering::EngineeringState>,
    request: ReportExportRequest,
) -> Result<ReportExportResult, String> {
    let review_result = crate::ai_core::ipc::ai_get_review_result();
    let eng_findings = eng_state.findings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let eng_traces = eng_state.traces.lock().unwrap_or_else(|e| e.into_inner()).clone();

    let bundle = build_report_bundle(&request, review_result.as_ref(), &eng_findings, &eng_traces)?;

    let output_path = &request.output_path;
    let target = Path::new(output_path);

    if target.exists() && !request.overwrite_existing {
        return Err(format!("Output file already exists: {}", output_path));
    }

    match request.format.as_str() {
        "html" => {
            let html = generate_html_report(&bundle, &request);
            if let Some(parent) = target.parent() { std::fs::create_dir_all(parent).ok(); }
            let mut file = std::fs::File::create(target).map_err(|e| format!("Create file: {e}"))?;
            file.write_all(html.as_bytes()).map_err(|e| format!("Write: {e}"))?;
        }
        "pdf" => {
            return Err("PDF report export is not available yet. HTML report is ready and can be printed to PDF from your browser.".to_string());
        }
        other => return Err(format!("Unsupported format: {other}")),
    }

    let mut sections = vec!["cover".to_string(), "metadata".to_string()];
    if request.include_review { sections.push("review".into()); }
    if request.include_engineering { sections.push("engineering".into()); }
    if request.include_calculation_traces { sections.push("traces".into()); }
    if request.include_ai_actions { sections.push("ai_actions".into()); }
    if request.include_audit_summary { sections.push("audit".into()); }
    if request.include_appendix { sections.push("appendix".into()); }

    Ok(ReportExportResult {
        report_id: format!("report-{}", epoch_ms()),
        output_path: output_path.clone(),
        format: request.format.clone(),
        sections_included: sections,
        findings_count: bundle.findings.len() + bundle.risks.len() + bundle.missing_information.len(),
        engineering_findings_count: bundle.engineering_findings.len(),
        calculation_traces_count: bundle.calculation_traces.len(),
        ai_actions_count: bundle.ai_actions.len(),
        citations_count: bundle.citations.len(),
        warnings: bundle.warnings,
        created_at: epoch_ms(),
    })
}

#[tauri::command]
pub fn report_preview_html(
    eng_state: State<'_, crate::engineering::EngineeringState>,
    request: ReportExportRequest,
) -> Result<String, String> {
    let review_result = crate::ai_core::ipc::ai_get_review_result();
    let eng_findings = eng_state.findings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let eng_traces = eng_state.traces.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let bundle = build_report_bundle(&request, review_result.as_ref(), &eng_findings, &eng_traces)?;
    Ok(generate_html_report(&bundle, &request))
}

fn epoch_ms() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_bundle_errors_when_no_data() {
        let req = ReportExportRequest { session_id: "s1".into(), report_title: "T".into(), output_path: "".into(), format: "html".into(), include_review: true, include_engineering: true, include_calculation_traces: true, include_ai_actions: true, include_audit_summary: true, include_source_snippets: true, include_appendix: true, overwrite_existing: true };
        let result = build_report_bundle(&req, None, &[], &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No review or engineering"));
    }

    #[test]
    fn build_bundle_from_engineering_only() {
        let req = ReportExportRequest { session_id: "s1".into(), report_title: "T".into(), output_path: "".into(), format: "html".into(), include_review: false, include_engineering: true, include_calculation_traces: true, include_ai_actions: false, include_audit_summary: false, include_source_snippets: false, include_appendix: false, overwrite_existing: true };
        let eng = vec![crate::engineering::types::EngineeringFinding { finding_id: "ef1".into(), finding_type: "total_connected_load".into(), severity: crate::engineering::types::FindingSeverity::Info, title: "50 kW".into(), description: "Sum".into(), page_refs: vec![0], source_citations: vec![], calculation_trace_id: None, recommendation: "Verify".into(), confidence: 0.9 }];
        let result = build_report_bundle(&req, None, &eng, &[]);
        assert!(result.is_ok());
        let bundle = result.unwrap();
        assert_eq!(bundle.engineering_findings.len(), 1);
    }

    #[test]
    fn deduplicates_citations() {
        let req = ReportExportRequest { session_id: "s1".into(), report_title: "T".into(), output_path: "".into(), format: "html".into(), include_review: true, include_engineering: false, include_calculation_traces: false, include_ai_actions: false, include_audit_summary: false, include_source_snippets: false, include_appendix: true, overwrite_existing: true };
        let review = crate::ai_core::review::DocumentReviewResult {
            review_id: "r1".into(), session_id: "s1".into(), summary: "S".into(),
            findings: vec![], risks: vec![], missing_information: vec![],
            engineering_findings: vec![], suggested_actions: vec![],
            citations: vec![
                crate::ai_core::rag::Citation { citation_id: "c1".into(), page_index: 0, chunk_id: "ch1".into(), snippet: "a".into(), score: 0.9, source: "native_text".into() },
                crate::ai_core::rag::Citation { citation_id: "c1".into(), page_index: 0, chunk_id: "ch1".into(), snippet: "a".into(), score: 0.9, source: "native_text".into() },
            ],
            warnings: vec![], elapsed_ms: 0, created_at: 0,
        };
        let bundle = build_report_bundle(&req, Some(&review), &[], &[]).unwrap();
        assert_eq!(bundle.citations.len(), 1); // Deduplicated.
    }
}
