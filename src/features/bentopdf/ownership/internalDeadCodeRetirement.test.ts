import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(here, "../../../../");

const sources = [
  "src-tauri/src/document_core/engine.rs",
  "src-tauri/src/document_core/mupdf_engine.rs",
  "src-tauri/src/document_core/session.rs",
  "src-tauri/src/document_core/types.rs",
].map((relativePath) =>
  fs.readFileSync(path.join(projectRoot, relativePath), "utf8"),
);

describe("retired internal duplicate PDF implementation", () => {
  it("removes internal merge, split, watermark, and header/footer code", () => {
    for (const symbol of [
      "merge_documents",
      "split_document",
      "add_watermark",
      "add_header_footer",
      "WatermarkRequest",
      "HeaderFooterRequest",
    ]) {
      for (const source of sources) {
        expect(source).not.toContain(symbol);
      }
    }
  });

  it("preserves PageRange for AI and text-extraction requests", () => {
    const typesSource = fs.readFileSync(
      path.join(projectRoot, "src-tauri/src/document_core/types.rs"),
      "utf8",
    );
    const aiTypesSource = fs.readFileSync(
      path.join(projectRoot, "src-tauri/src/ai_core/types.rs"),
      "utf8",
    );

    expect(typesSource).toContain("pub struct PageRange");
    expect(aiTypesSource).toContain("Option<PageRange>");
  });
});
