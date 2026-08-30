import type { EditorTool } from "./types";

interface EditorToolbarProps {
  activeTool: EditorTool;
  canUndo: boolean;
  canRedo: boolean;
  onSetTool: (tool: EditorTool) => void;
  onUndo: () => void;
  onRedo: () => void;
}

const tools: { id: EditorTool; label: string; title: string }[] = [
  { id: "select", label: "↖", title: "Select (V)" },
  { id: "textBox", label: "T", title: "Text Box" },
  { id: "comment", label: "💬", title: "Comment" },
  { id: "highlight", label: "▬", title: "Highlight" },
  { id: "rectangle", label: "□", title: "Rectangle" },
  { id: "redaction", label: "█", title: "Redaction" },
  { id: "stamp", label: "✓", title: "Stamp" },
];

export const EditorToolbar = ({
  activeTool,
  canUndo,
  canRedo,
  onSetTool,
  onUndo,
  onRedo,
}: EditorToolbarProps) => {
  return (
    <div className="editor-toolbar" role="toolbar" aria-label="Editor tools">
      <div className="editor-toolbar__tools">
        {tools.map((tool) => (
          <button
            key={tool.id}
            className={`ghost-btn editor-toolbar__btn ${activeTool === tool.id ? "is-active" : ""}`}
            onClick={() => onSetTool(tool.id)}
            title={tool.title}
            aria-pressed={activeTool === tool.id}
          >
            {tool.label}
          </button>
        ))}
      </div>
      <div className="editor-toolbar__divider" />
      <div className="editor-toolbar__history">
        <button className="ghost-btn editor-toolbar__btn" onClick={onUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">↩</button>
        <button className="ghost-btn editor-toolbar__btn" onClick={onRedo} disabled={!canRedo} title="Redo (Ctrl+Y)">↪</button>
      </div>
    </div>
  );
};
