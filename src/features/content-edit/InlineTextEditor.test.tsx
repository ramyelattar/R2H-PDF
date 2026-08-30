import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { InlineTextEditor, type InlineMethodPreview } from "./InlineTextEditor";

const baseProps = {
  bbox: [72, 700, 200, 720] as [number, number, number, number],
  pageHeightPts: 792,
  zoom: 1,
  initialText: "Hello",
  fontSize: 12,
};

describe("Phase 29E — InlineTextEditor", () => {
  it("shows the Native In-Place badge for safe edits", () => {
    const preview: InlineMethodPreview = {
      strategy: "native_in_place",
      fontPreserved: true,
      reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByTestId("inline-method-native")).toBeDefined();
    expect(screen.queryByTestId("inline-method-visual")).toBeNull();
    expect(screen.queryByTestId("inline-method-readonly")).toBeNull();
  });

  it("shows the Safe Visual badge + warning reasons for fallback", () => {
    const preview: InlineMethodPreview = {
      strategy: "safe_visual_replacement",
      fontPreserved: false,
      reasons: ["Subset font detected; using visual replacement."],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByTestId("inline-method-visual")).toBeDefined();
    const reasons = screen.getByTestId("inline-text-editor-reasons");
    expect(reasons.textContent?.toLowerCase()).toContain("subset");
  });

  it("disables apply and shows Read Only badge for read-only text", () => {
    const preview: InlineMethodPreview = {
      strategy: "read_only",
      fontPreserved: false,
      reasons: ["Marked read-only."],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByTestId("inline-method-readonly")).toBeDefined();
    const textarea = screen.getByTestId<HTMLTextAreaElement>("inline-text-editor-textarea");
    expect(textarea.disabled).toBe(true);
    // No apply button rendered.
    expect(screen.queryByTestId("inline-text-editor-apply")).toBeNull();
  });

  it("Escape triggers onCancel", () => {
    const onCancel = vi.fn();
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={onCancel}
      />,
    );
    const textarea = screen.getByTestId<HTMLTextAreaElement>("inline-text-editor-textarea");
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(onCancel).toHaveBeenCalled();
  });

  it("Ctrl+Enter triggers onApply with the current text", async () => {
    const onApply = vi.fn();
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={onApply}
        onCancel={() => {}}
      />,
    );
    const textarea = screen.getByTestId<HTMLTextAreaElement>("inline-text-editor-textarea");
    fireEvent.change(textarea, { target: { value: "Edited text" } });
    fireEvent.keyDown(textarea, { key: "Enter", ctrlKey: true });
    expect(onApply).toHaveBeenCalledWith("Edited text");
  });

  it("apply button is disabled when text equals the initial value", () => {
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    const btn = screen.getByTestId<HTMLButtonElement>("inline-text-editor-apply");
    expect(btn.disabled).toBe(true);
  });

  it("positions itself by flipping PDF y to screen y at zoom 1", () => {
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    const container = screen.getByTestId("inline-text-editor");
    // bbox = [72, 700, 200, 720]; pageH = 792
    // top = (792 - 720) * 1 = 72; left = 72; width = 128.
    expect(container.style.left).toBe("72px");
    expect(container.style.top).toBe("72px");
  });

  it("keeps the editor width aligned to the selected bbox instead of forcing an oversized input", () => {
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    const container = screen.getByTestId("inline-text-editor");
    expect(container.style.width).toBe("128px");
    expect(container.getAttribute("data-overflow-state")).toBe("fits");
  });

  it("warns when replacement text is longer than the original bbox and exposes shrink-to-fit", () => {
    const preview: InlineMethodPreview = {
      strategy: "safe_visual_replacement",
      fontPreserved: false,
      reasons: ["Visual replacement uses white cover."],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        initialText="Short"
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    const textarea = screen.getByTestId<HTMLTextAreaElement>("inline-text-editor-textarea");
    fireEvent.change(textarea, { target: { value: "This replacement is much longer than the selected text box" } });
    expect(screen.getByTestId("inline-text-overflow-warning").textContent).toContain("longer than the selected text box");
    const shrink = screen.getByTestId<HTMLInputElement>("inline-text-shrink-to-fit");
    expect(shrink.disabled).toBe(false);
    fireEvent.click(shrink);
    expect(screen.getByTestId("inline-text-editor").getAttribute("data-overflow-state")).toBe("shrink");
  });

  it("scales position by zoom (Acrobat-UX regression: heightPts must be in points, not pixels)", () => {
    // This test pins the contract that broke the canvas inline editor in
    // production: `pageHeightPts` MUST be the page height in PDF points
    // (792 for US Letter), zoom-independent. Previously the parent was
    // passing the rendered pixmap height (792 * zoom * dpr) which placed
    // the editor at the wrong vertical position at any zoom != 100%.
    const preview: InlineMethodPreview = {
      strategy: "native_in_place", fontPreserved: true, reasons: [],
    };
    render(
      <InlineTextEditor
        {...baseProps}
        zoom={1.5}
        methodPreview={preview}
        onApply={() => {}}
        onCancel={() => {}}
      />,
    );
    const container = screen.getByTestId("inline-text-editor");
    // bbox = [72, 700, 200, 720]; pageH = 792 pts; zoom = 1.5.
    // top = (792 - 720) * 1.5 = 108; left = 72 * 1.5 = 108.
    expect(container.style.left).toBe("108px");
    expect(container.style.top).toBe("108px");
  });
});
