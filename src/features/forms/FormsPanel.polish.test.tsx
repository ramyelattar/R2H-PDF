import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("../../lib/ipc", () => ({
  docListFormFields: vi.fn(async () => ({
    ok: true,
    data: [
      { name: "ClientName", field_type: "text", value: "Acme", page_index: 0, rect: [0, 0, 100, 20] },
      { name: "Accepted", field_type: "checkbox", value: "Yes", page_index: 1, rect: [0, 0, 20, 20] },
    ],
  })),
  editApplyTransaction: vi.fn(),
  pdfCreateFormField: vi.fn(),
  pdfDeleteFormField: vi.fn(),
  pdfUpdateFormFieldProperties: vi.fn(),
}));

import { FormsPanel } from "./FormsPanel";

describe("Pass 2D — FormsPanel polish", () => {
  it("renders professional field rows and visible create actions", async () => {
    render(<FormsPanel sessionId="doc-session-test" pageIndex={0} />);

    expect(screen.getByTestId("forms-create-text").textContent).toMatch(/Create text field/);
    expect(screen.getByTestId("forms-create-checkbox").textContent).toMatch(/Create checkbox/);
    await waitFor(() => expect(screen.getByText("ClientName")).toBeTruthy());
    expect(screen.getByText("Text")).toBeTruthy();
    expect(screen.getByText(/Value: Acme/)).toBeTruthy();
    expect(screen.getAllByTestId("forms-delete-field")[0].textContent).toMatch(/Delete/);
  });
});
