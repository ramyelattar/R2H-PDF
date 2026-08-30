import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CommandPalette } from "../overlays/CommandPalette";
import { commandCatalog } from "../../state/shellCatalog";

const here = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(here, "../../..");

const readSource = (relativePath: string): string => {
  const absolutePath = path.join(projectRoot, relativePath);
  return fs.existsSync(absolutePath) ? fs.readFileSync(absolutePath, "utf8") : "";
};

const walkProductionFiles = (directory: string): string[] => {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) return walkProductionFiles(absolutePath);
    if (/\.(test|spec)\.(ts|tsx)$/.test(entry.name)) return [];
    return [absolutePath];
  });
};

const EXPECTED_FEATURES = [
  {
    featureId: "ocr",
    tabId: "ocr",
    commandId: "cmd.open.ocr",
    panel: "OcrPanel",
    panelFile: "src/features/ocr/OcrPanel.tsx",
    controller: "useOcr",
    controllerFile: "src/features/ocr/useOcr.ts",
    ipcTokens: ["ocr_check_availability", "ocr_run_page", "pdfCreateOcrEditableOverlays"],
    backendTokens: ["ocr_check_availability", "ocr_run_page", "pdf_create_ocr_editable_overlays"],
    commandTitle: "Open OCR",
  },
  {
    featureId: "compare",
    tabId: "compare",
    commandId: "cmd.open.compare",
    panel: "ComparePanel",
    panelFile: "src/features/compare/ComparePanel.tsx",
    controller: "ComparePanel",
    controllerFile: "src/features/compare/ComparePanel.tsx",
    ipcTokens: ["pdfCompareDocuments"],
    backendTokens: ["pdf_compare_documents"],
    commandTitle: "Compare Documents",
  },
  {
    featureId: "forms",
    tabId: "forms",
    commandId: "cmd.open.forms",
    panel: "FormsPanel",
    panelFile: "src/features/forms/FormsPanel.tsx",
    controller: "FormsPanel",
    controllerFile: "src/features/forms/FormsPanel.tsx",
    ipcTokens: ["docListFormFields", "pdfCreateFormField", "pdfUpdateFormFieldProperties"],
    backendTokens: ["doc_list_form_fields", "pdf_create_form_field", "pdf_update_form_field_properties"],
    commandTitle: "Open Forms",
  },
  {
    featureId: "page-organizer",
    tabId: "organizer",
    commandId: "cmd.open.pageOrganizer",
    panel: "PageOrganizer",
    panelFile: "src/features/page-organizer/PageOrganizer.tsx",
    controller: "usePageOrganizer",
    controllerFile: "src/features/page-organizer/usePageOrganizer.ts",
    ipcTokens: ["editApplyTransaction", "getSessionState"],
    backendTokens: ["edit_apply_transaction", "get_session_state"],
    commandTitle: "Organize Pages",
  },
  {
    featureId: "sign-stamp",
    tabId: "sign-stamp",
    commandId: "cmd.open.signStamp",
    panel: "SignStampPanel",
    panelFile: "src/features/sign-stamp/SignStampPanel.tsx",
    controller: "SignStampPanel → usePdfSave/usePdfExport",
    controllerFile: "src/hooks/usePdfSave.ts",
    ipcTokens: ["projectSave", "doc_incremental_save"],
    backendTokens: ["project_save", "doc_incremental_save"],
    commandTitle: "Open Sign and Stamp",
  },
  {
    featureId: "local-generation",
    tabId: "local-generation",
    commandId: "cmd.open.localGeneration",
    panel: "LocalAiPanel",
    panelFile: "src/features/ai-local/LocalAiPanel.tsx",
    controller: "useLocalAiRuntime",
    controllerFile: "src/features/ai-local/useLocalAiRuntime.ts",
    ipcTokens: ["ai_generate_local"],
    backendTokens: ["ai_generate_local"],
    commandTitle: "Open Local Generation",
  },
] as const;

const appSource = readSource("src/App.tsx");
const inspectorSource = readSource("src/components/shell/RightInspector.tsx");
const menuSource = readSource("src/hooks/useMenuActions.ts");
const catalogSource = readSource("src/state/shellCatalog.ts");
const commandPaletteSource = readSource("src/components/overlays/CommandPalette.tsx");
const ipcSource = readSource("src/lib/ipc.ts");
const backendRegistrationSource = readSource("src-tauri/src/lib.rs");
const ownershipSource = readSource("src/state/r2hFeatureOwnership.ts");
const ownershipContract = readSource("audit-output/remediation-phase-2a-r2h-feature-ownership/ownership_contract.md");
const ownershipOverrides = JSON.parse(readSource("scripts/bentopdf/duplicate-ownership-overrides.json"));

describe("R2H feature ownership contract", () => {
  it("declares exactly one canonical R2H panel/controller record for each feature", () => {
    expect(ownershipSource).toContain('canonicalOwner: "R2H.AI-PDF"');
    expect((ownershipSource.match(/featureId:/g) ?? []).length - 1).toBe(EXPECTED_FEATURES.length);
    expect((ownershipSource.match(/duplicateProductionPathCount:/g) ?? []).length - 1).toBe(EXPECTED_FEATURES.length);

    const seenFeatures = new Set<string>();
    const seenTabs = new Set<string>();
    const seenCommands = new Set<string>();
    const seenPanels = new Set<string>();
    const seenControllers = new Set<string>();

    for (const feature of EXPECTED_FEATURES) {
      expect(ownershipSource).toContain(`featureId: "${feature.featureId}"`);
      expect(ownershipSource).toContain(`tabId: "${feature.tabId}"`);
      expect(ownershipSource).toContain(`commandId: "${feature.commandId}"`);
      expect(ownershipSource).toContain(`panel: "${feature.panel}"`);
      expect(ownershipSource).toContain(`controller: "${feature.controller}"`);
      expect(ownershipSource).toContain("requiresDocument: true");
      expect(ownershipSource).toContain("syntheticRoute: false");
      seenFeatures.add(feature.featureId);
      seenTabs.add(feature.tabId);
      seenCommands.add(feature.commandId);
      seenPanels.add(feature.panel);
      seenControllers.add(feature.controller);
    }

    expect(seenFeatures).toHaveLength(EXPECTED_FEATURES.length);
    expect(seenTabs).toHaveLength(EXPECTED_FEATURES.length);
    expect(seenCommands).toHaveLength(EXPECTED_FEATURES.length);
    expect(seenPanels).toHaveLength(EXPECTED_FEATURES.length);
    expect(seenControllers).toHaveLength(EXPECTED_FEATURES.length);
  });

  it("keeps the six command-palette entries and inspector entries on the active App graph", () => {
    expect(appSource).toContain('import { RightInspector');
    expect(appSource).toContain("<RightInspector");
    expect(inspectorSource).toContain('import { OcrPanel }');
    expect(inspectorSource).toContain('import { ComparePanel }');
    expect(inspectorSource).toContain('import { FormsPanel }');
    expect(inspectorSource).toContain('import { PageOrganizer }');
    expect(inspectorSource).toContain('import { SignStampPanel }');
    expect(inspectorSource).toContain('import { LocalAiPanel }');

    const featureCommands = commandCatalog.filter((command) =>
      EXPECTED_FEATURES.some((feature) => feature.commandId === command.id),
    );
    expect(featureCommands).toHaveLength(EXPECTED_FEATURES.length);
    expect(new Set(featureCommands.map((command) => command.id))).toHaveLength(EXPECTED_FEATURES.length);

    for (const feature of EXPECTED_FEATURES) {
      const command = featureCommands.find((entry) => entry.id === feature.commandId);
      expect(command?.title).toBe(feature.commandTitle);
      expect(command?.requiresDocument).toBe(true);
      expect(inspectorSource).toContain(`tab === "${feature.tabId}"`);
      expect(inspectorSource).toContain(`<${feature.panel}`);
    }

    expect(menuSource).toContain("R2H_FEATURE_OWNERSHIP.find");
    expect(menuSource).toContain("if (r2hFeature) d.requestInspectorTab(r2hFeature.tabId)");
    expect(catalogSource).not.toMatch(/BentoPDF|bentopdf/i);
    expect(menuSource).not.toMatch(/BentoPDF|bentopdf/i);
    expect(inspectorSource).not.toMatch(/BentoPDF|bentopdf/i);
  });

  it("traces every active feature panel/controller through typed IPC and registered backend commands", () => {
    for (const feature of EXPECTED_FEATURES) {
      const panelSource = readSource(feature.panelFile);
      const controllerSource = readSource(feature.controllerFile);
      const routeSource = `${panelSource}\n${controllerSource}`;

      expect(routeSource).toContain(feature.panel);
      expect(routeSource).toContain(feature.controller.split(" → ")[0]);
      for (const token of feature.ipcTokens) {
        expect(`${routeSource}\n${ipcSource}`, `${feature.featureId} IPC token ${token}`).toContain(token);
      }
      for (const token of feature.backendTokens) {
        expect(backendRegistrationSource, `${feature.featureId} backend token ${token}`).toContain(token);
      }
      if (feature.featureId === "sign-stamp") {
        expect(readSource("src/features/export/usePdfExport.ts")).toContain('invokeSafe<ExportResult>("doc_export"');
        expect(backendRegistrationSource).toContain("doc_export");
      }
    }
  });

  it("does not use synthetic session routes and leaves exactly one production panel file per feature", () => {
    const activeFeatureSources = [
      inspectorSource,
      menuSource,
      catalogSource,
      ...EXPECTED_FEATURES.flatMap((feature) => [readSource(feature.panelFile), readSource(feature.controllerFile)]),
    ];
    for (const source of activeFeatureSources) {
      expect(source).not.toContain("session://");
    }

    const productionFeatureFiles = walkProductionFiles(path.join(projectRoot, "src/features"));
    for (const feature of EXPECTED_FEATURES) {
      const fileName = path.basename(feature.panelFile);
      expect(productionFeatureFiles.filter((file) => path.basename(file) === fileName)).toHaveLength(1);
    }
  });

  it("keeps document-dependent command and inspector entries disabled and execution-guarded without a PDF", () => {
    expect(inspectorSource).toContain("if (!canAccessTab(next))");
    expect(inspectorSource).toContain("disabled={disabled}");
    expect(menuSource).toContain("command.requiresDocument && !isBackendPdfSession(d.activeTab)");
    expect(commandPaletteSource).toContain("disabled={isCommandAvailable ? !isCommandAvailable(entry) : false}");
    expect(commandPaletteSource).toContain("if (isCommandAvailable && !isCommandAvailable(entry)) return;");

    const onExecute = vi.fn();
    render(createElement(CommandPalette, {
      open: true,
      commands: commandCatalog,
      onClose: vi.fn(),
      onExecute,
      isCommandAvailable: (command) => !command.requiresDocument,
    }));

    for (const feature of EXPECTED_FEATURES) {
      const button = screen.getByRole("button", { name: new RegExp(feature.commandTitle) });
      expect((button as HTMLButtonElement).disabled).toBe(true);
      fireEvent.click(button);
      fireEvent.keyDown(button, { key: "Enter", code: "Enter" });
    }
    expect(onExecute).not.toHaveBeenCalled();
  });

  it("keeps BentoPDF as an optional independent surface, never the six-feature owner", () => {
    const overlappingIds = new Set([
      "ocr-pdf",
      "compare-pdfs",
      "form-creator",
      "form-filler",
      "organize-pdf",
      "sign-pdf",
      "add-stamps",
    ]);
    const overlapping = ownershipOverrides.capabilities.filter((entry: { capabilityId: string }) => overlappingIds.has(entry.capabilityId));
    expect(overlapping).toHaveLength(overlappingIds.size);
    expect(overlapping.every((entry: { finalOwner: string }) => entry.finalOwner === "r2h")).toBe(true);
    expect(overlapping.every((entry: { rationale: string }) => /separate optional surface/i.test(entry.rationale))).toBe(true);
    expect(appSource).toContain("BentoPDF remains a separate optional utility surface");
    expect(appSource).not.toContain("BentoPDF is the canonical owner");
    expect(ownershipContract).toContain("R2H.AI-PDF is the canonical owner");
    expect(ownershipContract).toContain("BentoPDF remains a separate optional surface");
  });

  it("preserves visual Sign & Stamp terminology without promoting certificate signing", () => {
    const signSource = readSource("src/features/sign-stamp/SignStampPanel.tsx");
    const command = commandCatalog.find((entry) => entry.id === "cmd.open.signStamp");
    expect(command?.description).toMatch(/visual signatures or stamps/i);
    expect(signSource).toContain("Visual signature only");
    expect(signSource).toContain("Sign &amp; Stamp");
  });
});
