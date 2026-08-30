export type CapabilityOwner = "bentopdf" | "r2h";

export type DeletionSafety =
  | "ui-only-safe"
  | "backend-shared"
  | "backend-safe"
  | "blocked";

export type OwnershipReviewStatus =
  | "pending-human-review"
  | "approved"
  | "blocked";

export interface CapabilityOwnershipRecord {
  capabilityId: string;
  displayName: string;
  finalOwner: CapabilityOwner;
  r2hUiPaths: string[];
  r2hBackendSymbols: string[];
  bentoToolPaths: string[];
  retainedR2hConsumers: string[];
  unresolvedConsumers: string[];
  deletionSafety: DeletionSafety;
  evidence: string[];
  reviewStatus: OwnershipReviewStatus;
}

export type OwnershipValidationIssueCode =
  | "EMPTY_CAPABILITY_ID"
  | "DUPLICATE_CAPABILITY_ID"
  | "BENTO_OWNER_WITHOUT_TOOL"
  | "R2H_OWNER_WITHOUT_SURFACE"
  | "BACKEND_SAFE_WITH_CONSUMERS"
  | "BACKEND_SAFE_WITH_UNRESOLVED_CONSUMERS"
  | "MISSING_EVIDENCE"
  | "OWNERSHIP_REVIEW_INCOMPLETE";

export interface OwnershipValidationIssue {
  code: OwnershipValidationIssueCode;
  capabilityId: string;
  message: string;
}
