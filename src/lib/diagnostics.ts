import type { DiagnosticLog } from "../types/shell";

/** The input type for appendDiagnostic (without auto-generated id and timestamp). */
export type DiagnosticEntry = Omit<DiagnosticLog, "id" | "at">;

/** Callback type for appending diagnostics, used across all hooks. */
export type AppendDiagnostic = (entry: DiagnosticEntry) => void;
