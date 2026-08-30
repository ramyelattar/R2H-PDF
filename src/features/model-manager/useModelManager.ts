import { useCallback, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { LocalAiValidationResult } from "./types";

export function useModelManager() {
  const [validation, setValidation] = useState<LocalAiValidationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const getStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    const result = await invokeSafe<LocalAiValidationResult>("local_ai_get_status", {});
    setLoading(false);
    if (result.ok) { setValidation(result.data); return result.data; }
    setError(result.error.message);
    return null;
  }, []);

  const validate = useCallback(async (path?: string) => {
    setLoading(true);
    setError(null);
    const result = await invokeSafe<LocalAiValidationResult>("local_ai_validate", { path: path ?? null });
    setLoading(false);
    if (result.ok) { setValidation(result.data); return result.data; }
    setError(result.error.message);
    return null;
  }, []);

  return { validation, loading, error, getStatus, validate };
}
