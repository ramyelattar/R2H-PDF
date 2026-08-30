import { useCallback, useEffect, useState } from "react";
import type { UiPreferences } from "../../types/shell";
import { aiGetStatus } from "../../lib/ipc";

const URL_REGEX = /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/[^\s]*)?$/;

interface AiSettingsPanelProps {
  preferences: UiPreferences;
  onChange: (prefs: UiPreferences) => void;
}

export const AiSettingsPanel = ({
  preferences,
  onChange,
}: AiSettingsPanelProps) => {
  const [models, setModels] = useState<string[]>([]);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [endpointError, setEndpointError] = useState<string | null>(null);

  const fetchModels = useCallback(async () => {
    const result = await aiGetStatus();
    if (result.ok) {
      setModels(result.data.models);
      setStatusError(result.data.error);
    } else {
      setModels([]);
      setStatusError(result.error.message);
    }
  }, []);

  useEffect(() => {
    if (preferences.aiEnabled) {
      fetchModels();
    }
  }, [preferences.aiEnabled, preferences.aiEndpoint, fetchModels]);

  const handleEndpointChange = (value: string) => {
    const valid = URL_REGEX.test(value);
    setEndpointError(valid ? null : "Invalid URL. Must use localhost or 127.0.0.1.");
    onChange({ ...preferences, aiEndpoint: value });
  };

  const disabled = !preferences.aiEnabled;

  return (
    <section className="ai-settings-panel">
      <h3>Local AI (Ollama)</h3>

      <label className="ai-toggle-prominent">
        <input
          type="checkbox"
          checked={preferences.aiEnabled}
          onChange={(e) =>
            onChange({ ...preferences, aiEnabled: e.target.checked })
          }
        />
        <strong>Enable Local AI (Ollama)</strong>
      </label>

      <fieldset disabled={disabled}>
        <div className="field-group">
          <label htmlFor="ai-endpoint">Endpoint URL</label>
          <input
            id="ai-endpoint"
            type="text"
            value={preferences.aiEndpoint}
            onChange={(e) => handleEndpointChange(e.target.value)}
            placeholder="http://localhost:11434"
          />
          {endpointError && (
            <span className="field-error">{endpointError}</span>
          )}
        </div>

        <div className="field-group">
          <label htmlFor="ai-model">Model</label>
          <select
            id="ai-model"
            value={preferences.aiModel}
            onChange={(e) =>
              onChange({ ...preferences, aiModel: e.target.value })
            }
          >
            <option value="">— Select a model —</option>
            {models.map((model) => (
              <option key={model} value={model}>
                {model}
              </option>
            ))}
          </select>
          {statusError && (
            <span className="field-error">{statusError}</span>
          )}
        </div>

        <div className="field-group">
          <h4>AI Features</h4>

          <label>
            <input
              type="checkbox"
              checked={preferences.aiSummarizeEnabled}
              onChange={(e) =>
                onChange({
                  ...preferences,
                  aiSummarizeEnabled: e.target.checked,
                })
              }
            />
            Summarisation
          </label>

          <label>
            <input
              type="checkbox"
              checked={preferences.aiQaEnabled}
              onChange={(e) =>
                onChange({ ...preferences, aiQaEnabled: e.target.checked })
              }
            />
            Q&A Chat
          </label>

          <label>
            <input
              type="checkbox"
              checked={preferences.aiAnnotationSuggestEnabled}
              onChange={(e) =>
                onChange({
                  ...preferences,
                  aiAnnotationSuggestEnabled: e.target.checked,
                })
              }
            />
            Annotation Suggestions
          </label>

          <label>
            <input
              type="checkbox"
              checked={preferences.aiEntityExtractEnabled}
              onChange={(e) =>
                onChange({
                  ...preferences,
                  aiEntityExtractEnabled: e.target.checked,
                })
              }
            />
            Entity Extraction
          </label>
        </div>
      </fieldset>
    </section>
  );
};
