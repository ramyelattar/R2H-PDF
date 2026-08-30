import { useCallback, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { invokeSafe } from "../../lib/ipc";
import type { ReportExportResult, ReportFormat, ReportStatus } from "./types";

export function useReportExport() {
  const [status, setStatus] = useState<ReportStatus>("idle");
  const [lastResult, setLastResult] = useState<ReportExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const exportReport = useCallback(async (options: {
    sessionId: string;
    reportTitle: string;
    format: ReportFormat;
    includeReview: boolean;
    includeEngineering: boolean;
    includeTraces: boolean;
    includeAiActions: boolean;
    includeAudit: boolean;
    includeAppendix: boolean;
  }) => {
    const ext = options.format === "pdf" ? "pdf" : "html";
    const outputPath = await save({
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      defaultPath: `review_report.${ext}`,
    });
    if (!outputPath) return null;

    setStatus("exporting");
    setError(null);

    const result = await invokeSafe<ReportExportResult>("report_export_review", {
      request: {
        session_id: options.sessionId,
        report_title: options.reportTitle,
        output_path: outputPath,
        format: options.format,
        include_review: options.includeReview,
        include_engineering: options.includeEngineering,
        include_calculation_traces: options.includeTraces,
        include_ai_actions: options.includeAiActions,
        include_audit_summary: options.includeAudit,
        include_source_snippets: true,
        include_appendix: options.includeAppendix,
        overwrite_existing: true,
      },
    });

    if (!result.ok) {
      setStatus("failed");
      setError(result.error.message);
      return null;
    }

    setStatus("completed");
    setLastResult(result.data);
    return result.data;
  }, []);

  return { status, lastResult, error, exportReport };
}
