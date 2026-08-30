import { useEffect, useRef } from "react";
import { docIncrementalSave } from "../lib/ipc";

export interface UseAutosaveOptions {
  /** The session ID of the currently open PDF document. */
  sessionId: string;
  /** Whether the session has unsaved changes. */
  isDirty: boolean;
  /** When false the timer is not started (e.g. no active session). */
  enabled: boolean;
  /** Optional authoritative save operation that also persists project state. */
  save?: () => Promise<{ ok: boolean; savedTo?: string; error?: string }>;
  /** Interval in milliseconds between autosave attempts. Defaults to 5 minutes. */
  intervalMs?: number;
  /**
   * Called when an autosave completes successfully.
   * Receives the path the document was saved to.
   * Req 17.5
   */
  onSuccess?: (savedTo: string) => void;
  /**
   * Called when an autosave fails.
   * Receives the human-readable error message.
   * Req 17.6
   */
  onError?: (message: string) => void;
}

/**
 * Starts a periodic autosave timer for the given session.
 *
 * - The timer fires every `intervalMs` milliseconds (default 300 000 = 5 min).
 * - On each tick, if `isDirty` is true, `doc_incremental_save` is called with
 *   `target_path: null` and `fsync: false`.
 * - If `isDirty` is false the tick is skipped without an IPC call.
 * - The interval is cleared when the component unmounts or `sessionId` changes.
 *
 * Req 17.1–17.6
 */
export const useAutosave = ({
  sessionId,
  isDirty,
  enabled,
  intervalMs = 300_000,
  onSuccess,
  onError,
  save,
}: UseAutosaveOptions): void => {
  // Keep a stable ref to the latest isDirty value so the interval callback
  // always reads the current value without needing to be recreated on every
  // render cycle.
  const isDirtyRef = useRef(isDirty);
  isDirtyRef.current = isDirty;

  // Keep stable refs to the callbacks so the interval is not recreated when
  // the parent component re-renders with new inline function references.
  const onSuccessRef = useRef(onSuccess);
  onSuccessRef.current = onSuccess;

  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;
  const saveRef = useRef(save);
  saveRef.current = save;

  useEffect(() => {
    // Req 17.1: only start the timer when a session is active and enabled.
    if (!enabled || !sessionId) return;

    const id = setInterval(async () => {
      // Req 17.3: skip the save call when the document is not dirty.
      if (!isDirtyRef.current) return;

      // Req 17.2: call doc_incremental_save when dirty.
      const result = saveRef.current
        ? await saveRef.current()
        : await docIncrementalSave({
            session_id: sessionId,
            target_path: null,
            fsync: false,
          }).then((nativeResult) => nativeResult.ok
            ? { ok: true, savedTo: nativeResult.data.saved_to }
            : { ok: false, error: nativeResult.error.message });

      if (result.ok) {
        // Req 17.5: notify caller on success so it can update lastSavedAt.
        onSuccessRef.current?.(result.savedTo ?? sessionId);
      } else {
        // Req 17.6: notify caller on failure; do not close the session.
        onErrorRef.current?.(result.error ?? "Autosave failed");
      }
    }, intervalMs);

    // Req 17.4: clear the timer when the session changes or the component unmounts.
    return () => clearInterval(id);
  }, [sessionId, enabled, intervalMs]);
};
