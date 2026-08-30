use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportExportRequest {
    pub session_id: String,
    pub report_title: String,
    pub output_path: String,
    pub format: String, // "html" | "pdf"
    pub include_review: bool,
    pub include_engineering: bool,
    pub include_calculation_traces: bool,
    pub include_ai_actions: bool,
    pub include_audit_summary: bool,
    pub include_source_snippets: bool,
    pub include_appendix: bool,
    pub overwrite_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportExportResult {
    pub report_id: String,
    pub output_path: String,
    pub format: String,
    pub sections_included: Vec<String>,
    pub findings_count: usize,
    pub engineering_findings_count: usize,
    pub calculation_traces_count: usize,
    pub ai_actions_count: usize,
    pub citations_count: usize,
    pub warnings: Vec<String>,
    pub created_at: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportBundle {
    pub session_id: String,
    pub document_path: String,
    pub report_title: String,
    pub review_summary: String,
    pub findings: Vec<ReportFinding>,
    pub risks: Vec<ReportFinding>,
    pub missing_information: Vec<ReportFinding>,
    pub engineering_findings: Vec<ReportFinding>,
    pub calculation_traces: Vec<ReportTrace>,
    pub ai_actions: Vec<ReportAction>,
    pub audit_entries: Vec<ReportAuditEntry>,
    pub citations: Vec<ReportCitation>,
    pub warnings: Vec<String>,
    pub metadata: ReportMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub generated_at: String,
    pub page_count: usize,
    pub retrieval_mode: String,
    pub ocr_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportFinding {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub page_refs: Vec<usize>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTrace {
    pub id: String,
    pub title: String,
    pub calculation_type: String,
    pub formula: String,
    pub inputs: Vec<String>,
    pub final_value: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAction {
    pub id: String,
    pub action_type: String,
    pub page_index: usize,
    pub text: String,
    pub reason: String,
    pub status: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportAuditEntry {
    pub timestamp: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportCitation {
    pub id: String,
    pub page: usize,
    pub source: String,
    pub snippet: String,
    pub score: f32,
}
