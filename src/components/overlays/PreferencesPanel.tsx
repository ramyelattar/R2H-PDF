import type { UiPreferences } from "../../types/shell";
import { AiSettingsPanel } from "./AiSettingsPanel";

interface PreferencesPanelProps {
  open: boolean;
  value: UiPreferences;
  onClose: () => void;
  onChange: (next: UiPreferences) => void;
}

export const PreferencesPanel = ({
  open,
  value,
  onClose,
  onChange,
}: PreferencesPanelProps) => {
  if (!open) {
    return null;
  }

  return (
    <aside className="slide-panel" role="dialog" aria-modal="true">
      <header>
        <h2>Preferences</h2>
        <button className="ghost-btn" onClick={onClose}>
          Close
        </button>
      </header>

      <section>
        <h3>Workstation</h3>
        <label>
          <input
            type="checkbox"
            checked={value.compactDensity}
            onChange={(event) =>
              onChange({
                ...value,
                compactDensity: event.target.checked,
              })
            }
          />
          Compact density mode
        </label>
        <label>
          <input
            type="checkbox"
            checked={value.showLeftPanel}
            onChange={(event) =>
              onChange({
                ...value,
                showLeftPanel: event.target.checked,
              })
            }
          />
          Show left panel by default
        </label>
        <label>
          <input
            type="checkbox"
            checked={value.showRightInspector}
            onChange={(event) =>
              onChange({
                ...value,
                showRightInspector: event.target.checked,
              })
            }
          />
          Show right inspector by default
        </label>
        <label>
          <input
            type="checkbox"
            checked={value.restoreLastSession}
            onChange={(event) =>
              onChange({
                ...value,
                restoreLastSession: event.target.checked,
              })
            }
          />
          Restore last session on launch
        </label>
      </section>

      <section>
        <h3>Diagnostics detail</h3>
        <div className="segmented">
          {(["minimal", "standard", "verbose"] as const).map((mode) => (
            <button
              key={mode}
              className={value.telemetryMode === mode ? "is-active" : ""}
              onClick={() => onChange({ ...value, telemetryMode: mode })}
            >
              {mode}
            </button>
          ))}
        </div>
      </section>

      <AiSettingsPanel preferences={value} onChange={onChange} />
    </aside>
  );
};
