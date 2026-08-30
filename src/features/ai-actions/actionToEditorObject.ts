/**
 * Converts AI proposed actions into editor overlay objects.
 * Each action type maps to a specific EditorObject subtype.
 */

import type {
  EditorObject,
  TextBoxObject,
  CommentObject,
  HighlightObject,
  RedactionObject,
  StampObject,
  EditorRect,
} from "../pdf-editor/types";
import type { AiProposedAction } from "./types";

export interface ApplyResult {
  applied: EditorObject[];
  skipped: { actionId: string; reason: string }[];
}

/**
 * Convert a batch of accepted AI actions into editor objects.
 * Validates each action before conversion. Skips invalid ones with reasons.
 */
export function applyAiActionsToEditorObjects(
  actions: AiProposedAction[],
  activeSessionId: string,
  pageCount: number,
): ApplyResult {
  const applied: EditorObject[] = [];
  const skipped: { actionId: string; reason: string }[] = [];

  for (const action of actions) {
    // Validate before applying.
    const skipReason = validateForApply(action, activeSessionId, pageCount);
    if (skipReason) {
      skipped.push({ actionId: action.action_id, reason: skipReason });
      continue;
    }

    const obj = aiActionToEditorObject(action);
    if (obj) {
      applied.push(obj);
    } else {
      skipped.push({ actionId: action.action_id, reason: `Unsupported action_type: ${action.action_type}` });
    }
  }

  return { applied, skipped };
}

/**
 * Validate an action is safe to apply. Returns null if valid, or a skip reason.
 */
function validateForApply(action: AiProposedAction, activeSessionId: string, pageCount: number): string | null {
  if (action.status !== "accepted" && action.status !== "applied") {
    return `Action status is "${action.status}", not accepted`;
  }
  if (action.session_id !== activeSessionId) {
    return `Action session "${action.session_id}" does not match active session "${activeSessionId}"`;
  }
  if (action.page_index >= pageCount) {
    return `Page ${action.page_index} is out of bounds (document has ${pageCount} pages)`;
  }
  if (action.rect.width <= 0 || action.rect.height <= 0) {
    return `Invalid rect: width=${action.rect.width}, height=${action.rect.height}`;
  }
  return null;
}

/**
 * Convert a single AI action to an EditorObject.
 */
function aiActionToEditorObject(action: AiProposedAction): EditorObject | null {
  const now = Date.now();
  const id = `ai-obj-${action.action_id}`;
  const rect: EditorRect = {
    x: action.rect.x,
    y: action.rect.y,
    width: action.rect.width,
    height: action.rect.height,
  };

  const baseMetadata: Record<string, unknown> = {
    ai: true,
    actionId: action.action_id,
    confidence: action.confidence,
    reason: action.reason,
    citations: action.source_citations,
  };

  const base = {
    id,
    sessionId: action.session_id,
    pageIndex: action.page_index,
    rect,
    rotation: 0,
    zIndex: 100 + Math.floor(Math.random() * 50),
    locked: false,
    hidden: false,
    createdAt: now,
    updatedAt: now,
    metadata: baseMetadata,
  };

  switch (action.action_type) {
    case "add_highlight":
      return {
        ...base,
        type: "highlight",
        color: "rgba(255,255,0,0.4)",
        opacity: 0.35,
        contents: action.reason || action.text,
      } as HighlightObject;

    case "add_comment":
      return {
        ...base,
        type: "comment",
        rect: { ...rect, width: Math.max(rect.width, 24), height: Math.max(rect.height, 24) },
        contents: action.text || action.reason,
        author: "R2H Local AI",
        color: "#74a2ff",
        status: "open",
      } as CommentObject;

    case "add_text_box":
      return {
        ...base,
        type: "textBox",
        text: action.text || action.reason,
        fontSize: 11,
        fontFamily: "Helvetica",
        color: "#000",
        backgroundColor: "rgba(255,255,255,0.95)",
        borderColor: "rgba(116,162,255,0.6)",
      } as TextBoxObject;

    case "add_redaction":
      return {
        ...base,
        type: "redaction",
        fillColor: "#000",
        replacementText: action.text || "",
        reason: action.reason,
        status: "draft", // Never burn in automatically.
      } as RedactionObject;

    case "apply_stamp":
      return {
        ...base,
        type: "stamp",
        stampText: action.text || "REVIEWED",
        stampType: deriveStampType(action.text),
        color: "#c00",
      } as StampObject;

    default:
      return null;
  }
}

function deriveStampType(text: string | undefined): "APPROVED" | "APPROVED_AS_NOTED" | "REJECTED" | "REVIEWED" | "DRAFT" | "CUSTOM" {
  if (!text) return "REVIEWED";
  const upper = text.toUpperCase();
  if (upper.includes("APPROVED") && upper.includes("NOTED")) return "APPROVED_AS_NOTED";
  if (upper.includes("APPROVED")) return "APPROVED";
  if (upper.includes("REJECTED")) return "REJECTED";
  if (upper.includes("DRAFT")) return "DRAFT";
  if (upper.includes("REVIEWED")) return "REVIEWED";
  return "CUSTOM";
}
