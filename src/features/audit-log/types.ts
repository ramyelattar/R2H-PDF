export type AuditAction =
  | "object_created"
  | "object_moved"
  | "object_resized"
  | "object_text_changed"
  | "object_deleted"
  | "object_restored"
  | "comment_resolved"
  | "annotations_committed"
  | "redaction_applied"
  | "project_saved"
  | "project_loaded"
  | "page_deleted"
  | "page_inserted"
  | "page_rotated"
  | "page_moved"
  | "page_duplicated"
  | "pages_extracted";

export interface AuditLogEntry {
  id: string;
  timestamp: number;
  action: AuditAction;
  objectId: string;
  objectType: string;
  pageIndex: number;
  detail: string;
  actor: "user" | "ai" | "system" | "compare";
}
