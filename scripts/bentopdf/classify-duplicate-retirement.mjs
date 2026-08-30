#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const args = parseArgs(process.argv.slice(2));

if (args["self-test"]) {
  runSelfTest();
  process.exit(0);
}

const projectRoot = requireDirectory(args["project-root"], "project root");
const auditRoot = requireDirectory(args["catalog-audit-root"], "Task 4 audit root");
const outputRoot = requireValue(args["output-root"], "output root");
const overridesPath = path.join(
  projectRoot,
  "scripts",
  "bentopdf",
  "duplicate-ownership-overrides.json",
);

fs.mkdirSync(outputRoot, { recursive: true });

const catalog = readCsv(path.join(auditRoot, "BENTOPDF_TOOL_CATALOG.csv"));
const featureRows = readCsv(path.join(auditRoot, "R2H_FEATURE_SURFACE.csv"));
const backendRows = readCsv(path.join(auditRoot, "R2H_BACKEND_COMMANDS.csv"));
const dependencyRows = readCsv(path.join(auditRoot, "DEPENDENCY_EDGES.csv"));
const overrides = readOverrides(overridesPath);
const trackedSources = loadTrackedSource(projectRoot);

const catalogById = new Map(catalog.map((row) => [row.capabilityId, row]));
const deletionRows = [];
const retainedRows = [];
const matrixRows = [];

for (const override of overrides.capabilities) {
  const bento = catalogById.get(override.capabilityId);
  if (!bento) {
    throw new Error(
      `Approved override references a missing BentoPDF tool: ${override.capabilityId}`,
    );
  }

  const uiRows = featureRows.filter((row) =>
    override.r2hFeatureCandidates.includes(row.capabilityCandidate),
  );

  const backendMatches = backendRows.filter((row) =>
    override.r2hBackendSymbols.includes(row.symbol),
  );

  const missingBackendSymbols = override.r2hBackendSymbols.filter(
    (symbol) => !backendMatches.some((row) => row.symbol === symbol),
  );

  if (missingBackendSymbols.length > 0) {
    throw new Error(
      `${override.capabilityId} references missing R2H backend symbols: ${missingBackendSymbols.join(", ")}`,
    );
  }

  const consumerEvidence = collectConsumers({
    trackedSources,
    backendSymbols: override.r2hBackendSymbols,
    uiPaths: new Set(uiRows.map((row) => row.path)),
  });

  const retainedConsumers = consumerEvidence
    .filter((item) => item.kind === "retained")
    .map((item) => item.reference);

  const unresolvedConsumers = consumerEvidence
    .filter((item) => item.kind === "unresolved")
    .map((item) => item.reference);

  const classification = classifyDeletion({
    override,
    uiRows,
    backendMatches,
    retainedConsumers,
    unresolvedConsumers,
  });

  const requiredTests = buildRequiredTests({
    capabilityId: override.capabilityId,
    uiRows,
    backendMatches,
    retainedConsumers,
  });

  const common = {
    capabilityId: override.capabilityId,
    displayName: bento.displayName,
    finalOwner: override.finalOwner,
    bentoToolPath: bento.sourcePath,
    r2hUiPaths: unique(uiRows.map((row) => row.path)).join(" | "),
    r2hBackendSymbols: unique(backendMatches.map((row) => row.symbol)).join(" | "),
    retainedConsumers: retainedConsumers.join(" | "),
    unresolvedConsumers: unresolvedConsumers.join(" | "),
    r2hUiDeletion: classification.r2hUiDeletion,
    r2hBackendDeletion: classification.r2hBackendDeletion,
    deletionSafety: classification.deletionSafety,
    blockingReason: classification.blockingReason,
    requiredTests: requiredTests.join(" | "),
    reviewStatus: override.reviewStatus,
    approvedDeletionLevel: override.approvedDeletionLevel,
    rationale: override.rationale,
  };

  matrixRows.push(common);

  if (
    classification.r2hUiDeletion !== "not-applicable" ||
    classification.r2hBackendDeletion !== "not-applicable"
  ) {
    deletionRows.push(common);
  }

  if (
    override.reviewStatus === "blocked" ||
    retainedConsumers.length > 0 ||
    classification.r2hBackendDeletion !== "safe-after-tests"
  ) {
    retainedRows.push({
      capabilityId: override.capabilityId,
      retainedR2hUiPaths: common.r2hUiPaths,
      retainedR2hBackendSymbols: common.r2hBackendSymbols,
      retainedConsumers: common.retainedConsumers,
      reason:
        override.reviewStatus === "blocked"
          ? override.rationale
          : classification.blockingReason,
      reviewStatus: override.reviewStatus,
    });
  }
}

for (const row of catalog) {
  if (matrixRows.some((item) => item.capabilityId === row.capabilityId)) {
    continue;
  }

  matrixRows.push({
    capabilityId: row.capabilityId,
    displayName: row.displayName,
    finalOwner: "bentopdf",
    bentoToolPath: row.sourcePath,
    r2hUiPaths: "",
    r2hBackendSymbols: "",
    retainedConsumers: "",
    unresolvedConsumers: "",
    r2hUiDeletion: "not-applicable",
    r2hBackendDeletion: "not-applicable",
    deletionSafety: "no-r2h-duplicate-detected",
    blockingReason: "",
    requiredTests: "bentopdf-tool-smoke",
    reviewStatus: "pending-human-review",
    approvedDeletionLevel: "none",
    rationale: "No explicit R2H duplicate mapping has been approved.",
  });
}

writeCsv(
  path.join(outputRoot, "DUPLICATE_OWNERSHIP_MATRIX_RECONCILED.csv"),
  matrixRows.sort(byCapability),
);
writeCsv(
  path.join(outputRoot, "DELETION_CANDIDATES.csv"),
  deletionRows.sort(byCapability),
);
writeCsv(
  path.join(outputRoot, "RETAINED_R2H_CAPABILITIES.csv"),
  retainedRows.sort(byCapability),
);
writeCsv(
  path.join(outputRoot, "DEPENDENCY_CLASSIFICATION_EVIDENCE.csv"),
  dependencyRows,
);

const summary = {
  schemaVersion: 1,
  sourceCatalogRows: catalog.length,
  explicitOwnershipRows: overrides.capabilities.length,
  deletionCandidateRows: deletionRows.length,
  retainedCapabilityRows: retainedRows.length,
  backendSafeRows: deletionRows.filter(
    (row) => row.r2hBackendDeletion === "safe-after-tests",
  ).length,
  backendSharedRows: deletionRows.filter(
    (row) => row.r2hBackendDeletion === "blocked-shared",
  ).length,
  uiSafeRows: deletionRows.filter(
    (row) => row.r2hUiDeletion === "safe-after-tests",
  ).length,
  sourceModified: false,
  generatedAtUtc: new Date().toISOString(),
};

fs.writeFileSync(
  path.join(outputRoot, "RETIREMENT_CLASSIFICATION_SUMMARY.json"),
  `${JSON.stringify(summary, null, 2)}\n`,
  "utf8",
);

console.log(`Explicit ownership rows: ${summary.explicitOwnershipRows}`);
console.log(`Deletion candidates: ${summary.deletionCandidateRows}`);
console.log(`Retained capabilities: ${summary.retainedCapabilityRows}`);
console.log(`UI safe after tests: ${summary.uiSafeRows}`);
console.log(`Backend safe after tests: ${summary.backendSafeRows}`);
console.log(`Backend shared/blocked: ${summary.backendSharedRows}`);
console.log("DUPLICATE_RETIREMENT_CLASSIFICATION_PASS");

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      result[key] = true;
    } else {
      result[key] = next;
      index += 1;
    }
  }
  return result;
}

function requireValue(value, description) {
  if (!value || typeof value !== "string") {
    throw new Error(`Missing ${description}.`);
  }
  return path.resolve(value);
}

function requireDirectory(value, description) {
  const resolved = requireValue(value, description);
  if (!fs.existsSync(resolved) || !fs.statSync(resolved).isDirectory()) {
    throw new Error(`${description} does not exist: ${resolved}`);
  }
  return resolved;
}

function readOverrides(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Ownership overrides not found: ${filePath}`);
  }
  const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
  if (!Array.isArray(parsed.capabilities)) {
    throw new Error("Ownership overrides must contain a capabilities array.");
  }
  return parsed;
}

function loadTrackedSource(root) {
  const result = spawnSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`git ls-files failed: ${result.stderr}`);
  }

  return result.stdout
    .split("\0")
    .filter(Boolean)
    .filter((relative) => /\.(?:ts|tsx|js|jsx|rs)$/.test(relative))
    .map((relative) => {
      const normalized = relative.replaceAll("\\", "/");
      const absolute = path.join(root, ...normalized.split("/"));
      return {
        path: normalized,
        text: fs.readFileSync(absolute, "utf8"),
      };
    });
}

function collectConsumers({
  trackedSources,
  backendSymbols,
  uiPaths,
}) {
  const results = [];

  for (const source of trackedSources) {
    for (const symbol of backendSymbols) {
      const escaped = escapeRegExp(symbol);
      const directPattern = new RegExp(`\\b${escaped}\\b`, "g");
      const invokePattern = new RegExp(
        `invoke(?:<[^>]+>)?\\s*\\(\\s*["']${escaped}["']`,
        "g",
      );

      if (!directPattern.test(source.text) && !invokePattern.test(source.text)) {
        continue;
      }

      const isDefinition =
        source.path.endsWith(".rs") &&
        new RegExp(`fn\\s+${escaped}\\b`).test(source.text);

      const isTauriRegistration =
        source.path === "src-tauri/src/lib.rs" &&
        new RegExp(`\\b${escaped}\\b`).test(source.text);

      if (isDefinition || isTauriRegistration) continue;

      const isTest = /\.(?:test|spec)\.(?:ts|tsx)$/.test(source.path);
      const isOwnedUi = uiPaths.has(source.path);

      results.push({
        kind: isTest || isOwnedUi ? "owned-or-test" : "retained",
        reference: `${source.path}:${symbol}`,
      });
    }


  }

  return uniqueObjects(results, (item) => `${item.kind}|${item.reference}`);
}

function classifyDeletion({
  override,
  uiRows,
  backendMatches,
  retainedConsumers,
  unresolvedConsumers,
}) {
  if (override.reviewStatus === "blocked") {
    return {
      r2hUiDeletion: "blocked",
      r2hBackendDeletion: "blocked",
      deletionSafety: "blocked",
      blockingReason: override.rationale,
    };
  }

  const r2hUiDeletion =
    uiRows.length > 0 ? "safe-after-tests" : "not-applicable";

  if (backendMatches.length === 0) {
    return {
      r2hUiDeletion,
      r2hBackendDeletion: "not-applicable",
      deletionSafety:
        uiRows.length > 0 ? "ui-only-safe" : "no-r2h-backend",
      blockingReason: "",
    };
  }

  if (unresolvedConsumers.length > 0) {
    return {
      r2hUiDeletion,
      r2hBackendDeletion: "blocked-unresolved",
      deletionSafety: "blocked",
      blockingReason:
        "Dynamic or unresolved Tauri invocation exists in the live source.",
    };
  }

  if (retainedConsumers.length > 0) {
    return {
      r2hUiDeletion,
      r2hBackendDeletion: "blocked-shared",
      deletionSafety: "backend-shared",
      blockingReason:
        "One or more retained R2H capabilities still consume the backend symbols.",
    };
  }

  return {
    r2hUiDeletion,
    r2hBackendDeletion: "safe-after-tests",
    deletionSafety:
      uiRows.length > 0 ? "backend-safe" : "backend-only-safe",
    blockingReason: "",
  };
}

function buildRequiredTests({
  capabilityId,
  uiRows,
  backendMatches,
  retainedConsumers,
}) {
  const tests = [
    `ownership:${capabilityId}`,
    "pnpm-typecheck",
    "vitest-full-suite",
  ];

  if (uiRows.length > 0) tests.push("r2h-navigation-regression");
  if (backendMatches.length > 0) tests.push("cargo-check-lib");
  if (retainedConsumers.length > 0) tests.push("retained-consumer-regression");

  return tests;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function unique(values) {
  return [...new Set(values.filter(Boolean))];
}

function uniqueObjects(values, keySelector) {
  const seen = new Set();
  return values.filter((value) => {
    const key = keySelector(value);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function byCapability(a, b) {
  return a.capabilityId.localeCompare(b.capabilityId);
}

function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];

    if (quoted) {
      if (character === '"' && text[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        field += character;
      }
    } else if (character === '"') {
      quoted = true;
    } else if (character === ",") {
      row.push(field);
      field = "";
    } else if (character === "\n") {
      row.push(field.replace(/\r$/, ""));
      field = "";
      if (row.some((value) => value !== "")) rows.push(row);
      row = [];
    } else {
      field += character;
    }
  }

  if (field || row.length > 0) {
    row.push(field);
    rows.push(row);
  }

  if (rows.length === 0) return [];

  const headers = rows[0];
  return rows.slice(1).map((values) =>
    Object.fromEntries(
      headers.map((header, index) => [header, values[index] ?? ""]),
    ),
  );
}

function readCsv(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Required CSV not found: ${filePath}`);
  }
  return parseCsv(fs.readFileSync(filePath, "utf8"));
}

function csvEscape(value) {
  const text = String(value ?? "");
  return `"${text.replaceAll('"', '""')}"`;
}

function writeCsv(filePath, rows) {
  if (rows.length === 0) {
    fs.writeFileSync(filePath, "\n", "utf8");
    return;
  }

  const headers = [];
  const seen = new Set();
  for (const row of rows) {
    for (const key of Object.keys(row)) {
      if (!seen.has(key)) {
        seen.add(key);
        headers.push(key);
      }
    }
  }

  const lines = [
    headers.map(csvEscape).join(","),
    ...rows.map((row) =>
      headers.map((header) => csvEscape(row[header])).join(","),
    ),
  ];

  fs.writeFileSync(filePath, `${lines.join("\n")}\n`, "utf8");
}

function runSelfTest() {
  const shared = classifyDeletion({
    override: {
      reviewStatus: "approved",
      rationale: "",
    },
    uiRows: [{ path: "src/features/ocr/OcrPanel.tsx" }],
    backendMatches: [{ symbol: "ocr_run_page" }],
    retainedConsumers: ["src/features/rag/useRag.ts:ocr_run_page"],
    unresolvedConsumers: [],
  });

  if (
    shared.r2hUiDeletion !== "safe-after-tests" ||
    shared.r2hBackendDeletion !== "blocked-shared"
  ) {
    throw new Error("Shared-backend classification self-test failed.");
  }

  const safe = classifyDeletion({
    override: {
      reviewStatus: "approved",
      rationale: "",
    },
    uiRows: [],
    backendMatches: [{ symbol: "doc_merge" }],
    retainedConsumers: [],
    unresolvedConsumers: [],
  });

  if (safe.r2hBackendDeletion !== "safe-after-tests") {
    throw new Error("Backend-safe classification self-test failed.");
  }

  const blocked = classifyDeletion({
    override: {
      reviewStatus: "blocked",
      rationale: "Retained product capability.",
    },
    uiRows: [{ path: "src/features/pdf-editor/index.ts" }],
    backendMatches: [],
    retainedConsumers: [],
    unresolvedConsumers: [],
  });

  if (blocked.deletionSafety !== "blocked") {
    throw new Error("Blocked classification self-test failed.");
  }

  const consumerEvidence = collectConsumers({
    trackedSources: [
      {
        path: "src/lib/ipc.ts",
        text: "export async function invokeCommand(command, args) { return invoke(command, args); }",
      },
      {
        path: "src-tauri/src/lib.rs",
        text: "#[tauri::command]\nasync fn doc_merge() {}\n.generate_handler![doc_merge]",
      },
    ],
    backendSymbols: ["doc_merge"],
    uiPaths: new Set(),
  });

  if (consumerEvidence.length !== 0) {
    throw new Error(
      "Infrastructure-only consumer filtering self-test failed.",
    );
  }

  console.log("DUPLICATE_RETIREMENT_SELF_TEST_PASS");
}
