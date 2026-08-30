import type { Citation } from "../rag/types";

export interface ReviewFinding {
  finding_id: string;
  title: string;
  description: string;
  severity: "info" | "warning" | "major" | "critical";
  category: string;
  page_refs: number[];
  citations: Citation[];
  recommendation: string;
  confidence: number;
}

export interface ReviewSuggestedAction {
  action_id: string;
  action_type: string;
  page_index: number;
  text: string;
  reason: string;
  citations: Citation[];
  confidence: number;
}

export interface DocumentReviewResult {
  review_id: string;
  session_id: string;
  summary: string;
  findings: ReviewFinding[];
  risks: ReviewFinding[];
  missing_information: ReviewFinding[];
  engineering_findings: ReviewFinding[];
  suggested_actions: ReviewSuggestedAction[];
  citations: Citation[];
  warnings: string[];
  elapsed_ms: number;
  created_at: number;
}

export type ReviewStatus = "idle" | "running" | "completed" | "failed";
