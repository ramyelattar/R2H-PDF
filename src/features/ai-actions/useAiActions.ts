import { useCallback, useRef, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { AiActionBatch, AiProposedAction } from "./types";

interface UseAiActionsDeps {
  sessionId: string;
  pageCount: number;
  appendDiagnostic: AppendDiagnostic;
}

export function useAiActions(deps: UseAiActionsDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [batch, setBatch] = useState<AiActionBatch | null>(null);
  const [planning, setPlanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const planActions = useCallback(async (userRequest: string) => {
    const { sessionId, pageCount, appendDiagnostic } = depsRef.current;
    setPlanning(true);
    setError(null);
    appendDiagnostic({ level: "INFO", source: "ipc", message: `AI planning actions: "${userRequest}"` });

    const result = await invokeSafe<AiActionBatch>("ai_plan_document_actions", {
      request: { session_id: sessionId, user_request: userRequest, page_count: pageCount, top_k: 5 },
    });

    setPlanning(false);
    if (!result.ok) {
      setError(result.error.message);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `AI planning failed: ${result.error.message}` });
      return null;
    }

    setBatch(result.data);
    appendDiagnostic({ level: "INFO", source: "ipc", message: `AI proposed ${result.data.actions.length} actions` });
    return result.data;
  }, []);

  const acceptAction = useCallback(async (actionId: string) => {
    if (!batch) return;
    const result = await invokeSafe<void>("ai_accept_action", { batchId: batch.batch_id, actionId });
    if (!result.ok) {
      setError(result.error.message);
      depsRef.current.appendDiagnostic({ level: "ERROR", source: "ipc", message: `AI action accept failed: ${result.error.message}` });
      return;
    }
    setBatch((prev) => prev ? {
      ...prev,
      actions: prev.actions.map((a) => a.action_id === actionId ? { ...a, status: "accepted" as const } : a),
    } : null);
  }, [batch]);

  const rejectAction = useCallback(async (actionId: string) => {
    if (!batch) return;
    const result = await invokeSafe<void>("ai_reject_action", { batchId: batch.batch_id, actionId });
    if (!result.ok) {
      setError(result.error.message);
      depsRef.current.appendDiagnostic({ level: "ERROR", source: "ipc", message: `AI action reject failed: ${result.error.message}` });
      return;
    }
    setBatch((prev) => prev ? {
      ...prev,
      actions: prev.actions.map((a) => a.action_id === actionId ? { ...a, status: "rejected" as const } : a),
    } : null);
  }, [batch]);

  const applyAccepted = useCallback(async (): Promise<AiProposedAction[]> => {
    if (!batch) return [];
    const result = await invokeSafe<string[]>("ai_apply_action_batch", { batchId: batch.batch_id });
    if (!result.ok) {
      setError(result.error.message);
      return [];
    }
    const appliedIds = new Set(result.data);
    const appliedActions = batch.actions.filter((a) => appliedIds.has(a.action_id));
    setBatch((prev) => prev ? {
      ...prev,
      actions: prev.actions.map((a) => appliedIds.has(a.action_id) ? { ...a, status: "applied" as const } : a),
    } : null);
    return appliedActions;
  }, [batch]);

  const clearBatch = useCallback(async () => {
    if (!batch) return;
    const result = await invokeSafe<void>("ai_clear_action_batch", { batchId: batch.batch_id });
    if (!result.ok) {
      setError(result.error.message);
      depsRef.current.appendDiagnostic({ level: "ERROR", source: "ipc", message: `AI action batch clear failed: ${result.error.message}` });
      return;
    }
    setBatch(null);
  }, [batch]);

  return { batch, planning, error, planActions, acceptAction, rejectAction, applyAccepted, clearBatch };
}
