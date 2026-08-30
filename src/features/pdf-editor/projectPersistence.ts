/**
 * Non-destructive project sidecar persistence for editor overlay state.
 * Serializes/deserializes editor objects to a versioned JSON format.
 */

import type { EditorObject, EditorObjectType } from "./types";
import type { ProjectSaveRequest } from "../../lib/ipc";
import type { DocumentTab } from "../../types/shell";

export const SCHEMA_VERSION = 2;

const KNOWN_TYPES: EditorObjectType[] = [
  "textBox", "comment", "highlight", "rectangle", "redaction", "stamp", "image",
  "strikethrough", "underline",
];

export interface ProjectDocument {
  path: string;
  fingerprint: string;
  pageCount: number;
}

export interface ProjectData {
  schemaVersion: number;
  app: string;
  projectId?: string;
  projectName?: string;
  migrationVersion?: number;
  createdAt?: number;
  lastModifiedAt?: number;
  document: ProjectDocument;
  editor: {
    objects: EditorObject[];
  };
  audit: AuditEntry[];
  reviewState?: unknown | null;
  viewState?: unknown | null;
  integritySha256?: string;
}

export interface AuditEntry {
  timestamp: number;
  action: string;
  objectId: string;
  detail: string;
}

/**
 * Serialize editor objects and audit log into a project sidecar JSON string.
 */
export function serializeProject(
  document: ProjectDocument,
  objects: EditorObject[],
  audit: AuditEntry[] = [],
): string {
  const project: ProjectData = {
    schemaVersion: SCHEMA_VERSION,
    app: "R2H PDF AI Workstation",
    document,
    editor: { objects },
    audit,
  };
  return JSON.stringify(project, null, 2);
}

/**
 * Deserialize a project sidecar JSON string.
 * Returns null if the schema version is unsupported.
 * Silently filters out objects with unknown types.
 */
export function deserializeProject(json: string): ProjectData | null {
  try {
    const parsed = JSON.parse(json) as Partial<ProjectData>;

    if (!parsed.schemaVersion || parsed.schemaVersion > SCHEMA_VERSION) {
      return null; // Unsupported future version.
    }

    if (!parsed.editor || !Array.isArray(parsed.editor.objects)) {
      return null;
    }

    // Filter out unknown object types safely.
    const validObjects = parsed.editor.objects.filter(
      (obj) => obj && typeof obj === "object" && KNOWN_TYPES.includes(obj.type),
    );

    return {
      schemaVersion: parsed.schemaVersion,
      app: parsed.app ?? "unknown",
      projectId: parsed.projectId,
      projectName: parsed.projectName,
      migrationVersion: parsed.migrationVersion,
      createdAt: parsed.createdAt,
      lastModifiedAt: parsed.lastModifiedAt,
      document: parsed.document ?? { path: "", fingerprint: "", pageCount: 0 },
      editor: { objects: validObjects },
      audit: Array.isArray(parsed.audit) ? parsed.audit : [],
      reviewState: parsed.reviewState ?? null,
      viewState: parsed.viewState ?? null,
      integritySha256: parsed.integritySha256,
    };
  } catch {
    return null;
  }
}

/**
 * Strip the volatile backend session id before crossing the persistence
 * boundary. Overlay ids, page coordinates, timestamps, and metadata remain
 * authoritative; the active session id is restored when the document opens.
 */
export function toPersistedOverlay(object: EditorObject): EditorObject {
  return { ...object, sessionId: "" } as EditorObject;
}

export function restoreOverlaySession(object: EditorObject, sessionId: string): EditorObject {
  return { ...object, sessionId } as EditorObject;
}

export function createProjectSaveRequest(
  tab: DocumentTab,
  objects: EditorObject[],
): ProjectSaveRequest {
  return {
    source_path: tab.sourcePath,
    project_name: tab.title,
    workspace_id: tab.workspaceId,
    page_count: tab.totalPages,
    overlay_objects: objects.map(toPersistedOverlay),
    review_state: null,
    view_state: {
      page: tab.page,
      zoom: tab.zoom,
      rotation: tab.rotation ?? 0,
    },
  };
}

export function restoreProjectObjects(
  project: { documents: Array<{ document_id: string; overlay_objects: unknown[] }> },
  documentId: string,
  sessionId: string,
): EditorObject[] {
  const document = project.documents.find((entry) => entry.document_id === documentId);
  if (!document) return [];
  return document.overlay_objects.flatMap((value) => {
    if (!value || typeof value !== "object") return [];
    const object = value as EditorObject;
    if (typeof object.id !== "string" || typeof object.type !== "string") return [];
    return [restoreOverlaySession(object, sessionId)];
  });
}
