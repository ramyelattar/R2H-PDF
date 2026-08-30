import { useCallback, useEffect, useRef, useState } from "react";
import {
  ocrCancel,
  ocrCheckAvailability,
  ocrHasResult,
  ocrRunPage,
} from "../../lib/ipc";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import { validateOcrResult } from "./ocrContract";
import type { OcrAvailability, OcrPageResult, OcrState } from "./types";

interface UseOcrDeps {
  sessionId: string;
  totalPages: number;
  appendDiagnostic: AppendDiagnostic;
}

const makeOperationId = (): string => {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `ocr-${crypto.randomUUID()}`;
  }
  return `ocr-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
};

const initialState = (totalPages: number): OcrState => ({
  status: "idle",
  currentPage: null,
  totalPages,
  progressCurrent: 0,
  progressTotal: 0,
  lastResult: null,
  error: null,
  available: false,
  availability: null,
  operationId: null,
  acceptance: "none",
  persistedBlockCount: 0,
});

export function useOcr(deps: UseOcrDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;
  const availabilityRef = useRef<OcrAvailability | null>(null);
  const inFlightRef = useRef(false);
  const operationRef = useRef<string | null>(null);

  const [state, setState] = useState<OcrState>(() => initialState(deps.totalPages));

  useEffect(() => {
    availabilityRef.current = null;
    operationRef.current = null;
    inFlightRef.current = false;
    setState(initialState(deps.totalPages));
  }, [deps.sessionId, deps.totalPages]);

  const checkAvailability = useCallback(async () => {
    setState((prev) => ({ ...prev, status: "validating", error: null }));
    const result = await ocrCheckAvailability();
    if (!result.ok) {
      availabilityRef.current = null;
      setState((prev) => ({
        ...prev,
        status: "blocked",
        available: false,
        availability: null,
        error: `OCR availability check failed: ${result.error.message}`,
      }));
      return false;
    }

    availabilityRef.current = result.data;
    setState((prev) => ({
      ...prev,
      status: result.data.available ? "idle" : "blocked",
      available: result.data.available,
      availability: result.data,
      error: null,
    }));
    return result.data.available;
  }, []);

  const ensureAvailability = useCallback(async () => {
    if (availabilityRef.current?.available) return true;
    return checkAvailability();
  }, [checkAvailability]);

  const fail = useCallback((message: string, appendDiagnostic: AppendDiagnostic, status: "failed" | "partial_failure" = "failed") => {
    setState((prev) => ({ ...prev, status, error: message, operationId: null }));
    appendDiagnostic({ level: "ERROR", source: "ipc", message: `OCR failed: ${message}` });
  }, []);

  const runPages = useCallback(async (pages: number[], force: boolean) => {
    const { sessionId, totalPages, appendDiagnostic } = depsRef.current;
    if (inFlightRef.current) {
      fail("An OCR operation is already running.", appendDiagnostic);
      return null;
    }
    if (!sessionId.startsWith("doc-session-")) {
      fail("OCR requires an active native PDF session.", appendDiagnostic);
      return null;
    }
    if (totalPages <= 0 || pages.length === 0 || pages.some((page) => !Number.isInteger(page) || page < 0 || page >= totalPages)) {
      fail("OCR page selection is invalid for the active PDF session.", appendDiagnostic);
      return null;
    }
    if (!(await ensureAvailability())) return null;

    const availability = availabilityRef.current;
    const operationId = makeOperationId();
    const cancellationId = operationId;
    const language = "auto";
    const modelId = availability?.model_id || "PP-OCRv5";
    inFlightRef.current = true;
    operationRef.current = operationId;
    setState((prev) => ({
      ...prev,
      status: "running",
      currentPage: pages[0] ?? null,
      progressCurrent: 0,
      progressTotal: pages.length,
      error: null,
      operationId,
      acceptance: "none",
      persistedBlockCount: 0,
    }));
    appendDiagnostic({
      level: "INFO",
      source: "ipc",
      message: `OCR started: ${pages.length === 1 ? `page ${pages[0] + 1}` : `pages ${pages[0] + 1}–${pages[pages.length - 1] + 1}`}.`,
    });

    let lastResult: OcrPageResult | null = null;
    let allPagesNoText = true;
    try {
      for (let index = 0; index < pages.length; index += 1) {
        const pageIndex = pages[index];
        setState((prev) => ({ ...prev, currentPage: pageIndex, progressCurrent: index }));
        const result = await ocrRunPage({
          operation_id: operationId,
          session_id: sessionId,
          page_index: pageIndex,
          force,
          dpi: 200,
          language,
          model_id: modelId,
          output_format_version: 1,
          timeout_secs: modelId.startsWith("PP-OCRv5") ? 600 : 7200,
          cancellation_id: cancellationId,
        });

        if (!result.ok) {
          const message = result.error.message;
          const cancelled = message.includes("OCR_CANCELLED") || result.error.code === "OCR_CANCELLED";
          setState((prev) => ({
            ...prev,
            status: cancelled ? "cancelled" : "failed",
            error: message,
            operationId: null,
            progressCurrent: index,
          }));
          appendDiagnostic({ level: cancelled ? "WARN" : "ERROR", source: "ipc", message: `OCR ${cancelled ? "cancelled" : "failed"} on page ${pageIndex + 1}: ${message}` });
          return null;
        }

        const validation = validateOcrResult(result.data);
        if (!validation.ok) {
          fail(`[${validation.code}] ${validation.message}`, appendDiagnostic);
          return null;
        }

        lastResult = result.data;
        if (result.data.status !== "no_text_detected") allPagesNoText = false;
        setState((prev) => ({
          ...prev,
          lastResult: result.data,
          progressCurrent: index + 1,
          error: null,
        }));
      }

      const finalStatus = allPagesNoText ? "no_text_detected" : "completed";
      setState((prev) => ({
        ...prev,
        status: finalStatus,
        lastResult,
        progressCurrent: pages.length,
        error: null,
        operationId: null,
      }));
      if (finalStatus === "no_text_detected") {
        appendDiagnostic({ level: "INFO", source: "ipc", message: `OCR completed with no text detected across ${pages.length} page(s).` });
      } else {
        appendDiagnostic({ level: "INFO", source: "ipc", message: `OCR completed: ${pages.length} page(s), ${lastResult?.blocks.length ?? 0} validated text blocks.` });
      }
      return lastResult;
    } finally {
      inFlightRef.current = false;
      operationRef.current = null;
    }
  }, [ensureAvailability, fail]);

  const ocrCurrentPage = useCallback((pageIndex: number, force = false) => runPages([pageIndex], force), [runPages]);

  const ocrPageRange = useCallback((startPage: number, endPage: number, force = false) => {
    if (!Number.isInteger(startPage) || !Number.isInteger(endPage) || endPage < startPage) {
      const { appendDiagnostic } = depsRef.current;
      fail("OCR page range is empty or invalid.", appendDiagnostic);
      return Promise.resolve(null);
    }
    return runPages(Array.from({ length: endPage - startPage + 1 }, (_, offset) => startPage + offset), force);
  }, [fail, runPages]);

  const cancelOcr = useCallback(async () => {
    const operationId = operationRef.current;
    if (!operationId) return false;
    const result = await ocrCancel(operationId);
    if (!result.ok) {
      const { appendDiagnostic } = depsRef.current;
      fail(`Cancellation request failed: ${result.error.message}`, appendDiagnostic);
      return false;
    }
    setState((prev) => ({ ...prev, status: "cancelled", error: null }));
    return result.data.cancelled;
  }, [fail]);

  const hasPageResult = useCallback(async (pageIndex: number): Promise<boolean> => {
    const { sessionId } = depsRef.current;
    const result = await ocrHasResult(sessionId, pageIndex);
    return result.ok && result.data;
  }, []);

  const setAcceptance = useCallback((acceptance: OcrState["acceptance"], persistedBlockCount = 0) => {
    setState((prev) => ({ ...prev, acceptance, persistedBlockCount }));
  }, []);

  return {
    state,
    checkAvailability,
    ocrCurrentPage,
    ocrPageRange,
    cancelOcr,
    hasPageResult,
    setAcceptance,
  };
}
