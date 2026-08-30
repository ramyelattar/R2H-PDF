import { describe, expect, it } from "vitest";

import {
  type CapabilityOwnershipRecord,
  validateOwnershipMatrix,
} from "./ownershipValidation";

const validRecord: CapabilityOwnershipRecord = {
  capabilityId: "merge-pdf",
  displayName: "Merge PDF",
  finalOwner: "bentopdf",
  r2hUiPaths: ["src/components/shell/WelcomeScreen.tsx"],
  r2hBackendSymbols: ["doc_merge"],
  bentoToolPaths: ["src/pages/merge-pdf.astro"],
  retainedR2hConsumers: [],
  unresolvedConsumers: [],
  deletionSafety: "backend-safe",
  evidence: ["live-source"],
  reviewStatus: "approved",
};

describe("validateOwnershipMatrix", () => {
  it("accepts a complete single-owner record", () => {
    expect(validateOwnershipMatrix([validRecord])).toEqual([]);
  });

  it("rejects missing Bento evidence for a Bento-owned capability", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        bentoToolPaths: [],
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "BENTO_OWNER_WITHOUT_TOOL",
    );
  });

  it("rejects backend-safe when retained R2H consumers remain", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        retainedR2hConsumers: ["src/features/export/usePdfExport.ts"],
        deletionSafety: "backend-safe",
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "BACKEND_SAFE_WITH_CONSUMERS",
    );
  });

  it("rejects backend-safe when unresolved consumers remain", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        unresolvedConsumers: ["dynamic:doc_merge"],
        deletionSafety: "backend-safe",
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "BACKEND_SAFE_WITH_UNRESOLVED_CONSUMERS",
    );
  });

  it("rejects duplicate capability identifiers", () => {
    const issues = validateOwnershipMatrix([validRecord, validRecord]);

    expect(issues.map((issue) => issue.code)).toContain(
      "DUPLICATE_CAPABILITY_ID",
    );
  });

  it("rejects records without source evidence", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        evidence: [],
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "MISSING_EVIDENCE",
    );
  });

  it("rejects R2H-owned capabilities without a retained R2H surface", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        finalOwner: "r2h",
        r2hUiPaths: [],
        r2hBackendSymbols: [],
        bentoToolPaths: [],
        deletionSafety: "blocked",
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "R2H_OWNER_WITHOUT_SURFACE",
    );
  });

  it("rejects records without a completed review status", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        reviewStatus: "pending-human-review",
      },
    ]);

    expect(issues.map((issue) => issue.code)).toContain(
      "OWNERSHIP_REVIEW_INCOMPLETE",
    );
  });
});
