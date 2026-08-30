import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SignStampPanel } from "./SignStampPanel";

describe("Pass 2D — SignStampPanel polish", () => {
  it("renders signature warning, preset cards, and image card actions", () => {
    render(
      <SignStampPanel
        sessionId="doc-session-test"
        activePageIndex={0}
        onAddOverlay={vi.fn()}
      />,
    );

    expect(screen.getByTestId("signature-warning").textContent).toMatch(/not a certificate-based digital signature/i);
    expect(screen.getByTestId("stamp-presets").textContent).toMatch(/Reviewed/);
    expect(screen.getByTestId("signature-pick").textContent).toMatch(/Choose Image/);
  });
});
