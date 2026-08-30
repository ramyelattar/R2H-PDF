import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(here, "../../../../");

const ipcSource = fs.readFileSync(
  path.join(projectRoot, "src-tauri", "src", "document_core", "ipc.rs"),
  "utf8",
);
const libSource = fs.readFileSync(
  path.join(projectRoot, "src-tauri", "src", "lib.rs"),
  "utf8",
);
const overrides = JSON.parse(
  fs.readFileSync(
    path.join(
      projectRoot,
      "scripts",
      "bentopdf",
      "duplicate-ownership-overrides.json",
    ),
    "utf8",
  ),
);

const retiredCommands = [
  "doc_merge",
  "doc_split",
  "doc_add_watermark",
  "doc_add_header_footer",
];

describe("retired duplicate R2H backend commands", () => {
  it("removes retired commands from Rust definitions and Tauri registration", () => {
    for (const command of retiredCommands) {
      expect(ipcSource).not.toContain(command);
      expect(libSource).not.toContain(command);
    }
  });

  it("records the four Bento-owned capabilities as retired", () => {
    for (const capabilityId of [
      "merge-pdf",
      "split-pdf",
      "add-watermark",
      "header-footer",
    ]) {
      const capability = overrides.capabilities.find(
        (item: { capabilityId: string }) =>
          item.capabilityId === capabilityId,
      );

      expect(capability).toBeTruthy();
      expect(capability.r2hBackendSymbols).toEqual([]);
      expect(capability.approvedDeletionLevel).toBe("retired");
    }
  });
});
