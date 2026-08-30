import type { StampType } from "../pdf-editor/types";

export interface StampPreset {
  id: StampType;
  label: string;
  color: string;
  defaultText: string;
}

/**
 * Phase 25C: review + engineering stamp presets. These ship as standard
 * appearances in Acrobat; we surface them with sensible defaults and
 * allow the user to add a date and author line.
 */
export const STAMP_PRESETS: StampPreset[] = [
  { id: "APPROVED",            label: "Approved",            color: "#157a3d", defaultText: "APPROVED" },
  { id: "APPROVED_AS_NOTED",   label: "Approved as Noted",   color: "#157a3d", defaultText: "APPROVED AS NOTED" },
  { id: "REVIEWED",            label: "Reviewed",            color: "#1b5fa7", defaultText: "REVIEWED" },
  { id: "REJECTED",            label: "Rejected",            color: "#c52030", defaultText: "REJECTED" },
  { id: "DRAFT",               label: "Draft",               color: "#7a7a7a", defaultText: "DRAFT" },
  { id: "FOR_CONSTRUCTION",    label: "For Construction",    color: "#1f7a4d", defaultText: "FOR CONSTRUCTION" },
  { id: "AS_BUILT",            label: "As Built",            color: "#3b66a6", defaultText: "AS BUILT" },
  { id: "REVISE_AND_RESUBMIT", label: "Revise & Resubmit",   color: "#c75200", defaultText: "REVISE AND RESUBMIT" },
  { id: "CONFIDENTIAL",        label: "Confidential",        color: "#7a1f7d", defaultText: "CONFIDENTIAL" },
  { id: "VOID",                label: "Void",                color: "#8b1a1a", defaultText: "VOID" },
  { id: "CUSTOM",              label: "Custom",              color: "#3a3a3a", defaultText: "" },
];

/**
 * Compose the final stamp text from preset + optional date + optional author.
 * Newlines are preserved by the StampObject rendering and by the export
 * Stamp annotation appearance.
 */
export function composeStampText(opts: {
  baseText: string;
  includeDate?: boolean;
  author?: string;
  customText?: string;
}): string {
  const lines: string[] = [];
  const main = (opts.customText ?? "").trim() || opts.baseText;
  if (main) lines.push(main);
  if (opts.includeDate) {
    const now = new Date();
    const iso = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    lines.push(iso);
  }
  if (opts.author && opts.author.trim().length > 0) {
    lines.push(opts.author.trim());
  }
  return lines.join("\n");
}
