import { useMemo, useState } from "react";
import type { CommandItem } from "../../types/shell";

interface CommandPaletteProps {
  open: boolean;
  commands: CommandItem[];
  onClose: () => void;
  onExecute: (command: CommandItem) => void;
  isCommandAvailable?: (command: CommandItem) => boolean;
}

export const CommandPalette = ({
  open,
  commands,
  onClose,
  onExecute,
  isCommandAvailable,
}: CommandPaletteProps) => {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const lowered = query.trim().toLowerCase();
    if (!lowered) {
      return commands;
    }

    return commands.filter((entry) =>
      `${entry.title} ${entry.description} ${entry.category}`
        .toLowerCase()
        .includes(lowered),
    );
  }, [commands, query]);

  if (!open) {
    return null;
  }

  return (
    <div className="overlay-root" role="dialog" aria-modal="true">
      <div className="palette">
        <div className="palette__header">
          <input
            autoFocus
            className="palette__input"
            value={query}
            placeholder="Search commands, files, workflows…"
            onChange={(event) => setQuery(event.target.value)}
          />
          <button className="ghost-btn" onClick={onClose}>
            Esc
          </button>
        </div>

        <div className="palette__results">
          {filtered.map((entry) => (
            <button
              key={entry.id}
              className="palette__item"
              disabled={isCommandAvailable ? !isCommandAvailable(entry) : false}
              aria-disabled={isCommandAvailable ? !isCommandAvailable(entry) : undefined}
              onClick={() => {
                if (isCommandAvailable && !isCommandAvailable(entry)) return;
                onExecute(entry);
              }}
            >
              <div>
                <strong>{entry.title}</strong>
                <p>{entry.description}{entry.requiresDocument && isCommandAvailable && !isCommandAvailable(entry) ? " · Open a PDF first" : ""}</p>
              </div>
              <div className="palette__meta">
                <span>{entry.category}</span>
                <kbd>{entry.shortcut}</kbd>
              </div>
            </button>
          ))}
          {!filtered.length && (
            <div className="palette__empty">No matching command for “{query}”.</div>
          )}
        </div>
      </div>
    </div>
  );
};
