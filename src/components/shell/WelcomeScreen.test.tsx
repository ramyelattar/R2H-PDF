/**
 * Phase UX — welcome screen tests.
 *
 * Pins the discoverability contract for the empty-state landing screen:
 *   - hero, primary CTA, secondary CTA, and retained workflow cards must render
 *   - clicking the primary CTA opens the file dialog
 *   - clicking "Validate Local AI" jumps to the Models tab
 *   - workflow cards both record the picked tab and open the file dialog
 *   - no fake/dead buttons are present
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WelcomeScreen } from "./WelcomeScreen";
import type { InspectorTab } from "./RightInspector";

const renderWelcome = (overrides: Partial<{
  onOpenFile: () => void;
  onOpenBentoPdf: () => void;
  onShowInspectorTab: (tab: InspectorTab) => void;
  onPickWorkflow: (tab: InspectorTab) => void;
}> = {}) => {
  const onOpenFile = overrides.onOpenFile ?? vi.fn();
  const onOpenBentoPdf = overrides.onOpenBentoPdf ?? vi.fn();
  const onShowInspectorTab = overrides.onShowInspectorTab ?? vi.fn();
  const onPickWorkflow = overrides.onPickWorkflow ?? vi.fn();
  render(
    <WelcomeScreen
      onOpenFile={onOpenFile}
      onOpenBentoPdf={onOpenBentoPdf}
      onShowInspectorTab={onShowInspectorTab}
      onPickWorkflow={onPickWorkflow}
    />,
  );
  return { onOpenFile, onOpenBentoPdf, onShowInspectorTab, onPickWorkflow };};

describe("Phase UX — WelcomeScreen", () => {
  it("renders the hero with app name, subtitle, and primary CTA", () => {
    renderWelcome();
    expect(screen.getByText(/R2H PDF AI Workstation/)).toBeTruthy();
    expect(screen.getByText(/Edit, review, compare, OCR/)).toBeTruthy();
    expect(screen.getByTestId("welcome-open-pdf")).toBeTruthy();
  });

  it("renders the Validate Local AI secondary CTA", () => {
    renderWelcome();
    expect(screen.getByTestId("welcome-validate-ai")).toBeTruthy();
  });

  it("opens the real local BentoPDF toolkit callback", () => {
    const { onOpenBentoPdf } = renderWelcome();
    fireEvent.click(screen.getByTestId("welcome-open-bentopdf"));
    expect(onOpenBentoPdf).toHaveBeenCalledTimes(1);
  });

  it("renders retained workflow cards (Edit / AI / Export)", () => {
    renderWelcome();
    for (const id of [
      "welcome-card-edit",
      "welcome-card-ai",
      "welcome-card-export",
    ]) {
      expect(screen.getByTestId(id), `${id} card should render`).toBeTruthy();
    }
  });

  it("clicking 'Open PDF' calls onOpenFile", () => {
    const { onOpenFile } = renderWelcome();
    fireEvent.click(screen.getByTestId("welcome-open-pdf"));
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });

  it("clicking 'Validate Local AI' jumps to the Models tab", () => {
    const { onShowInspectorTab } = renderWelcome();
    fireEvent.click(screen.getByTestId("welcome-validate-ai"));
    expect(onShowInspectorTab).toHaveBeenCalledWith("models");
  });

  it("clicking a workflow card records the picked tab AND opens the file dialog", () => {
    const { onOpenFile, onPickWorkflow } = renderWelcome();
    fireEvent.click(screen.getByTestId("welcome-card-edit"));
    expect(onPickWorkflow).toHaveBeenCalledWith("edit");
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });

  it.each<[string, InspectorTab]>([
    ["welcome-card-ai", "ai"],
    ["welcome-card-export", "export"],
  ])("workflow card %s maps to the %s inspector tab", (testId, tab) => {
    const { onPickWorkflow } = renderWelcome();
    fireEvent.click(screen.getByTestId(testId));
    expect(onPickWorkflow).toHaveBeenCalledWith(tab);
  });

  it("every visible button has a working onClick handler — no dead buttons", () => {
    const onOpenFile = vi.fn();
    const onOpenBentoPdf = vi.fn();
    const onShowInspectorTab = vi.fn();
    const onPickWorkflow = vi.fn();
    render(
      <WelcomeScreen
        onOpenFile={onOpenFile}
          onOpenBentoPdf={onOpenBentoPdf}
        onShowInspectorTab={onShowInspectorTab}
        onPickWorkflow={onPickWorkflow}
      />,
    );
    const buttons = screen.getAllByRole("button");
    expect(buttons.length).toBeGreaterThan(0);
    for (const btn of buttons) {
      fireEvent.click(btn);
    }
    const totalCalls =
      onOpenFile.mock.calls.length +
      onOpenBentoPdf.mock.calls.length +
      onShowInspectorTab.mock.calls.length +
      onPickWorkflow.mock.calls.length;
    expect(totalCalls).toBeGreaterThanOrEqual(buttons.length);
  });
});
