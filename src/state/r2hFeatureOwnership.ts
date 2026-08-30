/**
 * Current product ownership contract for document features that are exposed
 * by the R2H.AI-PDF production shell. The registry is intentionally metadata:
 * the existing panels, controllers, typed IPC wrappers, and Rust commands
 * remain the implementation authorities.
 */
export type R2hFeatureId =
  | "ocr"
  | "compare"
  | "forms"
  | "page-organizer"
  | "sign-stamp"
  | "local-generation";

export type R2hFeatureTabId =
  | "ocr"
  | "compare"
  | "forms"
  | "organizer"
  | "sign-stamp"
  | "local-generation";

export interface R2hFeatureOwnership {
  featureId: R2hFeatureId;
  canonicalOwner: "R2H.AI-PDF";
  tabId: R2hFeatureTabId;
  tabShortLabel: string;
  tabLongLabel: string;
  commandId: string;
  commandTitle: string;
  commandDescription: string;
  panel: string;
  controller: string;
  ipcWrappers: readonly string[];
  backendCommands: readonly string[];
  requiresDocument: true;
  syntheticRoute: false;
  /** Command and tab entries are two entry affordances for one route. */
  duplicateProductionPathCount: 0;
}

export const R2H_FEATURE_OWNERSHIP: readonly R2hFeatureOwnership[] = [
  {
    featureId: "ocr",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "ocr",
    tabShortLabel: "OCR",
    tabLongLabel: "OCR",
    commandId: "cmd.open.ocr",
    commandTitle: "Open OCR",
    commandDescription: "Run local OCR on the active PDF",
    panel: "OcrPanel",
    controller: "useOcr",
    ipcWrappers: [
      "ocrCheckAvailability",
      "ocrRunPage",
      "ocrCancel",
      "pdfCreateOcrEditableOverlays",
    ],
    backendCommands: [
      "ocr_check_availability",
      "ocr_run_page",
      "ocr_cancel",
      "pdf_create_ocr_editable_overlays",
    ],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
  {
    featureId: "compare",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "compare",
    tabShortLabel: "Compare",
    tabLongLabel: "Compare",
    commandId: "cmd.open.compare",
    commandTitle: "Compare Documents",
    commandDescription: "Compare the active PDF with another PDF",
    panel: "ComparePanel",
    controller: "ComparePanel",
    ipcWrappers: ["pdfCompareDocuments", "reportExportCompareReview", "aiReviewCompareResult"],
    backendCommands: ["pdf_compare_documents", "report_export_compare_review", "ai_review_compare_result"],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
  {
    featureId: "forms",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "forms",
    tabShortLabel: "Forms",
    tabLongLabel: "Forms",
    commandId: "cmd.open.forms",
    commandTitle: "Open Forms",
    commandDescription: "Inspect supported form fields in the active PDF",
    panel: "FormsPanel",
    controller: "FormsPanel",
    ipcWrappers: ["docListFormFields", "pdfCreateFormField", "pdfDeleteFormField", "pdfUpdateFormFieldProperties"],
    backendCommands: ["doc_list_form_fields", "pdf_create_form_field", "pdf_delete_form_field", "pdf_update_form_field_properties"],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
  {
    featureId: "page-organizer",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "organizer",
    tabShortLabel: "Pages",
    tabLongLabel: "Organize Pages",
    commandId: "cmd.open.pageOrganizer",
    commandTitle: "Organize Pages",
    commandDescription: "Reorder, rotate, insert, or delete PDF pages",
    panel: "PageOrganizer",
    controller: "usePageOrganizer",
    ipcWrappers: ["editApplyTransaction", "getSessionState"],
    backendCommands: ["edit_apply_transaction", "get_session_state"],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
  {
    featureId: "sign-stamp",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "sign-stamp",
    tabShortLabel: "Sign",
    tabLongLabel: "Sign & Stamp",
    commandId: "cmd.open.signStamp",
    commandTitle: "Open Sign and Stamp",
    commandDescription: "Place visual signatures or stamps",
    panel: "SignStampPanel",
    controller: "SignStampPanel → usePdfSave/usePdfExport",
    ipcWrappers: ["projectSave", "docIncrementalSave", 'invokeSafe<ExportResult>("doc_export")'],
    backendCommands: ["project_save", "doc_incremental_save", "doc_export"],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
  {
    featureId: "local-generation",
    canonicalOwner: "R2H.AI-PDF",
    tabId: "local-generation",
    tabShortLabel: "Local AI",
    tabLongLabel: "Local Generation",
    commandId: "cmd.open.localGeneration",
    commandTitle: "Open Local Generation",
    commandDescription: "Generate document-context text with the local runtime",
    panel: "LocalAiPanel",
    controller: "useLocalAiRuntime",
    ipcWrappers: ['invokeSafe<LocalGenerateResult>("ai_generate_local")'],
    backendCommands: ["ai_generate_local"],
    requiresDocument: true,
    syntheticRoute: false,
    duplicateProductionPathCount: 0,
  },
];
