export interface ReportExportResult {
  report_id: string;
  output_path: string;
  format: string;
  sections_included: string[];
  findings_count: number;
  engineering_findings_count: number;
  calculation_traces_count: number;
  ai_actions_count: number;
  citations_count: number;
  warnings: string[];
  created_at: number;
}

export type ReportFormat = "html" | "pdf";
export type ReportStatus = "idle" | "exporting" | "completed" | "failed";
