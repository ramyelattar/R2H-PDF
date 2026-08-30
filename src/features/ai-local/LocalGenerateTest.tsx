import { useState } from "react";
import type { LocalGenerateRequest, LocalGenerateResult, LocalModelInfo } from "./types";

interface LocalGenerateTestProps {
  models: LocalModelInfo[];
  generating: boolean;
  lastResult: LocalGenerateResult | null;
  error: string | null;
  onGenerate: (request: LocalGenerateRequest) => Promise<LocalGenerateResult | null>;
  runtimeAvailable: boolean;
  documentLabel: string;
  onAccept?: (text: string) => Promise<boolean>;
  onReject?: () => void;
}

export const LocalGenerateTest = ({
  models,
  generating,
  lastResult,
  error,
  onGenerate,
  runtimeAvailable,
  documentLabel,
  onAccept,
  onReject,
}: LocalGenerateTestProps) => {
  const [prompt, setPrompt] = useState("Summarize the key points of a construction contract in 3 bullet points.");
  const defaultModel = models.find((m) => m.default) ?? models[0];
  const canGenerate = runtimeAvailable && defaultModel?.exists && !generating;

  const handleGenerate = () => {
    if (!defaultModel) return;
    void onGenerate({
      model_id: defaultModel.id,
      prompt,
      system_prompt: "You are a helpful document assistant. Be concise.",
      max_tokens: 256,
      temperature: 0.7,
      top_p: null,
      timeout_ms: 60000,
    });
  };

  return (
    <div className="local-ai-generate">
      <h5>Test Generation</h5>
      <p className="empty-state__hint">Context source: {documentLabel}</p>
      <textarea
        className="local-ai-prompt"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        rows={3}
        placeholder="Enter a test prompt…"
      />
      <button
        className="ghost-btn"
        onClick={handleGenerate}
        disabled={!canGenerate}
        title={!runtimeAvailable ? "Runtime not available" : !defaultModel?.exists ? "Model not found" : ""}
      >
        {generating ? "Generating…" : "Run Local Test"}
      </button>

      {error && <div className="local-ai-error">{error}</div>}

      {lastResult && (
        <div className="local-ai-result">
          <div className="local-ai-result__meta">
            <span>Model: {lastResult.model_id}</span>
            <span>Time: {lastResult.elapsed_ms}ms</span>
            <span>Reason: {lastResult.finish_reason}</span>
            {lastResult.tokens_generated && <span>Tokens: {lastResult.tokens_generated}</span>}
          </div>
          <div className="local-ai-result__text">{lastResult.text || "(empty response)"}</div>
          {onAccept && lastResult.text.trim() && (
            <div style={{ display: "flex", gap: 6, marginTop: 6 }}>
              <button className="btn btn--primary btn--sm" onClick={() => void onAccept(lastResult.text)} disabled={generating}>
                Accept &amp; Save to PDF
              </button>
              <button className="btn btn--ghost btn--sm" onClick={onReject} disabled={generating}>
                Reject
              </button>
            </div>
          )}
          {lastResult.warnings.length > 0 && (
            <details className="local-ai-result__warnings">
              <summary>Warnings</summary>
              {lastResult.warnings.map((w, i) => <p key={i}>{w}</p>)}
            </details>
          )}
        </div>
      )}
    </div>
  );
};
