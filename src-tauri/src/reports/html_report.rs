//! Self-contained HTML report generator. No external assets.

use super::types::*;

/// Escape HTML special characters to prevent injection.
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Generate a complete self-contained HTML report.
pub fn generate_html_report(bundle: &ReportBundle, request: &ReportExportRequest) -> String {
    let mut html = String::with_capacity(32_000);

    html.push_str(&format!(r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>{}</title><style>{}</style></head><body>"#,
        escape_html(&request.report_title), CSS));

    // Cover
    html.push_str(&format!(r#"<div class="cover"><h1>{}</h1><p class="subtitle">R2H PDF AI Workstation — Review Report</p><table class="meta-table"><tr><td>Document</td><td>{}</td></tr><tr><td>Generated</td><td>{}</td></tr><tr><td>Pages</td><td>{}</td></tr><tr><td>Retrieval</td><td>{}</td></tr><tr><td>OCR</td><td>{}</td></tr></table></div>"#,
        escape_html(&bundle.report_title),
        escape_html(&bundle.document_path),
        escape_html(&bundle.metadata.generated_at),
        bundle.metadata.page_count,
        escape_html(&bundle.metadata.retrieval_mode),
        if bundle.metadata.ocr_included { "Yes" } else { "No" },
    ));

    // Summary
    if request.include_review && !bundle.review_summary.is_empty() {
        html.push_str(&format!(
            r#"<h2>Executive Summary</h2><p>{}</p>"#,
            escape_html(&bundle.review_summary)
        ));
    }

    // Findings Overview
    let total = bundle.findings.len()
        + bundle.risks.len()
        + bundle.missing_information.len()
        + bundle.engineering_findings.len();
    if total > 0 {
        html.push_str(&format!(
            r#"<h2>Findings Overview</h2><p>Total: {} findings</p><ul>"#,
            total
        ));
        let counts = count_severities(
            &bundle.findings,
            &bundle.risks,
            &bundle.missing_information,
            &bundle.engineering_findings,
        );
        for (sev, count) in &counts {
            html.push_str(&format!(
                "<li><span class=\"badge badge-{}\">{}</span>: {}</li>",
                sev, sev, count
            ));
        }
        html.push_str("</ul>");
    }

    // Findings table
    let all_findings: Vec<&ReportFinding> = bundle
        .findings
        .iter()
        .chain(&bundle.risks)
        .chain(&bundle.missing_information)
        .collect();
    if !all_findings.is_empty() && request.include_review {
        html.push_str(r#"<h2>Review Findings</h2><table class="findings-table"><thead><tr><th>Severity</th><th>Category</th><th>Title</th><th>Description</th><th>Pages</th><th>Recommendation</th></tr></thead><tbody>"#);
        for f in &all_findings {
            html.push_str(&format!(r#"<tr><td><span class="badge badge-{}">{}</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                escape_html(&f.severity), escape_html(&f.severity), escape_html(&f.category),
                escape_html(&f.title), escape_html(&f.description),
                f.page_refs.iter().map(|p| format!("{}", p + 1)).collect::<Vec<_>>().join(", "),
                escape_html(&f.recommendation),
            ));
        }
        html.push_str("</tbody></table>");
    }

    // Engineering findings
    if request.include_engineering && !bundle.engineering_findings.is_empty() {
        html.push_str("<h2>Engineering Findings</h2><table class=\"findings-table\"><thead><tr><th>Severity</th><th>Title</th><th>Description</th><th>Pages</th><th>Recommendation</th></tr></thead><tbody>");
        for f in &bundle.engineering_findings {
            html.push_str(&format!("<tr><td><span class=\"badge badge-{}\">{}</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&f.severity), escape_html(&f.severity),
                escape_html(&f.title), escape_html(&f.description),
                f.page_refs.iter().map(|p| format!("{}", p + 1)).collect::<Vec<_>>().join(", "),
                escape_html(&f.recommendation),
            ));
        }
        html.push_str("</tbody></table>");
    }

    // Calculation traces
    if request.include_calculation_traces && !bundle.calculation_traces.is_empty() {
        html.push_str("<h2>Calculation Traces</h2>");
        for t in &bundle.calculation_traces {
            html.push_str(&format!(
                r#"<div class="trace-block"><h4>{}</h4><p class="formula">{}</p><ul>"#,
                escape_html(&t.title),
                escape_html(&t.formula)
            ));
            for inp in &t.inputs {
                html.push_str(&format!("<li>{}</li>", escape_html(inp)));
            }
            html.push_str(&format!(
                "</ul><p class=\"result\">Result: {}</p>",
                escape_html(&t.final_value)
            ));
            if !t.warnings.is_empty() {
                for w in &t.warnings {
                    html.push_str(&format!("<p class=\"warning\">⚠ {}</p>", escape_html(w)));
                }
            }
            html.push_str("</div>");
        }
    }

    // AI Actions
    if request.include_ai_actions && !bundle.ai_actions.is_empty() {
        html.push_str("<h2>AI Suggested Actions</h2><table class=\"actions-table\"><thead><tr><th>Type</th><th>Page</th><th>Text</th><th>Status</th><th>Confidence</th></tr></thead><tbody>");
        for a in &bundle.ai_actions {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}%</td></tr>",
                escape_html(&a.action_type),
                a.page_index + 1,
                escape_html(&a.text),
                escape_html(&a.status),
                (a.confidence * 100.0) as u32,
            ));
        }
        html.push_str("</tbody></table>");
    }

    // Audit summary
    if request.include_audit_summary && !bundle.audit_entries.is_empty() {
        html.push_str("<h2>Audit Summary</h2><table class=\"audit-table\"><thead><tr><th>Time</th><th>Action</th><th>Detail</th></tr></thead><tbody>");
        for e in bundle.audit_entries.iter().take(50) {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&e.timestamp),
                escape_html(&e.action),
                escape_html(&e.detail)
            ));
        }
        html.push_str("</tbody></table>");
    }

    // Appendix: Citations
    if request.include_appendix && !bundle.citations.is_empty() {
        html.push_str("<h2>Appendix: Source Citations</h2><table class=\"citations-table\"><thead><tr><th>ID</th><th>Page</th><th>Source</th><th>Snippet</th></tr></thead><tbody>");
        for c in &bundle.citations {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"snippet\">{}</td></tr>",
                escape_html(&c.id),
                c.page + 1,
                escape_html(&c.source),
                escape_html(&c.snippet.chars().take(150).collect::<String>()),
            ));
        }
        html.push_str("</tbody></table>");
    }

    html.push_str("</body></html>");
    html
}

fn count_severities(
    a: &[ReportFinding],
    b: &[ReportFinding],
    c: &[ReportFinding],
    d: &[ReportFinding],
) -> Vec<(String, usize)> {
    let all: Vec<&str> = a
        .iter()
        .chain(b)
        .chain(c)
        .chain(d)
        .map(|f| f.severity.as_str())
        .collect();
    let mut counts = Vec::new();
    for sev in &["critical", "major", "warning", "info"] {
        let n = all.iter().filter(|s| **s == *sev).count();
        if n > 0 {
            counts.push((sev.to_string(), n));
        }
    }
    counts
}

const CSS: &str = r#"
*{box-sizing:border-box;margin:0;padding:0}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;line-height:1.6;color:#1a1a2e;max-width:900px;margin:0 auto;padding:40px 24px}
h1{font-size:24px;margin-bottom:8px}h2{font-size:18px;margin:32px 0 12px;padding-bottom:6px;border-bottom:2px solid #e0e0e0}
h4{font-size:14px;margin-bottom:6px}
p{margin:6px 0}
.cover{margin-bottom:40px;padding-bottom:20px;border-bottom:3px solid #333}
.subtitle{color:#666;font-size:14px;margin-bottom:16px}
.meta-table{font-size:13px}
.meta-table td{padding:3px 12px 3px 0}
.meta-table td:first-child{font-weight:600;color:#555}
table{width:100%;border-collapse:collapse;margin:12px 0;font-size:12px}
th,td{border:1px solid #ddd;padding:6px 8px;text-align:left;vertical-align:top}
th{background:#f5f5f5;font-weight:600}
.badge{display:inline-block;padding:2px 8px;border-radius:4px;font-size:11px;font-weight:600;text-transform:uppercase}
.badge-info{background:#e3f2fd;color:#1565c0}
.badge-warning{background:#fff3e0;color:#e65100}
.badge-major{background:#fce4ec;color:#c62828}
.badge-critical{background:#b71c1c;color:#fff}
.trace-block{background:#f8f9fa;border:1px solid #e0e0e0;border-radius:6px;padding:12px;margin:12px 0}
.formula{font-family:monospace;background:#fff;padding:4px 8px;border-radius:3px;border:1px solid #ddd}
.result{font-weight:700;color:#1565c0}
.warning{color:#e65100;font-size:12px}
.snippet{font-size:11px;color:#555;max-width:300px;overflow:hidden;text-overflow:ellipsis}
@media print{body{padding:20px}h2{page-break-before:auto}table{page-break-inside:avoid}.trace-block{page-break-inside:avoid}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_html_works() {
        assert_eq!(
            escape_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"
        );
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn generates_self_contained_html() {
        let bundle = ReportBundle {
            session_id: "s1".into(),
            document_path: "/test.pdf".into(),
            report_title: "Test Report".into(),
            review_summary: "Summary here.".into(),
            findings: vec![ReportFinding {
                id: "f1".into(),
                severity: "warning".into(),
                category: "general".into(),
                title: "Test".into(),
                description: "Desc".into(),
                recommendation: "Fix".into(),
                page_refs: vec![0],
                confidence: 0.8,
            }],
            risks: vec![],
            missing_information: vec![],
            engineering_findings: vec![],
            calculation_traces: vec![],
            ai_actions: vec![],
            audit_entries: vec![],
            citations: vec![ReportCitation {
                id: "c1".into(),
                page: 0,
                source: "native_text".into(),
                snippet: "test snippet".into(),
                score: 0.9,
            }],
            warnings: vec![],
            metadata: ReportMetadata {
                generated_at: "2026-05-14".into(),
                page_count: 10,
                retrieval_mode: "hybrid".into(),
                ocr_included: false,
            },
        };
        let req = ReportExportRequest {
            session_id: "s1".into(),
            report_title: "Test".into(),
            output_path: "/tmp/r.html".into(),
            format: "html".into(),
            include_review: true,
            include_engineering: true,
            include_calculation_traces: true,
            include_ai_actions: true,
            include_audit_summary: true,
            include_source_snippets: true,
            include_appendix: true,
            overwrite_existing: true,
        };
        let html = generate_html_report(&bundle, &req);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Report"));
        assert!(html.contains("Summary here."));
        assert!(html.contains("test snippet"));
        // No external URLs.
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn html_escapes_user_text() {
        let bundle = ReportBundle {
            session_id: "s1".into(),
            document_path: "<script>bad</script>".into(),
            report_title: "Report".into(),
            review_summary: "a < b & c > d".into(),
            findings: vec![],
            risks: vec![],
            missing_information: vec![],
            engineering_findings: vec![],
            calculation_traces: vec![],
            ai_actions: vec![],
            audit_entries: vec![],
            citations: vec![],
            warnings: vec![],
            metadata: ReportMetadata {
                generated_at: "now".into(),
                page_count: 1,
                retrieval_mode: "bm25".into(),
                ocr_included: false,
            },
        };
        let req = ReportExportRequest {
            session_id: "s1".into(),
            report_title: "R".into(),
            output_path: "".into(),
            format: "html".into(),
            include_review: true,
            include_engineering: false,
            include_calculation_traces: false,
            include_ai_actions: false,
            include_audit_summary: false,
            include_source_snippets: false,
            include_appendix: false,
            overwrite_existing: true,
        };
        let html = generate_html_report(&bundle, &req);
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("a &lt; b &amp; c &gt; d"));
        assert!(!html.contains("<script>bad</script>"));
    }
}
