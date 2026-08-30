import { useEffect, useRef, useState } from "react";
import type { TextBlock, BlockEditStrategy, BlockOverflowPolicy } from "../../lib/ipc";

/**
 * Phase 30E — in-canvas inline editor for a multi-line block.
 *
 * Floats a multi-line textarea over the block bbox with a strategy
 * selector (Auto / Force native multi-op / Force visual reflow). Honest
 * warning chip when the user picks visual reflow.
 */

export interface InlineBlockEditorProps {
  block: TextBlock;
  pageHeightPts: number;
  zoom: number;
  onApply: (replacement: string, strategy: BlockEditStrategy, overflowPolicy: BlockOverflowPolicy) => void | Promise<void>;
  onCancel: () => void;
}

export function InlineBlockEditor({
  block,
  pageHeightPts,
  zoom,
  onApply,
  onCancel,
}: InlineBlockEditorProps) {
  const [text, setText] = useState(block.combined_text);
  const [strategy, setStrategy] = useState<BlockEditStrategy>("auto");
  const [overflowPolicy, setOverflowPolicy] = useState<BlockOverflowPolicy>("reject");
  const ref = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  // PDF → screen coords. block.bbox = [x0, y0, x1, y1] (Y up).
  const left = block.bbox[0] * zoom;
  const top = (pageHeightPts - block.bbox[3]) * zoom;
  const width = Math.max(80, (block.bbox[2] - block.bbox[0]) * zoom);
  const height = Math.max(40, (block.bbox[3] - block.bbox[1]) * zoom);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
      return;
    }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void onApply(text, strategy, overflowPolicy);
      return;
    }
    // Allow plain Enter as a newline in block mode (no preventDefault).
  };

  return (
    <div
      data-testid="inline-block-editor"
      style={{
        position: "absolute",
        left,
        top,
        width,
        minHeight: height,
        zIndex: 60,
        background: "rgba(255,255,255,0.96)",
        border: "2px solid #74a2ff",
        borderRadius: 3,
        boxShadow: "0 2px 8px rgba(0,0,0,0.2)",
        padding: 4,
        display: "flex",
        flexDirection: "column",
        gap: 4,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, flexWrap: "wrap" }}>
        <span className="badge badge-info">Block edit</span>
        <span>{block.member_ids.length} line(s)</span>
        <select
          value={strategy}
          onChange={(e) => setStrategy(e.target.value as BlockEditStrategy)}
          data-testid="inline-block-strategy"
          style={{ fontSize: 11 }}
        >
          <option value="auto">Auto</option>
          <option value="native_multi_operator">Force native multi-op</option>
          <option value="visual_reflow">Force visual reflow</option>
        </select>
        <select
          value={overflowPolicy}
          onChange={(e) => setOverflowPolicy(e.target.value as BlockOverflowPolicy)}
          data-testid="inline-block-overflow-policy"
          style={{ fontSize: 11 }}
          title="How to handle wrapped text that exceeds the original block height."
        >
          <option value="reject">Warn / do not apply if overflow</option>
          <option value="shrink_to_fit">Fit by shrinking slightly</option>
          <option value="allow_overflow">Allow visual overflow</option>
        </select>
        {strategy === "visual_reflow" && (
          <span className="badge badge-warning" data-testid="inline-block-visual-warning">
            Visual reflow ≠ Acrobat paragraph reflow
          </span>
        )}
      </div>
      <textarea
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        rows={Math.max(2, block.member_ids.length + 1)}
        data-testid="inline-block-textarea"
        style={{
          width: "100%",
          fontSize: Math.max(11, block.font_size * zoom * 0.9),
          resize: "none",
          border: "1px solid #ccc",
          padding: 2,
          fontFamily: "inherit",
        }}
      />
      <div style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
        <button
          className="ghost-btn"
          onClick={() => onCancel()}
          data-testid="inline-block-cancel"
        >
          Cancel
        </button>
        <button
          className="ghost-btn"
          onClick={() => void onApply(text, strategy, overflowPolicy)}
          disabled={text === block.combined_text}
          data-testid="inline-block-apply"
        >
          Apply Block
        </button>
      </div>
    </div>
  );
}
