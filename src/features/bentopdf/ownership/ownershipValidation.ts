import type {
  CapabilityOwnershipRecord,
  OwnershipValidationIssue,
} from "./ownershipTypes";

export type {
  CapabilityOwnershipRecord,
  CapabilityOwner,
  DeletionSafety,
  OwnershipReviewStatus,
  OwnershipValidationIssue,
  OwnershipValidationIssueCode,
} from "./ownershipTypes";

export function validateOwnershipMatrix(
  records: readonly CapabilityOwnershipRecord[],
): OwnershipValidationIssue[] {
  const issues: OwnershipValidationIssue[] = [];
  const seenCapabilityIds = new Set<string>();

  for (const record of records) {
    const capabilityId = record.capabilityId.trim();

    if (capabilityId.length === 0) {
      issues.push({
        code: "EMPTY_CAPABILITY_ID",
        capabilityId: record.capabilityId,
        message: "Capability identifier must not be empty.",
      });
      continue;
    }

    if (seenCapabilityIds.has(capabilityId)) {
      issues.push({
        code: "DUPLICATE_CAPABILITY_ID",
        capabilityId,
        message: `Capability identifier "${capabilityId}" appears more than once.`,
      });
    } else {
      seenCapabilityIds.add(capabilityId);
    }

    if (
      record.finalOwner === "bentopdf" &&
      record.bentoToolPaths.length === 0
    ) {
      issues.push({
        code: "BENTO_OWNER_WITHOUT_TOOL",
        capabilityId,
        message:
          "A BentoPDF-owned capability must reference at least one live BentoPDF tool path.",
      });
    }

    if (
      record.finalOwner === "r2h" &&
      record.r2hUiPaths.length === 0 &&
      record.r2hBackendSymbols.length === 0
    ) {
      issues.push({
        code: "R2H_OWNER_WITHOUT_SURFACE",
        capabilityId,
        message:
          "An R2H-owned capability must retain at least one R2H UI path or backend symbol.",
      });
    }

    if (
      record.deletionSafety === "backend-safe" &&
      record.retainedR2hConsumers.length > 0
    ) {
      issues.push({
        code: "BACKEND_SAFE_WITH_CONSUMERS",
        capabilityId,
        message:
          "Backend deletion cannot be marked safe while retained R2H consumers remain.",
      });
    }

    if (
      record.deletionSafety === "backend-safe" &&
      record.unresolvedConsumers.length > 0
    ) {
      issues.push({
        code: "BACKEND_SAFE_WITH_UNRESOLVED_CONSUMERS",
        capabilityId,
        message:
          "Backend deletion cannot be marked safe while unresolved consumers remain.",
      });
    }

    if (record.evidence.length === 0) {
      issues.push({
        code: "MISSING_EVIDENCE",
        capabilityId,
        message:
          "Every ownership record must contain at least one live-source evidence reference.",
      });
    }

    if (record.reviewStatus === "pending-human-review") {
      issues.push({
        code: "OWNERSHIP_REVIEW_INCOMPLETE",
        capabilityId,
        message:
          "Ownership records must be approved or explicitly blocked before destructive work.",
      });
    }
  }

  return issues;
}
