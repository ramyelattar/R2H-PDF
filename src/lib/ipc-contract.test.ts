/**
 * IPC Contract Tests
 * 
 * These tests verify that frontend invoke wrappers send the EXACT payload
 * shape expected by Tauri 2 commands.
 * 
 * Tauri 2 rule: Direct command parameters use camelCase (Rust snake_case → JS camelCase).
 * Struct fields inside a request parameter use serde field names (snake_case by default).
 * 
 * The sessionId bug (sending session_id instead of sessionId for direct params)
 * must never be reintroduced.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the @tauri-apps/api/core invoke function
const mockInvoke = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  docExtractAllText,
  goToPage,
  getSessionState,
  renderPage,
  setZoom,
  closeSession,
  docClose,
  docDiagnostics,
  searchIndexDocument,
  searchIndexDocumentWithSpans,
  searchGetIndexStatus,
  searchClearIndex,
  searchSetActivePage,
  annotList,
  annotDelete,
  annotExportFdf,
  editUndo,
  editRedo,
  editGetUndoRedoState,
  aiCancelTask,
  docOpen,
  docRender,
  searchQuery,
  docIncrementalSave,
  libraryLoad,
  libraryRecordExport,
  libraryRecordOpen,
  libraryRecordProject,
  libraryRecordReview,
} from "./ipc";

beforeEach(() => {
  mockInvoke.mockClear();
  mockInvoke.mockResolvedValue(undefined);
});

describe("IPC Contract: Direct parameter commands use camelCase keys", () => {
  it("doc_extract_all_text sends sessionId (not session_id)", async () => {
    await docExtractAllText("test-session-123");
    expect(mockInvoke).toHaveBeenCalledWith("doc_extract_all_text", {
      sessionId: "test-session-123",
    });
  });

  it("go_to_page sends sessionId and pageIndex (not session_id/page_index)", async () => {
    await goToPage("sess-1", 5);
    expect(mockInvoke).toHaveBeenCalledWith("go_to_page", {
      sessionId: "sess-1",
      pageIndex: 5,
    });
  });

  it("get_session_state sends sessionId", async () => {
    await getSessionState("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("get_session_state", {
      sessionId: "sess-1",
    });
  });

  it("render_page sends sessionId, pageIndex, zoom", async () => {
    await renderPage("sess-1", 3, 1.5);
    expect(mockInvoke).toHaveBeenCalledWith("render_page", {
      sessionId: "sess-1",
      pageIndex: 3,
      zoom: 1.5,
    });
  });

  it("set_zoom sends sessionId and zoom", async () => {
    await setZoom("sess-1", 150);
    expect(mockInvoke).toHaveBeenCalledWith("set_zoom", {
      sessionId: "sess-1",
      zoom: 150,
    });
  });

  it("close_session sends sessionId", async () => {
    await closeSession("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("close_session", {
      sessionId: "sess-1",
    });
  });

  it("doc_close sends sessionId", async () => {
    await docClose("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("doc_close", {
      sessionId: "sess-1",
    });
  });

  it("doc_diagnostics sends sessionId", async () => {
    await docDiagnostics("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("doc_diagnostics", {
      sessionId: "sess-1",
    });
  });

  it("search_index_document sends sessionId and pages", async () => {
    await searchIndexDocument("sess-1", ["page 1 text", "page 2 text"]);
    expect(mockInvoke).toHaveBeenCalledWith("search_index_document", {
      sessionId: "sess-1",
      pages: ["page 1 text", "page 2 text"],
    });
  });

  it("search_index_document_with_spans sends sessionId and pages", async () => {
    const pages = [{ text: "hello", spans: [] }];
    await searchIndexDocumentWithSpans("sess-1", pages);
    expect(mockInvoke).toHaveBeenCalledWith("search_index_document_with_spans", {
      sessionId: "sess-1",
      pages,
    });
  });

  it("search_get_index_status sends sessionId", async () => {
    await searchGetIndexStatus("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("search_get_index_status", {
      sessionId: "sess-1",
    });
  });

  it("search_clear_index sends sessionId", async () => {
    await searchClearIndex("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("search_clear_index", {
      sessionId: "sess-1",
    });
  });

  it("search_set_active_page sends sessionId and pageIndex", async () => {
    await searchSetActivePage("sess-1", 7);
    expect(mockInvoke).toHaveBeenCalledWith("search_set_active_page", {
      sessionId: "sess-1",
      pageIndex: 7,
    });
  });

  it("annot_list sends sessionId and pageIndex", async () => {
    await annotList("sess-1", 2);
    expect(mockInvoke).toHaveBeenCalledWith("annot_list", {
      sessionId: "sess-1",
      pageIndex: 2,
    });
  });

  it("annot_delete sends sessionId and annotationId", async () => {
    await annotDelete("sess-1", "annot-abc");
    expect(mockInvoke).toHaveBeenCalledWith("annot_delete", {
      sessionId: "sess-1",
      annotationId: "annot-abc",
    });
  });

  it("annot_export_fdf sends sessionId", async () => {
    await annotExportFdf("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("annot_export_fdf", {
      sessionId: "sess-1",
    });
  });

  it("edit_undo sends sessionId", async () => {
    await editUndo("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("edit_undo", {
      sessionId: "sess-1",
    });
  });

  it("edit_redo sends sessionId", async () => {
    await editRedo("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("edit_redo", {
      sessionId: "sess-1",
    });
  });

  it("edit_get_undo_redo_state sends sessionId", async () => {
    await editGetUndoRedoState("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("edit_get_undo_redo_state", {
      sessionId: "sess-1",
    });
  });

  it("ai_cancel_task sends taskId (not task_id)", async () => {
    await aiCancelTask("task-xyz");
    expect(mockInvoke).toHaveBeenCalledWith("ai_cancel_task", {
      taskId: "task-xyz",
    });
  });
});

describe("IPC Contract: Struct-wrapped commands use snake_case inside request", () => {
  it("doc_open wraps request struct with snake_case fields", async () => {
    await docOpen({ path: "/test.pdf", recover_if_damaged: false });
    expect(mockInvoke).toHaveBeenCalledWith("doc_open", {
      request: { path: "/test.pdf", recover_if_damaged: false },
    });
  });

  it("doc_render wraps request struct with snake_case fields", async () => {
    const req = {
      session_id: "s1",
      page_index: 0,
      zoom: 1.0,
      viewport: { x: 0, y: 0, width: 800, height: 600 },
      device_pixel_ratio: 2.0,
    };
    await docRender(req);
    expect(mockInvoke).toHaveBeenCalledWith("doc_render", { request: req });
  });

  it("search_query wraps query struct with snake_case fields", async () => {
    const q = {
      session_id: "s1",
      query: "test",
      scope: "AllPages" as const,
      case_sensitive: false,
      whole_words: false,
      use_regex: false,
      max_results: 100,
    };
    await searchQuery(q);
    expect(mockInvoke).toHaveBeenCalledWith("search_query", { query: q });
  });

  it("doc_incremental_save wraps request struct with snake_case fields", async () => {
    const req = { session_id: "s1", target_path: null, fsync: true };
    await docIncrementalSave(req);
    expect(mockInvoke).toHaveBeenCalledWith("doc_incremental_save", { request: req });
  });
});


describe("IPC Contract: Phase 22 commands", () => {
  it("doc_list_form_fields sends sessionId", async () => {
    const { docListFormFields } = await import("./ipc");
    await docListFormFields("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("doc_list_form_fields", {
      sessionId: "sess-1",
    });
  });

  it("pdf_revert_content_edit sends sessionId and editId", async () => {
    const { pdfRevertContentEdit } = await import("./ipc");
    await pdfRevertContentEdit("sess-1", "edit-123");
    expect(mockInvoke).toHaveBeenCalledWith("pdf_revert_content_edit", {
      sessionId: "sess-1",
      editId: "edit-123",
    });
  });

  it("pdf_clear_content_edit_history sends sessionId", async () => {
    const { pdfClearContentEditHistory } = await import("./ipc");
    await pdfClearContentEditHistory("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("pdf_clear_content_edit_history", {
      sessionId: "sess-1",
    });
  });

  it("pdf_get_page_content_objects sends sessionId and pageIndex", async () => {
    const { pdfGetPageContentObjects } = await import("./ipc");
    await pdfGetPageContentObjects("sess-1", 3);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_get_page_content_objects", {
      sessionId: "sess-1",
      pageIndex: 3,
    });
  });

  it("pdf_list_content_edits sends sessionId", async () => {
    const { pdfListContentEdits } = await import("./ipc");
    await pdfListContentEdits("sess-1");
    expect(mockInvoke).toHaveBeenCalledWith("pdf_list_content_edits", {
      sessionId: "sess-1",
    });
  });

  it("pdf_apply_native_text_edit wraps request struct", async () => {
    const { pdfApplyNativeTextEdit } = await import("./ipc");
    const req = {
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-1",
      replacement_text: "New text",
      preserve_style: true,
    };
    await pdfApplyNativeTextEdit(req);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_apply_native_text_edit", { request: req });
  });

  it("pdf_preview_find_replace and pdf_apply_find_replace wrap request structs", async () => {
    const { pdfPreviewFindReplace, pdfApplyFindReplace } = await import("./ipc");
    const previewReq = {
      session_id: "s1",
      current_page_index: 0,
      find_text: "old",
      replace_text: "new",
      scope: "current_page" as const,
      case_sensitive: false,
      whole_word: true,
    };
    await pdfPreviewFindReplace(previewReq);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_preview_find_replace", { request: previewReq });

    const applyReq = {
      session_id: "s1",
      replace_text: "new",
      matches: [{ id: "fr-1", page_index: 0, content_object_id: "co-1" }],
    };
    await pdfApplyFindReplace(applyReq);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_apply_find_replace", { request: applyReq });
  });

  it("pdf_move_native_image wraps request struct", async () => {
    const { pdfMoveNativeImage } = await import("./ipc");
    const req = {
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-img-1",
      new_rect: [10, 20, 200, 300] as [number, number, number, number],
    };
    await pdfMoveNativeImage(req);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_move_native_image", { request: req });
  });

  it("pdf_replace_native_image forwards export layout mode", async () => {
    const { pdfReplaceNativeImage } = await import("./ipc");
    const req = {
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-img-1",
      image_bytes_base64: "abc",
      layout_mode: "fill" as const,
    };
    await pdfReplaceNativeImage(req);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_replace_native_image", { request: req });
  });

  it("pdf_crop_native_image and pdf_rotate_native_image wrap request structs", async () => {
    const { pdfCropNativeImage, pdfRotateNativeImage } = await import("./ipc");
    const cropReq = {
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-img-1",
      crop_rect: [10, 20, 100, 120] as [number, number, number, number],
      target_rect: [0, 0, 200, 200] as [number, number, number, number],
    };
    await pdfCropNativeImage(cropReq);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_crop_native_image", { request: cropReq });

    const rotateReq = {
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-img-1",
      degrees: 90 as const,
    };
    await pdfRotateNativeImage(rotateReq);
    expect(mockInvoke).toHaveBeenCalledWith("pdf_rotate_native_image", { request: rotateReq });
  });
});

describe("IPC Contract: Phase 2 shell library commands", () => {
  it("loads the backend-owned library without renderer record arguments", async () => {
    await libraryLoad();
    expect(mockInvoke).toHaveBeenCalledWith("library_load", undefined);
  });

  it("records a successfully opened PDF with a typed request struct", async () => {
    await libraryRecordOpen({
      source_path: "C:\\docs\\source.pdf",
      workspace_id: "workspace-1",
      page_count: 3,
    });
    expect(mockInvoke).toHaveBeenCalledWith("library_record_open", {
      request: {
        source_path: "C:\\docs\\source.pdf",
        workspace_id: "workspace-1",
        page_count: 3,
      },
    });
  });

  it("records a project only after the authoritative project lifecycle succeeds", async () => {
    await libraryRecordProject({
      source_path: "C:\\docs\\source.pdf",
      project_name: "Source",
      workspace_id: null,
      page_count: 3,
    });
    expect(mockInvoke).toHaveBeenCalledWith("library_record_project", {
      request: {
        source_path: "C:\\docs\\source.pdf",
        project_name: "Source",
        workspace_id: null,
        page_count: 3,
      },
    });
  });

  it("records validated export and completed review metadata with typed requests", async () => {
    await libraryRecordExport({
      source_path: "C:\\docs\\source.pdf",
      destination: "C:\\docs\\export.pdf",
      export_type: "pdf",
      timestamp_epoch_ms: 42,
      size_bytes: 10,
      sha256: "abc",
      page_count: 3,
      included_overlays: 2,
      warnings: [],
    });
    expect(mockInvoke).toHaveBeenCalledWith("library_record_export", {
      request: {
        source_path: "C:\\docs\\source.pdf",
        destination: "C:\\docs\\export.pdf",
        export_type: "pdf",
        timestamp_epoch_ms: 42,
        size_bytes: 10,
        sha256: "abc",
        page_count: 3,
        included_overlays: 2,
        warnings: [],
      },
    });

    await libraryRecordReview({
      source_path: "C:\\docs\\source.pdf",
      review_id: "review-1",
      status: "in-review",
    });
    expect(mockInvoke).toHaveBeenCalledWith("library_record_review", {
      request: {
        source_path: "C:\\docs\\source.pdf",
        review_id: "review-1",
        status: "in-review",
      },
    });
  });
});
