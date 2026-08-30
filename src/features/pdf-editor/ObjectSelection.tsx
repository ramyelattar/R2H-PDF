import { useCallback, useRef } from "react";
import type { EditorRect } from "./types";

interface ObjectSelectionProps {
  rect: EditorRect;
  locked: boolean;
  onMove: (newRect: EditorRect) => void;
  onResize: (newRect: EditorRect) => void;
}

type HandlePosition = "nw" | "ne" | "sw" | "se" | "n" | "s" | "e" | "w";

const HANDLE_SIZE = 8;

/**
 * Renders selection outline and resize handles around a selected editor object.
 * Supports drag-to-move and handle-drag-to-resize.
 * All coordinates are in screen space (CSS pixels).
 */
export const ObjectSelection = ({ rect, locked, onMove, onResize }: ObjectSelectionProps) => {
  const dragRef = useRef<{
    type: "move" | "resize";
    handle?: HandlePosition;
    startX: number;
    startY: number;
    startRect: EditorRect;
  } | null>(null);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent, type: "move" | "resize", handle?: HandlePosition) => {
      if (locked) return;
      e.stopPropagation();
      e.preventDefault();
      const el = e.currentTarget as HTMLElement;
      el.setPointerCapture(e.pointerId);
      dragRef.current = { type, handle, startX: e.clientX, startY: e.clientY, startRect: { ...rect } };
    },
    [rect, locked],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      const dx = e.clientX - drag.startX;
      const dy = e.clientY - drag.startY;

      if (drag.type === "move") {
        onMove({
          x: drag.startRect.x + dx,
          y: drag.startRect.y + dy,
          width: drag.startRect.width,
          height: drag.startRect.height,
        });
      } else if (drag.type === "resize" && drag.handle) {
        const newRect = computeResize(drag.startRect, drag.handle, dx, dy);
        onResize(newRect);
      }
    },
    [onMove, onResize],
  );

  const handlePointerUp = useCallback((e: React.PointerEvent) => {
    const el = e.currentTarget as HTMLElement;
    el.releasePointerCapture(e.pointerId);
    dragRef.current = null;
  }, []);

  const handles: { pos: HandlePosition; style: React.CSSProperties }[] = [
    { pos: "nw", style: { top: -HANDLE_SIZE / 2, left: -HANDLE_SIZE / 2, cursor: "nwse-resize" } },
    { pos: "ne", style: { top: -HANDLE_SIZE / 2, right: -HANDLE_SIZE / 2, cursor: "nesw-resize" } },
    { pos: "sw", style: { bottom: -HANDLE_SIZE / 2, left: -HANDLE_SIZE / 2, cursor: "nesw-resize" } },
    { pos: "se", style: { bottom: -HANDLE_SIZE / 2, right: -HANDLE_SIZE / 2, cursor: "nwse-resize" } },
    { pos: "n", style: { top: -HANDLE_SIZE / 2, left: "50%", marginLeft: -HANDLE_SIZE / 2, cursor: "ns-resize" } },
    { pos: "s", style: { bottom: -HANDLE_SIZE / 2, left: "50%", marginLeft: -HANDLE_SIZE / 2, cursor: "ns-resize" } },
    { pos: "e", style: { top: "50%", right: -HANDLE_SIZE / 2, marginTop: -HANDLE_SIZE / 2, cursor: "ew-resize" } },
    { pos: "w", style: { top: "50%", left: -HANDLE_SIZE / 2, marginTop: -HANDLE_SIZE / 2, cursor: "ew-resize" } },
  ];

  return (
    <div
      className="editor-selection"
      style={{
        position: "absolute",
        left: rect.x,
        top: rect.y,
        width: rect.width,
        height: rect.height,
        pointerEvents: "auto",
      }}
      onPointerDown={(e) => handlePointerDown(e, "move")}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
    >
      {/* Selection border */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          border: "2px solid #74a2ff",
          borderRadius: 2,
          pointerEvents: "none",
        }}
      />
      {/* Resize handles */}
      {!locked &&
        handles.map(({ pos, style }) => (
          <div
            key={pos}
            style={{
              position: "absolute",
              width: HANDLE_SIZE,
              height: HANDLE_SIZE,
              background: "#fff",
              border: "1.5px solid #74a2ff",
              borderRadius: 2,
              ...style,
            }}
            onPointerDown={(e) => handlePointerDown(e, "resize", pos)}
          />
        ))}
    </div>
  );
};

function computeResize(start: EditorRect, handle: HandlePosition, dx: number, dy: number): EditorRect {
  let { x, y, width, height } = start;
  const minSize = 10;

  switch (handle) {
    case "se": width = Math.max(minSize, width + dx); height = Math.max(minSize, height + dy); break;
    case "sw": x += dx; width = Math.max(minSize, width - dx); height = Math.max(minSize, height + dy); break;
    case "ne": width = Math.max(minSize, width + dx); y += dy; height = Math.max(minSize, height - dy); break;
    case "nw": x += dx; width = Math.max(minSize, width - dx); y += dy; height = Math.max(minSize, height - dy); break;
    case "e": width = Math.max(minSize, width + dx); break;
    case "w": x += dx; width = Math.max(minSize, width - dx); break;
    case "s": height = Math.max(minSize, height + dy); break;
    case "n": y += dy; height = Math.max(minSize, height - dy); break;
  }

  return { x, y, width, height };
}
