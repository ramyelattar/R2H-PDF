import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const outputRoot = path.resolve(process.argv[2] || "");
const requestedBase = process.argv[3] || "/";
const reportPath = process.argv[4]
  ? path.resolve(process.argv[4])
  : null;

if (!fs.existsSync(outputRoot) || !fs.statSync(outputRoot).isDirectory()) {
  throw new Error(`Output root does not exist: ${outputRoot}`);
}

const normalizedBase =
  "/" + requestedBase.replace(/^\/+|\/+$/g, "") + "/";

if (normalizedBase === "//") {
  throw new Error("A non-root base path is required.");
}

const baseSegment = normalizedBase.slice(1);
const escapedBaseSegment = baseSegment.replace(
  /[.*+?^${}()|[\]\\]/g,
  "\\$&"
);

function walkHtml(directory) {
  const output = [];

  for (const entry of fs.readdirSync(directory, {
    withFileTypes: true,
  })) {
    const absolute = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      output.push(...walkHtml(absolute));
    } else if (entry.isFile() && entry.name.endsWith(".html")) {
      output.push(absolute);
    }
  }

  return output;
}

function rebuildCompressedSidecars(filePath, text) {
  const bytes = Buffer.from(text, "utf8");
  const result = {
    gzip: false,
    brotli: false,
  };

  const gzipPath = `${filePath}.gz`;
  const brotliPath = `${filePath}.br`;

  if (fs.existsSync(gzipPath)) {
    fs.writeFileSync(
      gzipPath,
      zlib.gzipSync(bytes, {
        level: 9,
      })
    );

    result.gzip = true;
  }

  if (fs.existsSync(brotliPath)) {
    fs.writeFileSync(
      brotliPath,
      zlib.brotliCompressSync(bytes, {
        params: {
          [zlib.constants.BROTLI_PARAM_QUALITY]: 11,
          [zlib.constants.BROTLI_PARAM_MODE]:
            zlib.constants.BROTLI_MODE_TEXT,
        },
      })
    );

    result.brotli = true;
  }

  return result;
}

function isRootAbsoluteLocalPath(value) {
  return (
    typeof value === "string" &&
    value.startsWith("/") &&
    !value.startsWith("//") &&
    !value.startsWith(normalizedBase)
  );
}

function prefixBase(value) {
  if (!isRootAbsoluteLocalPath(value)) {
    return value;
  }

  if (value === "/") {
    return normalizedBase;
  }

  return normalizedBase + value.replace(/^\/+/, "");
}

const htmlFiles = walkHtml(outputRoot);
const changedHtmlFiles = [];
let htmlReplacementCount = 0;
let gzipSidecarsRebuilt = 0;
let brotliSidecarsRebuilt = 0;

const quotedAttributePattern = new RegExp(
  `\\b(href|src|content)\\s*=\\s*(["'])(\\/(?!\\/|${escapedBaseSegment})[^"']*)\\2`,
  "gi"
);

const unquotedAttributePattern = new RegExp(
  `\\b(href|src|content)\\s*=\\s*(\\/(?!\\/|${escapedBaseSegment})[^\\s>]+)`,
  "gi"
);

for (const htmlFile of htmlFiles) {
  const original = fs.readFileSync(htmlFile, "utf8");
  let fileReplacementCount = 0;

  let updated = original.replace(
    quotedAttributePattern,
    (full, attribute, quote, value) => {
      fileReplacementCount++;
      htmlReplacementCount++;

      return `${attribute}=${quote}${prefixBase(value)}${quote}`;
    }
  );

  updated = updated.replace(
    unquotedAttributePattern,
    (full, attribute, value) => {
      fileReplacementCount++;
      htmlReplacementCount++;

      return `${attribute}=${prefixBase(value)}`;
    }
  );

  if (updated === original) {
    continue;
  }

  fs.writeFileSync(htmlFile, updated, "utf8");

  const compressed = rebuildCompressedSidecars(
    htmlFile,
    updated
  );

  if (compressed.gzip) {
    gzipSidecarsRebuilt++;
  }

  if (compressed.brotli) {
    brotliSidecarsRebuilt++;
  }

  changedHtmlFiles.push({
    path: path.relative(outputRoot, htmlFile).replaceAll(path.sep, "/"),
    replacementCount: fileReplacementCount,
    gzipSidecarRebuilt: compressed.gzip,
    brotliSidecarRebuilt: compressed.brotli,
  });
}

const remainingHtmlRootAbsoluteReferences = [];

for (const htmlFile of htmlFiles) {
  const source = fs.readFileSync(htmlFile, "utf8");

  quotedAttributePattern.lastIndex = 0;
  unquotedAttributePattern.lastIndex = 0;

  for (const match of source.matchAll(quotedAttributePattern)) {
    remainingHtmlRootAbsoluteReferences.push({
      path: path.relative(outputRoot, htmlFile).replaceAll(path.sep, "/"),
      attribute: match[1],
      value: match[3],
    });
  }

  for (const match of source.matchAll(unquotedAttributePattern)) {
    remainingHtmlRootAbsoluteReferences.push({
      path: path.relative(outputRoot, htmlFile).replaceAll(path.sep, "/"),
      attribute: match[1],
      value: match[2],
    });
  }
}

const webManifestPath = path.join(outputRoot, "site.webmanifest");
let webManifestFound = false;
let webManifestChanged = false;
let webManifestReplacementCount = 0;
let webManifestGzipSidecarRebuilt = false;
let webManifestBrotliSidecarRebuilt = false;
const remainingWebManifestRootAbsoluteReferences = [];

function rewriteManifestValue(value, jsonPath) {
  if (typeof value === "string") {
    if (isRootAbsoluteLocalPath(value)) {
      webManifestReplacementCount++;
      return prefixBase(value);
    }

    return value;
  }

  if (Array.isArray(value)) {
    return value.map(
      (entry, index) =>
        rewriteManifestValue(entry, `${jsonPath}[${index}]`)
    );
  }

  if (value && typeof value === "object") {
    const result = {};

    for (const [key, entry] of Object.entries(value)) {
      result[key] = rewriteManifestValue(
        entry,
        jsonPath ? `${jsonPath}.${key}` : key
      );
    }

    return result;
  }

  return value;
}

function collectManifestRootPaths(value, jsonPath) {
  if (typeof value === "string") {
    if (isRootAbsoluteLocalPath(value)) {
      remainingWebManifestRootAbsoluteReferences.push({
        jsonPath,
        value,
      });
    }

    return;
  }

  if (Array.isArray(value)) {
    value.forEach(
      (entry, index) =>
        collectManifestRootPaths(entry, `${jsonPath}[${index}]`)
    );

    return;
  }

  if (value && typeof value === "object") {
    for (const [key, entry] of Object.entries(value)) {
      collectManifestRootPaths(
        entry,
        jsonPath ? `${jsonPath}.${key}` : key
      );
    }
  }
}

if (fs.existsSync(webManifestPath)) {
  webManifestFound = true;

  const originalText = fs.readFileSync(webManifestPath, "utf8");
  const originalManifest = JSON.parse(originalText);
  const updatedManifest = rewriteManifestValue(
    originalManifest,
    ""
  );

  if (
    !Object.prototype.hasOwnProperty.call(
      updatedManifest,
      "scope"
    ) ||
    updatedManifest.scope !== normalizedBase
  ) {
    updatedManifest.scope = normalizedBase;
    webManifestReplacementCount++;
  }

  const updatedText =
    JSON.stringify(updatedManifest, null, 2) + "\n";

  if (updatedText !== originalText) {
    fs.writeFileSync(
      webManifestPath,
      updatedText,
      "utf8"
    );

    webManifestChanged = true;

    const compressed = rebuildCompressedSidecars(
      webManifestPath,
      updatedText
    );

    webManifestGzipSidecarRebuilt = compressed.gzip;
    webManifestBrotliSidecarRebuilt = compressed.brotli;

    if (compressed.gzip) {
      gzipSidecarsRebuilt++;
    }

    if (compressed.brotli) {
      brotliSidecarsRebuilt++;
    }
  }

  const finalManifest = JSON.parse(
    fs.readFileSync(webManifestPath, "utf8")
  );

  collectManifestRootPaths(finalManifest, "");
}

const result = {
  schemaVersion: 2,
  outputRoot,
  normalizedBase,

  scannedHtmlFileCount: htmlFiles.length,
  changedHtmlFileCount: changedHtmlFiles.length,
  htmlReplacementCount,

  webManifestFound,
  webManifestChanged,
  webManifestReplacementCount,

  gzipSidecarsRebuilt,
  brotliSidecarsRebuilt,
  webManifestGzipSidecarRebuilt,
  webManifestBrotliSidecarRebuilt,

  remainingHtmlRootAbsoluteReferenceCount:
    remainingHtmlRootAbsoluteReferences.length,

  remainingWebManifestRootAbsoluteReferenceCount:
    remainingWebManifestRootAbsoluteReferences.length,

  changedHtmlFiles,
  remainingHtmlRootAbsoluteReferences,
  remainingWebManifestRootAbsoluteReferences,
};

if (reportPath) {
  fs.mkdirSync(path.dirname(reportPath), {
    recursive: true,
  });

  fs.writeFileSync(
    reportPath,
    JSON.stringify(result, null, 2),
    "utf8"
  );
}

console.log(JSON.stringify(result, null, 2));

if (
  !webManifestFound ||
  remainingHtmlRootAbsoluteReferences.length > 0 ||
  remainingWebManifestRootAbsoluteReferences.length > 0
) {
  process.exit(1);
}
