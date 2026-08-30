import { useCallback, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { CalculationTrace, EngineeringFinding, LoadScheduleExtractionResult, LoadScheduleRow, UnitParseResult } from "./types";

export interface PageEngineeringText {
  page_index: number;
  native_text: string;
  ocr_text: string;
  source: string;
}

export function useEngineering() {
  const [loadRows, setLoadRows] = useState<LoadScheduleRow[]>([]);
  const [findings, setFindings] = useState<EngineeringFinding[]>([]);
  const [traces, setTraces] = useState<CalculationTrace[]>([]);
  const [pageTexts, setPageTexts] = useState<PageEngineeringText[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const parseUnits = useCallback(async (text: string): Promise<UnitParseResult | null> => {
    const result = await invokeSafe<UnitParseResult>("engineering_parse_units", { text });
    if (result.ok) return result.data;
    setError(result.error.message);
    return null;
  }, []);

  const loadPageTexts = useCallback(async (sessionId: string, includeOcr: boolean): Promise<PageEngineeringText[]> => {
    const result = await invokeSafe<PageEngineeringText[]>("engineering_get_page_texts", {
      sessionId, includeOcr,
    });
    if (result.ok) {
      setPageTexts(result.data);
      return result.data;
    }
    setError(result.error.message);
    return [];
  }, []);

  const extractLoadSchedule = useCallback(async (sessionId: string, includeOcr: boolean) => {
    setLoading(true);
    setError(null);

    // First load page texts from the active session.
    const texts = await loadPageTexts(sessionId, includeOcr);
    if (texts.length === 0) {
      setError("No native or OCR text is available. Run OCR or extract text first.");
      setLoading(false);
      return null;
    }

    // Convert to the format expected by the backend.
    const pageTextTuples: [number, string, string][] = texts
      .filter((t) => t.source !== "empty")
      .map((t) => {
        const mergedText = t.native_text || t.ocr_text;
        return [t.page_index, mergedText, t.source] as [number, string, string];
      });

    const result = await invokeSafe<LoadScheduleExtractionResult>("engineering_extract_load_schedule", {
      request: { session_id: sessionId, page_texts: pageTextTuples, include_ocr: includeOcr },
    });
    setLoading(false);
    if (result.ok) {
      setLoadRows(result.data.rows);
      setFindings(result.data.findings);
      return result.data;
    }
    setError(result.error.message);
    return null;
  }, [loadPageTexts]);

  const generateFindings = useCallback(async () => {
    const result = await invokeSafe<EngineeringFinding[]>("engineering_generate_findings", {});
    if (result.ok) { setFindings(result.data); return result.data; }
    setError(result.error.message);
    return null;
  }, []);

  const getTrace = useCallback(async (calcId: string): Promise<CalculationTrace | null> => {
    const result = await invokeSafe<CalculationTrace | null>("engineering_get_calculation_trace", { calculationId: calcId });
    if (result.ok && result.data) { setTraces((prev) => [...prev, result.data!]); return result.data; }
    return null;
  }, []);

  const clearState = useCallback(async () => {
    await invokeSafe<void>("engineering_clear_load_rows", {});
    setLoadRows([]);
    setFindings([]);
    setTraces([]);
    setPageTexts([]);
    setError(null);
  }, []);

  return { loadRows, findings, traces, pageTexts, loading, error, parseUnits, loadPageTexts, extractLoadSchedule, generateFindings, getTrace, clearState };
}
