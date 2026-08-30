import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../src/lib/sessionRestore.ts", import.meta.url), "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;

const moduleUrl = `data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`;
const { prepareRestoredSession, isPendingReopenTabId, PENDING_REOPEN_PREFIX } =
  await import(moduleUrl);

test("stale backend PDF tabs are reopened with non-live placeholder ids", () => {
  const snapshot = {
    activeWorkspaceId: "ws-1",
    activeTabId: "doc-session-1",
    currentMode: "read",
    isLeftSidebarOpen: false,
    isRightSidebarOpen: false,
    diagnosticsOpen: false,
    preferences: {
      compactDensity: true,
      showRightInspector: true,
      showLeftPanel: true,
      restoreLastSession: true,
      telemetryMode: "standard",
    },
    openTabs: [
      {
        id: "doc-session-1",
        title: "contract.pdf",
        kind: "pdf",
        workspaceId: "ws-1",
        sourcePath: "E:/docs/contract.pdf",
        pinned: false,
        dirty: true, // intentionally dirty in snapshot — must be reset on restore
        page: 3,
        totalPages: 10,
        zoom: 150,
        loadState: "ready",
      },
      {
        id: "tab-review",
        title: "Review",
        kind: "review",
        workspaceId: "ws-1",
        sourcePath: "session://review",
        pinned: false,
        dirty: false,
        page: 0,
        totalPages: 0,
        zoom: 100,
        loadState: "ready",
      },
    ],
  };

  const result = prepareRestoredSession(snapshot);

  // Path-based reopen list is unchanged.
  assert.deepEqual(result.pdfPathsToReopen, ["E:/docs/contract.pdf"]);

  // PDF tab is preserved in the snapshot so it stays visible during restore,
  // but its id is now a `pending-reopen-` placeholder so no code path can
  // mistake it for a live backend session (the `doc-session-` prefix).
  assert.equal(result.snapshot.openTabs.length, 2);
  const restoredPdf = result.snapshot.openTabs[0];
  assert.notEqual(restoredPdf.id, "doc-session-1");
  assert.ok(
    isPendingReopenTabId(restoredPdf.id),
    `restored pdf id ${restoredPdf.id} must start with ${PENDING_REOPEN_PREFIX}`,
  );
  assert.equal(restoredPdf.loadState, "loading");
  // The dirty flag from the persisted snapshot must NOT be trusted: the
  // backend session is dead, autosave would otherwise hit SESSION_NOT_FOUND.
  assert.equal(restoredPdf.dirty, false);

  // Non-PDF tabs are untouched.
  assert.equal(result.snapshot.openTabs[1].id, "tab-review");

  // Reopen entries reference the new placeholder id so the App loop can
  // close the right tab once the file is reopened from disk.
  assert.deepEqual(result.pdfTabsToReopen, [
    { tabId: restoredPdf.id, path: "E:/docs/contract.pdf" },
  ]);

  // The activeTabId is remapped to the new placeholder id so the active tab
  // still resolves after re-id.
  assert.equal(result.snapshot.activeTabId, restoredPdf.id);
});

test("isPendingReopenTabId distinguishes placeholders from live sessions", () => {
  assert.equal(isPendingReopenTabId(`${PENDING_REOPEN_PREFIX}123-1`), true);
  assert.equal(isPendingReopenTabId("doc-session-abc"), false);
  assert.equal(isPendingReopenTabId("tab-review"), false);
});
