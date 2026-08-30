/**
 * Pass 2B — Model Manager panel polish tests.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === "local_ai_get_status" || cmd === "local_ai_validate") {
      return Promise.resolve({
        local_ai_root: "C:/tmp/local-ai",
        ready: false,
        total_size_bytes: 0,
        required_ok_count: 0,
        required_missing_count: 2,
        optional_missing_count: 1,
        assets: [
          { asset_id: "model.bin", name: "Model weights", category: "model", path: "model.bin", required: true, exists: false, size_bytes: 0, status: "missing", message: "File not found" },
          { asset_id: "tokenizer.json", name: "Tokenizer", category: "model", path: "tokenizer.json", required: true, exists: false, size_bytes: 0, status: "missing", message: "" },
          { asset_id: "extras.bin", name: "Optional extras", category: "extras", path: "extras.bin", required: false, exists: false, size_bytes: 0, status: "optional_missing", message: "" },
        ],
        warnings: ["GPU runtime not detected"],
      });
    }
    return Promise.reject(new Error("not stubbed"));
  }),
}));

import { ModelManagerPanel } from "./ModelManagerPanel";

const flushAsync = async () => {
  for (let i = 0; i < 6; i++) await Promise.resolve();
};

describe("Pass 2B — ModelManagerPanel", () => {
  it("renders the panel title and a status badge", async () => {
    render(<ModelManagerPanel />);
    await flushAsync();
    expect(screen.getByText(/Local AI Runtime/)).toBeTruthy();
    const badge = screen.getByTestId("model-manager-status-badge");
    expect(badge).toBeTruthy();
  });

  it("surfaces the Validate Local AI primary button", async () => {
    render(<ModelManagerPanel />);
    // After the initial status load settles, the button label settles to
    // its idle state.
    await waitFor(() => {
      const btn = screen.getByTestId("model-manager-validate-btn");
      expect(btn.textContent).toMatch(/Validate Local AI/);
    });
  });

  it("shows the status card with the at-a-glance counts", async () => {
    render(<ModelManagerPanel />);
    await flushAsync();
    expect(screen.getByTestId("model-manager-status-card")).toBeTruthy();
  });

  it("renders a missing-required-files callout when assets are missing", async () => {
    render(<ModelManagerPanel />);
    await flushAsync();
    const card = await screen.findByTestId("model-manager-missing-card");
    expect(card.textContent).toMatch(/missing/i);
    expect(card.textContent).toMatch(/Model weights/);
  });

  it("collapses technical paths under an Advanced details expander", async () => {
    render(<ModelManagerPanel />);
    await flushAsync();
    const adv = await screen.findByTestId("model-manager-advanced");
    expect(adv).toBeTruthy();
    expect((adv as HTMLDetailsElement).tagName.toLowerCase()).toBe("details");
    // Closed by default — the user opts in to seeing raw paths.
    expect((adv as HTMLDetailsElement).open).toBe(false);
  });

  it("renders validation warnings as a callout when present", async () => {
    render(<ModelManagerPanel />);
    await flushAsync();
    const warn = await screen.findByTestId("model-manager-warnings");
    expect(warn.textContent).toMatch(/GPU runtime/);
  });
});
