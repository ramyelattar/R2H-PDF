import { useCallback, useState } from "react";
import type { UiPreferences } from "../../types/shell";
import {
  aiRunTask,
  annotCreate,
  type AiTaskRequest,
  type AiTaskResult,
  type AiAnnotationSuggestion,
  type AiEntity,
} from "../../lib/ipc";

type AiTab = "summary" | "qa" | "suggestions" | "entities";

interface AiPanelProps {
  sessionId: string;
  preferences: UiPreferences;
}

export const AiPanel = ({ sessionId, preferences }: AiPanelProps) => {
  const [activeTab, setActiveTab] = useState<AiTab>("summary");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Results state
  const [summaryText, setSummaryText] = useState<string | null>(null);
  const [qaText, setQaText] = useState<string | null>(null);
  const [qaQuestion, setQaQuestion] = useState("");
  const [suggestions, setSuggestions] = useState<AiAnnotationSuggestion[]>([]);
  const [entities, setEntities] = useState<AiEntity[]>([]);

  const runTask = useCallback(
    async (
      taskType: AiTaskRequest["task_type"],
      question?: string,
    ) => {
      setLoading(true);
      setError(null);

      const request: AiTaskRequest = {
        task_id: `${taskType}-${Date.now()}`,
        session_id: sessionId,
        task_type: taskType,
        model: preferences.aiModel,
        input_text: "",
        ...(question ? { question } : {}),
      };

      const result = await aiRunTask(request);

      if (!result.ok) {
        setError(result.error.message);
        setLoading(false);
        return;
      }

      const data: AiTaskResult = result.data;
      if (!data.success) {
        setError(data.error ?? "Task failed");
        setLoading(false);
        return;
      }

      switch (taskType) {
        case "summarize":
          setSummaryText(data.output_text ?? null);
          break;
        case "question_answer":
          setQaText(data.output_text ?? null);
          break;
        case "suggest_annotations":
          setSuggestions(data.suggestions ?? []);
          break;
        case "extract_entities":
          setEntities(data.entities ?? []);
          break;
      }

      setLoading(false);
    },
    [sessionId, preferences.aiModel],
  );

  const handleAcceptSuggestion = useCallback(
    async (suggestion: AiAnnotationSuggestion) => {
      await annotCreate({
        session_id: sessionId,
        source_path: null,
        page_index: suggestion.page_index,
        annot_type: "Note",
        color: { r: 255, g: 234, b: 0, a: 255 },
        contents: suggestion.suggested_note,
        author: "AI",
        rect: [50, 50, 250, 100],
      });
      setSuggestions((prev) =>
        prev.filter((s) => s !== suggestion),
      );
    },
    [sessionId],
  );

  const handleDismissSuggestion = useCallback(
    (suggestion: AiAnnotationSuggestion) => {
      setSuggestions((prev) =>
        prev.filter((s) => s !== suggestion),
      );
    },
    [],
  );

  const tabs: { key: AiTab; label: string; enabled: boolean }[] = [
    { key: "summary", label: "Summary", enabled: preferences.aiSummarizeEnabled },
    { key: "qa", label: "Q&A", enabled: preferences.aiQaEnabled },
    { key: "suggestions", label: "Suggestions", enabled: preferences.aiAnnotationSuggestEnabled },
    { key: "entities", label: "Entities", enabled: preferences.aiEntityExtractEnabled },
  ];

  return (
    <section className="ai-panel">
      <nav className="ai-panel-tabs" role="tablist" aria-label="AI features">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            role="tab"
            aria-selected={activeTab === tab.key}
            className={`ai-tab ${activeTab === tab.key ? "ai-tab--active" : ""}`}
            disabled={!tab.enabled}
            onClick={() => setActiveTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      <div className="ai-panel-content" role="tabpanel">
        {error && <div className="ai-panel-error" role="alert">{error}</div>}

        {loading && <div className="ai-panel-loading">Running…</div>}

        {!loading && activeTab === "summary" && (
          <div className="ai-tab-content">
            <button
              className="ai-run-btn"
              onClick={() => runTask("summarize")}
              disabled={!preferences.aiSummarizeEnabled}
            >
              Run Summary
            </button>
            {summaryText && (
              <div className="ai-result-text">{summaryText}</div>
            )}
          </div>
        )}

        {!loading && activeTab === "qa" && (
          <div className="ai-tab-content">
            <div className="ai-qa-input">
              <input
                type="text"
                value={qaQuestion}
                onChange={(e) => setQaQuestion(e.target.value)}
                placeholder="Ask a question about the document…"
                aria-label="Question"
              />
              <button
                className="ai-run-btn"
                onClick={() => runTask("question_answer", qaQuestion)}
                disabled={!preferences.aiQaEnabled || !qaQuestion.trim()}
              >
                Ask
              </button>
            </div>
            {qaText && (
              <div className="ai-result-text">{qaText}</div>
            )}
          </div>
        )}

        {!loading && activeTab === "suggestions" && (
          <div className="ai-tab-content">
            <button
              className="ai-run-btn"
              onClick={() => runTask("suggest_annotations")}
              disabled={!preferences.aiAnnotationSuggestEnabled}
            >
              Run Suggestions
            </button>
            {suggestions.length > 0 && (
              <ul className="ai-suggestions-list">
                {suggestions.map((s, i) => (
                  <li key={i} className="ai-suggestion-item">
                    <p className="ai-suggestion-snippet">
                      <strong>Page {s.page_index + 1}:</strong> {s.text_snippet}
                    </p>
                    <p className="ai-suggestion-note">{s.suggested_note}</p>
                    <div className="ai-suggestion-actions">
                      <button
                        className="ai-accept-btn"
                        onClick={() => handleAcceptSuggestion(s)}
                      >
                        Accept
                      </button>
                      <button
                        className="ai-dismiss-btn"
                        onClick={() => handleDismissSuggestion(s)}
                      >
                        Dismiss
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {!loading && activeTab === "entities" && (
          <div className="ai-tab-content">
            <button
              className="ai-run-btn"
              onClick={() => runTask("extract_entities")}
              disabled={!preferences.aiEntityExtractEnabled}
            >
              Extract Entities
            </button>
            {entities.length > 0 && (
              <table className="ai-entities-table">
                <thead>
                  <tr>
                    <th>Type</th>
                    <th>Value</th>
                    <th>Page</th>
                    <th>Context</th>
                  </tr>
                </thead>
                <tbody>
                  {entities.map((e, i) => (
                    <tr key={i}>
                      <td>{e.entity_type}</td>
                      <td>{e.value}</td>
                      <td>{e.page_index + 1}</td>
                      <td>{e.snippet}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        )}
      </div>
    </section>
  );
};
