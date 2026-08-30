import type { Citation } from "../rag/types";

export type AiActionType = "add_highlight" | "add_comment" | "add_text_box" | "add_redaction" | "apply_stamp";
export type AiActionStatus = "proposed" | "accepted" | "rejected" | "applied" | "failed";

export interface AiActionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AiProposedAction {
  action_id: string;
  action_type: AiActionType;
  session_id: string;
  page_index: number;
  rect: AiActionRect;
  text: string;
  reason: string;
  confidence: number;
  source_citations: Citation[];
  status: AiActionStatus;
  created_at: number;
}

export interface AiActionBatch {
  batch_id: string;
  session_id: string;
  user_request: string;
  actions: AiProposedAction[];
  citations: Citation[];
  warnings: string[];
  created_at: number;
}
