import { useCallback, useRef, useState } from "react";
import type { RenderResponse } from "../lib/ipc";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { DocumentTab } from "../types/shell";

interface UseRenderLifecycleDeps {
  activeTab: DocumentTab | null;
  setTabLoadState: (tabId: string, loadState: DocumentTab["loadState"], errorMessage?: string) => void;
  appendDiagnostic: AppendDiagnostic;
}

export function useRenderLifecycle(deps: UseRenderLifecycleDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [lastRenderMs, setLastRenderMs] = useState<number | null>(null);

  const onRenderSuccess = useCallback((render: RenderResponse) => {
    const { activeTab, setTabLoadState, appendDiagnostic } = depsRef.current;
    if (activeTab && activeTab.loadState === "loading") {
      setTabLoadState(activeTab.id, "ready");
    }
    setLastRenderMs(render.render_time_ms);
    appendDiagnostic({
      level: "INFO",
      source: "ipc",
      message: `render_page success: p.${render.page_index + 1} ${render.width_px}x${render.height_px} @ ${Math.round(render.zoom * 100)}% in ${render.render_time_ms}ms`,
    });
  }, []);

  const onRenderError = useCallback((message: string) => {
    const { activeTab, setTabLoadState, appendDiagnostic } = depsRef.current;
    if (activeTab && activeTab.loadState === "loading") {
      setTabLoadState(activeTab.id, "error", message);
    }
    appendDiagnostic({ level: "ERROR", source: "ipc", message: `Render failed: ${message}` });
  }, []);

  return { lastRenderMs, onRenderSuccess, onRenderError };
}
