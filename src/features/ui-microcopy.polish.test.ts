import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

const primaryUiFiles = [
  "src/features/objects/useObjectsPanel.ts",
  "src/features/objects/ObjectsPanel.tsx",
  "src/features/compare/ComparePanel.tsx",
  "src/features/review/ReviewPanel.tsx",
  "src/features/reports/ReportExportPanel.tsx",
];

describe("Pass 2G — primary UI microcopy", () => {
  it("keeps raw implementation labels out of primary panel copy", () => {
    const source = primaryUiFiles.map((path) => readFileSync(path, "utf8")).join("\n");
    expect(source).not.toContain(">NativeMultiOperator<");
    expect(source).not.toContain(">SafeVisualReplacement<");
    expect(source).not.toContain(">XObject swap<");
    expect(source).not.toContain(">sessionId<");
    expect(source).not.toContain(">doc_extract_all_text<");
    expect(source).not.toContain(">render exception<");
    expect(source).not.toContain(">Content object<");
  });
});
