#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";

const args = parseArgs(process.argv.slice(2));

if (args["self-test"]) {
  runSelfTest();
  process.exit(0);
}

const projectRoot = requireDirectory(args["project-root"], "project root");
const bentoRoot = requireDirectory(args["bento-root"], "BentoPDF root");
const auditRoot = requireValue(args["audit-root"], "audit root");
const inventoryOnly = Boolean(args["inventory-only"]);

fs.mkdirSync(auditRoot, { recursive: true });

const requiredR2hPrefixes = [
  "src/components/shell/",
  "src/features/compare/",
  "src/features/export/",
  "src/features/forms/",
  "src/features/ocr/",
  "src/features/page-organizer/",
  "src/features/pdf-editor/",
  "src/features/sign-stamp/",
  "src/lib/",
  "src/state/",
  "src-tauri/src/",
];

const r2hFiles = gitTrackedFiles(projectRoot)
  .filter((relative) => requiredR2hPrefixes.some((prefix) => relative.startsWith(prefix)))
  .filter(isSupportedSource);

const r2hRows = [];
const backendRows = [];
const dependencyRows = [];

for (const relative of r2hFiles) {
  const absolute = path.join(projectRoot, ...relative.split("/"));
  const text = fs.readFileSync(absolute, "utf8");
  const sha256 = hashFile(absolute);
  const imports = extractImports(text, relative);
  const importedSymbols = extractExportedSymbols(text, relative);
  const invokes = extractTauriInvocations(text);
  const rustCommands = relative.endsWith(".rs")
    ? extractRustCommands(text)
    : [];

  for (const item of buildFeatureRows({
    relative,
    text,
    sha256,
    imports,
    importedSymbols,
    invokes,
  })) {
    r2hRows.push(item);
  }

  for (const command of rustCommands) {
    backendRows.push({
      capabilityCandidate: normalizeCapability(command),
      path: relative,
      symbol: command,
      surfaceType: "rust-tauri-command",
      tauriCommand: command,
      imports: imports.join(" | "),
      importedBy: "",
      testPaths: "",
      sourceSha256: sha256,
    });
  }

  for (const target of imports) {
    dependencyRows.push({
      sourcePath: relative,
      target,
      edgeType: "static-import",
      symbol: "",
      evidence: `${relative}:import`,
    });
  }

  for (const command of invokes) {
    dependencyRows.push({
      sourcePath: relative,
      target: command,
      edgeType: "tauri-invoke",
      symbol: command,
      evidence: `${relative}:invoke`,
    });
  }
}

const importBacklinks = buildImportBacklinks(dependencyRows);
for (const row of r2hRows) {
  row.importedBy = (importBacklinks.get(row.path) ?? []).join(" | ");
}

const testFiles = gitTrackedFiles(projectRoot)
  .filter((relative) => /\.(test|spec)\.(ts|tsx)$/.test(relative));

for (const row of r2hRows) {
  row.testPaths = testFiles
    .filter((testPath) => likelyRelatedTest(row.path, testPath))
    .join(" | ");
}

writeCsv(path.join(auditRoot, "R2H_FEATURE_SURFACE.csv"), dedupeRows(r2hRows));
writeCsv(path.join(auditRoot, "R2H_BACKEND_COMMANDS.csv"), dedupeRows(backendRows));
writeCsv(path.join(auditRoot, "DEPENDENCY_EDGES.csv"), dedupeRows(dependencyRows));

verifyKnownSurfaces(r2hRows, backendRows);

let bentoCatalogRows = [];
let ownershipRows = [];

if (!inventoryOnly) {
  const overridesPath = path.join(
    projectRoot,
    "scripts",
    "bentopdf",
    "duplicate-ownership-overrides.json",
  );

  const overrides = readOverrides(overridesPath);
  bentoCatalogRows = buildBentoToolCatalog(bentoRoot);
  ownershipRows = buildOwnershipMatrix({
    r2hRows: dedupeRows(r2hRows),
    backendRows: dedupeRows(backendRows),
    bentoRows: bentoCatalogRows,
    overrides,
  });

  writeCsv(
    path.join(auditRoot, "BENTOPDF_TOOL_CATALOG.csv"),
    bentoCatalogRows,
  );
  writeCsv(
    path.join(auditRoot, "DUPLICATE_OWNERSHIP_MATRIX.csv"),
    ownershipRows,
  );
}

console.log("R2H_FEATURE_SURFACE.csv:", r2hRows.length);
console.log("R2H_BACKEND_COMMANDS.csv:", backendRows.length);
console.log("DEPENDENCY_EDGES.csv:", dependencyRows.length);

if (!inventoryOnly) {
  console.log("BENTOPDF_TOOL_CATALOG.csv:", bentoCatalogRows.length);
  console.log("DUPLICATE_OWNERSHIP_MATRIX.csv:", ownershipRows.length);

  if (bentoCatalogRows.length === 0 || ownershipRows.length === 0) {
    throw new Error(
      "BentoPDF catalog generation produced no tools. Check the live page extensions and source paths.",
    );
  }

  console.log("BENTO_TOOL_CATALOG_PASS");
}

console.log("KNOWN_R2H_SURFACES_PASS");

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

function gitTrackedFiles(root) {
  const result = spawnSync(
    "git",
    ["ls-files", "-z"],
    {
      cwd: root,
      encoding: "utf8",
      windowsHide: true,
      maxBuffer: 64 * 1024 * 1024,
    },
  );

  if (result.status !== 0) {
    throw new Error(`git ls-files failed in ${root}: ${result.stderr}`);
  }

  return result.stdout
    .split("\0")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => entry.replaceAll("\\", "/"));
}

function isSupportedSource(relative) {
  return /\.(ts|tsx|js|jsx|rs)$/.test(relative);
}

function hashFile(filePath) {
  return crypto
    .createHash("sha256")
    .update(fs.readFileSync(filePath))
    .digest("hex")
    .toUpperCase();
}

function normalizeCapability(value) {
  return value
    .replace(/\.(test|spec)$/i, "")
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/[_\s]+/g, "-")
    .replace(/[^a-zA-Z0-9-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase();
}

function extractImports(text, relativePath) {
  if (relativePath.endsWith(".rs")) {
    return [...text.matchAll(/^\s*(?:use|mod)\s+([^;]+);/gm)]
      .map((match) => match[1].trim());
  }

  return [...text.matchAll(
    /^\s*(?:import|export)\s+(?:[\s\S]*?\s+from\s+)?["']([^"']+)["'];?/gm,
  )].map((match) => match[1]);
}

function extractExportedSymbols(text, relativePath) {
  if (relativePath.endsWith(".rs")) {
    return [...text.matchAll(
      /^\s*pub\s+(?:async\s+)?(?:fn|struct|enum|trait|type|const|static)\s+([A-Za-z0-9_]+)/gm,
    )].map((match) => match[1]);
  }

  return [...text.matchAll(
    /^\s*export\s+(?:default\s+)?(?:async\s+)?(?:function|class|const|let|var|type|interface|enum)\s+([A-Za-z0-9_]+)/gm,
  )].map((match) => match[1]);
}

function extractTauriInvocations(text) {
  const commands = new Set();
  for (const match of text.matchAll(/\binvoke(?:<[^>]+>)?\s*\(\s*["']([^"']+)["']/g)) {
    commands.add(match[1]);
  }
  return [...commands];
}

function extractRustCommands(text) {
  const commands = [];
  const pattern = /#\s*\[\s*tauri::command(?:\s*\([^)]*\))?\s*\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)/g;
  for (const match of text.matchAll(pattern)) {
    commands.push(match[1]);
  }
  return commands;
}

function buildFeatureRows({
  relative,
  text,
  sha256,
  imports,
  importedSymbols,
  invokes,
}) {
  const rows = [];
  const baseName = path.posix.basename(relative).replace(/\.[^.]+$/, "");
  const pathCapability = capabilityFromPath(relative);

  const symbols = importedSymbols.length > 0
    ? importedSymbols
    : [baseName];

  for (const symbol of symbols) {
    rows.push({
      capabilityCandidate: pathCapability || normalizeCapability(symbol),
      path: relative,
      symbol,
      surfaceType: classifySurface(relative, text),
      tauriCommand: invokes.join(" | "),
      imports: imports.join(" | "),
      importedBy: "",
      testPaths: "",
      sourceSha256: sha256,
    });
  }

  return rows;
}

function capabilityFromPath(relative) {
  const known = [
    "compare",
    "export",
    "forms",
    "ocr",
    "page-organizer",
    "pdf-editor",
    "sign-stamp",
    "bentopdf",
  ];

  const normalized = relative.replaceAll("\\", "/");
  return known.find((candidate) => normalized.includes(`/${candidate}/`))
    ?? (normalized.includes("bentopdf_runtime.rs") ? "bentopdf-window-integration" : "")
    ?? "";
}

function classifySurface(relative, text) {
  if (relative.endsWith(".rs")) {
    return text.includes("#[tauri::command")
      ? "rust-tauri-command-file"
      : "rust-module";
  }
  if (/\.(test|spec)\.(ts|tsx)$/.test(relative)) {
    return "test";
  }
  if (relative.endsWith(".tsx")) {
    return /function\s+[A-Z]|const\s+[A-Z][A-Za-z0-9_]*\s*=/.test(text)
      ? "react-component"
      : "tsx-module";
  }
  if (relative.includes("/state/")) return "state";
  if (relative.includes("/hooks/") || /use[A-Z][A-Za-z0-9_]*\s*\(/.test(text)) return "hook";
  return "typescript-module";
}

function buildImportBacklinks(rows) {
  const map = new Map();
  for (const row of rows) {
    if (row.edgeType !== "static-import") continue;
    const normalized = row.target.replaceAll("\\", "/");
    if (!map.has(normalized)) map.set(normalized, []);
    map.get(normalized).push(row.sourcePath);
  }
  return map;
}

function likelyRelatedTest(sourcePath, testPath) {
  const sourceStem = path.posix.basename(sourcePath).replace(/\.[^.]+$/, "");
  const testStem = path.posix.basename(testPath);
  if (testStem.startsWith(`${sourceStem}.`)) return true;

  const sourceDir = path.posix.dirname(sourcePath);
  const testDir = path.posix.dirname(testPath);
  return sourceDir === testDir;
}

function verifyKnownSurfaces(featureRows, backendRows) {
  const combined = [
    ...featureRows.map((row) => `${row.capabilityCandidate} ${row.path}`),
    ...backendRows.map((row) => `${row.capabilityCandidate} ${row.path} ${row.symbol}`),
  ].join("\n").toLowerCase();

  const required = [
    "compare",
    "forms",
    "ocr",
    "page-organizer",
    "sign-stamp",
    "bentopdf",
  ];

  const missing = required.filter((value) => !combined.includes(value));
  if (missing.length > 0) {
    throw new Error(`Known R2H surfaces were not detected: ${missing.join(", ")}`);
  }
}

function dedupeRows(rows) {
  const seen = new Set();
  const result = [];
  for (const row of rows) {
    const key = JSON.stringify(row);
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(row);
  }
  return result;
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

  const headers = Object.keys(rows[0]);
  const lines = [
    headers.map(csvEscape).join(","),
    ...rows.map((row) => headers.map((header) => csvEscape(row[header])).join(",")),
  ];
  fs.writeFileSync(filePath, `${lines.join("\n")}\n`, "utf8");
}


function readOverrides(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Ownership overrides file not found: ${filePath}`);
  }

  const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
  if (!Array.isArray(parsed.capabilities)) {
    throw new Error("Ownership overrides must contain a capabilities array.");
  }

  return new Map(
    parsed.capabilities.map((entry) => [entry.capabilityId, entry]),
  );
}

function buildBentoToolCatalog(root) {
  const rows = [];

  for (const relative of gitTrackedFiles(root)) {
    const normalized = relative.replaceAll("\\\\", "/");

    if (!/^src\/pages\/.+\.(?:html|astro)$/i.test(normalized)) {
      continue;
    }

    const route = routeFromAstroPath(normalized);
    const capabilityId = capabilityFromBentoRoute(route);

    if (!capabilityId || isWebsiteOnlyRoute(route)) {
      continue;
    }

    const absolute = path.join(root, ...normalized.split("/"));
    const text = fs.readFileSync(absolute, "utf8");

    rows.push({
      capabilityId,
      route,
      sourcePath: normalized,
      displayName: extractPageTitle(text) || humanizeCapability(capabilityId),
      category: inferBentoCategory(route, text),
      usesWorker: /\bWorker\s*\(|worker\.js|\.worker\./i.test(text),
      usesWasm: /\.wasm\b|WebAssembly|wasm/i.test(text),
      usesOcr: /\bOCR\b|tesseract|ocr-language/i.test(text),
      usesFileInput: /type\s*=\s*["']file["']|showOpenFilePicker|FileReader/i.test(text),
      createsDownload: /createObjectURL|download\s*=|saveAs\s*\(/i.test(text),
      localizedCopy: isLocalizedRoute(route),
      sourceSha256: hashFile(absolute),
    });
  }

  return dedupeBy(rows, (row) => `${row.capabilityId}|${row.sourcePath}`)
    .sort((a, b) => a.capabilityId.localeCompare(b.capabilityId));
}

function routeFromAstroPath(relative) {
  let route = relative
    .replace(/^src\/pages\//, "")
    .replace(/\.(?:html|astro)$/i, "")
    .replace(/\/index$/i, "")
    .replace(/\\/g, "/");

  return `/${route}`.replace(/\/+/g, "/");
}

function isLocalizedRoute(route) {
  return /^\/(?:ar|de|es|fr|it|ja|ko|pt|ru|tr|zh)(?:\/|$)/i.test(route);
}

function isWebsiteOnlyRoute(route) {
  return /^\/(?:$|index|about|privacy|terms|license|blog|donate|donation|support|contact|changelog|roadmap|sitemap)(?:\/|$)/i.test(route);
}

function capabilityFromBentoRoute(route) {
  const withoutLocale = route.replace(
    /^\/(?:ar|de|es|fr|it|ja|ko|pt|ru|tr|zh)(?=\/)/i,
    "",
  );

  const last = withoutLocale
    .split("/")
    .filter(Boolean)
    .at(-1);

  if (!last) return "";

  return normalizeCapability(last);
}

function extractPageTitle(text) {
  const htmlTitle = text.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
  if (htmlTitle) {
    return htmlTitle[1]
      .replace(/\s+/g, " ")
      .trim();
  }

  const frontmatter = text.match(/title\s*:\s*["']([^"']+)["']/i);
  if (frontmatter) return frontmatter[1].trim();

  const heading = text.match(/<h1[^>]*>([\s\S]*?)<\/h1>/i);
  if (!heading) return "";

  return heading[1]
    .replace(/<[^>]+>/g, " ")
    .replace(/\{[^}]+\}/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function humanizeCapability(capabilityId) {
  return capabilityId
    .split("-")
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

function inferBentoCategory(route, text) {
  const combined = `${route}\n${text}`.toLowerCase();

  if (/ocr|scan|deskew/.test(combined)) return "ocr";
  if (/sign|signature|stamp|encrypt|decrypt|permission|security/.test(combined)) return "security";
  if (/convert|to-pdf|pdf-to|image|office|epub|mobi|html/.test(combined)) return "conversion";
  if (/merge|split|page|rotate|crop|organize|booklet|n-up/.test(combined)) return "page-structure";
  if (/compress|repair|raster|sanitize|linearize/.test(combined)) return "processing";
  if (/form|watermark|header|footer|metadata|attachment/.test(combined)) return "document-utilities";
  return "other";
}

function buildOwnershipMatrix({
  r2hRows,
  backendRows,
  bentoRows,
  overrides,
}) {
  const r2hByCapability = groupByCapability([...r2hRows, ...backendRows]);
  const uniqueBento = dedupeBy(
    bentoRows.filter((row) => !row.localizedCopy),
    (row) => row.capabilityId,
  );

  const matrix = [];

  for (const bento of uniqueBento) {
    const override = overrides.get(bento.capabilityId);
    const directMatches = r2hByCapability.get(bento.capabilityId) ?? [];
    const aliasMatches = findAliasMatches(bento.capabilityId, r2hByCapability);
    const matches = dedupeBy(
      [...directMatches, ...aliasMatches],
      (row) => `${row.path}|${row.symbol}`,
    );

    const matchMethod = override
      ? "explicit-override"
      : directMatches.length > 0
        ? "exact-capability-id"
        : aliasMatches.length > 0
          ? "reviewed-token-alias"
          : "unmatched";

    matrix.push({
      capabilityId: bento.capabilityId,
      displayName: bento.displayName,
      finalOwner: override?.finalOwner ?? "bentopdf",
      matchMethod,
      r2hUiPaths: matches
        .filter((row) => !row.path.endsWith(".rs"))
        .map((row) => row.path)
        .join(" | "),
      r2hBackendSymbols: matches
        .filter((row) => row.path.endsWith(".rs") || row.tauriCommand)
        .map((row) => row.symbol || row.tauriCommand)
        .filter(Boolean)
        .join(" | "),
      bentoToolPaths: bento.sourcePath,
      duplicateDetected: matches.length > 0,
      deletionSafety: override?.deletionSafety ?? (matches.length > 0 ? "blocked" : "blocked"),
      reviewStatus: override?.reviewStatus ?? "pending-human-review",
      rationale: override?.rationale ?? "",
      evidence: `live-source:${bento.sourcePath}`,
    });
  }

  return matrix.sort((a, b) => a.capabilityId.localeCompare(b.capabilityId));
}

function groupByCapability(rows) {
  const result = new Map();

  for (const row of rows) {
    const candidate = normalizeCapability(
      row.capabilityCandidate || row.tauriCommand || row.symbol || "",
    );

    if (!candidate) continue;

    if (!result.has(candidate)) result.set(candidate, []);
    result.get(candidate).push(row);
  }

  return result;
}

function findAliasMatches(capabilityId, grouped) {
  const aliases = {
    "merge-pdf": ["merge", "doc-merge"],
    "split-pdf": ["split", "doc-split"],
    "compare-pdf": ["compare", "compare-documents"],
    "ocr-pdf": ["ocr"],
    "sign-pdf": ["sign-stamp", "signature"],
    "add-watermark": ["watermark"],
    "organize-pdf": ["page-organizer"],
    "edit-pdf": ["pdf-editor"],
    "fill-pdf-forms": ["forms"],
  };

  const candidates = aliases[capabilityId] ?? [];
  return candidates.flatMap((candidate) => grouped.get(candidate) ?? []);
}

function dedupeBy(rows, keySelector) {
  const seen = new Set();
  const result = [];

  for (const row of rows) {
    const key = keySelector(row);
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(row);
  }

  return result;
}

function runSelfTest() {
  const quoted = csvEscape('a,"b"');
  if (quoted !== '"a,""b"""') {
    throw new Error("CSV quoting self-test failed.");
  }

  const imports = extractImports(
    'import { x } from "./alpha";\nexport { y } from "../beta";',
    "fixture.ts",
  );
  if (imports.join("|") !== "./alpha|../beta") {
    throw new Error("Import extraction self-test failed.");
  }

  const commands = extractRustCommands(
    "#[tauri::command]\npub async fn doc_merge() {}\n",
  );
  if (commands.length !== 1 || commands[0] !== "doc_merge") {
    throw new Error("Rust command extraction self-test failed.");
  }

  const deduped = dedupeRows([{ path: "a" }, { path: "a" }, { path: "b" }]);
  if (deduped.length !== 2) {
    throw new Error("Duplicate-path self-test failed.");
  }

  const htmlRoute = routeFromAstroPath("src/pages/merge-pdf.html");
  const astroRoute = routeFromAstroPath("src/pages/merge-pdf.astro");

  if (htmlRoute !== "/merge-pdf" || astroRoute !== "/merge-pdf") {
    throw new Error("Bento route normalization self-test failed.");
  }

  if (capabilityFromBentoRoute("/ar/merge-pdf") !== "merge-pdf") {
    throw new Error("Localized Bento capability self-test failed.");
  }

  if (!isWebsiteOnlyRoute("/privacy")) {
    throw new Error("Website-only route exclusion self-test failed.");
  }

  const grouped = new Map([
    ["merge", [{ path: "src/features/merge/index.ts", symbol: "merge" }]],
  ]);
  if (findAliasMatches("merge-pdf", grouped).length !== 1) {
    throw new Error("Reviewed alias matching self-test failed.");
  }

  console.log("OWNERSHIP_MATRIX_SELF_TEST_PASS");
}

