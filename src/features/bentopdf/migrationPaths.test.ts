import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(here, "../../../");

const bentoPdfToolingScripts = [
  "scripts/bentopdf/prepare-offline-runtime.ps1",
  "scripts/bentopdf/audit-browser-network-isolation.ps1",
  "scripts/bentopdf/r2h-pdf-bentopdf-per-script-site-url-repair.ps1",
];

describe("BentoPDF migration paths", () => {
  it("derives its project root from the script location", () => {
    for (const relativePath of bentoPdfToolingScripts) {
      const source = fs.readFileSync(path.join(projectRoot, relativePath), "utf8");

      expect(source).not.toMatch(/[A-Z]:[\\/]Projects[\\/]R2H-PDF/);
      expect(source).toContain("$PSScriptRoot");
    }
  });
});
