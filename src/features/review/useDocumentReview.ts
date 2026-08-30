import { useCallback, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { DocumentReviewResult, ReviewStatus } from "./types";

export function useDocumentReview() {
  const [status, setStatus] = useState<ReviewStatus>("idle");
  const [result, setResult] = useState<DocumentReviewResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reviewDocument = useCallback(async (sessionId: string, includeEngineering: boolean) => {
    setStatus("running");
    setError(null);

    const res = await invokeSafe<DocumentReviewResult>("ai_review_document", {
      request: {
        session_id: sessionId,
        include_engineering: includeEngineering,
        include_ocr: false,
        max_findings: 20,
        suggest_actions: true,
      },
    });

    if (!res.ok) {
      setStatus("failed");
      setError(res.error.message);
      return null;
    }

    setResult(res.data);
    setStatus("completed");
    return res.data;
  }, []);

  const clearReview = useCallback(async () => {
    const result = await invokeSafe<void>("ai_clear_review_result", {});
    if (!result.ok) {
      setStatus("failed");
      setError(result.error.message);
      return false;
    }
    setResult(null);
    setStatus("idle");
    setError(null);
    return true;
  }, []);

  return { status, result, error, reviewDocument, clearReview };
}
