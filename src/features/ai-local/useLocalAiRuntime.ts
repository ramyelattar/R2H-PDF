import { useCallback, useState } from "react";
import { invokeSafe } from "../../lib/ipc";
import type { LocalGenerateRequest, LocalGenerateResult, LocalModelInfo, LocalRuntimeStatus } from "./types";

export function useLocalAiRuntime() {
  const [status, setStatus] = useState<LocalRuntimeStatus | null>(null);
  const [models, setModels] = useState<LocalModelInfo[]>([]);
  const [generating, setGenerating] = useState(false);
  const [lastResult, setLastResult] = useState<LocalGenerateResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    const result = await invokeSafe<LocalRuntimeStatus>("ai_get_local_runtime_status", {});
    if (result.ok) setStatus(result.data);
    else setError(result.error.message);
  }, []);

  const refreshModels = useCallback(async () => {
    const result = await invokeSafe<LocalModelInfo[]>("ai_list_local_models", {});
    if (result.ok) setModels(result.data);
    else setError(result.error.message);
  }, []);

  const validateModel = useCallback(async (modelId: string): Promise<LocalModelInfo | null> => {
    const result = await invokeSafe<LocalModelInfo>("ai_validate_local_model", { modelId });
    if (result.ok) return result.data;
    setError(result.error.message);
    return null;
  }, []);

  const generate = useCallback(async (request: LocalGenerateRequest): Promise<LocalGenerateResult | null> => {
    setGenerating(true);
    setError(null);
    setLastResult(null);

    const result = await invokeSafe<LocalGenerateResult>("ai_generate_local", { request });

    setGenerating(false);
    if (result.ok) {
      if (result.data.finish_reason === "error" || !result.data.text.trim()) {
        const message = result.data.finish_reason === "error"
          ? "Local runtime reported an error without a usable response."
          : "Local runtime returned an empty response.";
        setError(message);
        return null;
      }
      setLastResult(result.data);
      return result.data;
    }
    setError(result.error.message);
    return null;
  }, []);

  return {
    status,
    models,
    generating,
    lastResult,
    error,
    refreshStatus,
    refreshModels,
    validateModel,
    generate,
    clearLastResult: () => setLastResult(null),
    setErrorMessage: setError,
  };
}
