import { useCallback, useRef, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { IndexStatus, PageText, RagAnswerResult, RagStatus } from "./types";

interface UseRagDeps {
  sessionId: string;
  appendDiagnostic: AppendDiagnostic;
}

export function useRag(deps: UseRagDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [status, setStatus] = useState<RagStatus>("idle");
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [lastAnswer, setLastAnswer] = useState<RagAnswerResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshIndexStatus = useCallback(async () => {
    const { sessionId, appendDiagnostic } = depsRef.current;
    const result = await invokeSafe<IndexStatus>("ai_get_document_index_status", { sessionId });
    if (result.ok) {
      setIndexStatus(result.data);
      return;
    }
    setStatus("error");
    setError(result.error.message);
    appendDiagnostic({ level: "ERROR", source: "ipc", message: `Index status failed: ${result.error.message}` });
  }, []);

  const buildIndex = useCallback(async (pageTexts: PageText[], includeOcr: boolean) => {
    const { sessionId, appendDiagnostic } = depsRef.current;
    setStatus("building");
    setError(null);
    appendDiagnostic({ level: "INFO", source: "ipc", message: "Building document index…" });

    const result = await invokeSafe<IndexStatus>("ai_build_document_index", {
      request: { session_id: sessionId, include_ocr: includeOcr, page_texts: pageTexts },
    });

    if (!result.ok) {
      setStatus("error");
      setError(result.error.message);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Index build failed: ${result.error.message}` });
      return;
    }

    setIndexStatus(result.data);
    setStatus("ready");
    appendDiagnostic({ level: "INFO", source: "ipc", message: `Index ready: ${result.data.chunk_count} chunks from ${result.data.indexed_pages} pages` });
  }, []);

  const clearIndex = useCallback(async () => {
    const { sessionId, appendDiagnostic } = depsRef.current;
    const result = await invokeSafe<void>("ai_clear_document_index", { sessionId });
    if (!result.ok) {
      setStatus("error");
      setError(result.error.message);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Index clear failed: ${result.error.message}` });
      return;
    }
    setIndexStatus(null);
    setStatus("idle");
    setLastAnswer(null);
  }, []);

  const askQuestion = useCallback(async (question: string, includeOcr: boolean, useReranker: boolean): Promise<RagAnswerResult | null> => {
    const { sessionId, appendDiagnostic } = depsRef.current;
    setStatus("asking");
    setError(null);
    setLastAnswer(null);

    const result = await invokeSafe<RagAnswerResult>("ai_ask_document_rag", {
      request: {
        session_id: sessionId,
        question,
        top_k: 5,
        include_ocr: includeOcr,
        use_reranker: useReranker,
        max_context_chars: 4000,
      },
    });

    if (!result.ok) {
      setStatus("error");
      setError(result.error.message);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `RAG failed: ${result.error.message}` });
      return null;
    }

    setLastAnswer(result.data);
    setStatus("ready");
    appendDiagnostic({ level: "INFO", source: "ipc", message: `RAG answer: ${result.data.retrieved_count} chunks, ${result.data.elapsed_ms}ms` });
    return result.data;
  }, []);

  return {
    status,
    indexStatus,
    lastAnswer,
    error,
    refreshIndexStatus,
    buildIndex,
    clearIndex,
    askQuestion,
  };
}
