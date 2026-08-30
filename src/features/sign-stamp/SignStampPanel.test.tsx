import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SignStampPanel } from "./SignStampPanel";

// Stub the global Image constructor so file → image dimension probing is
// deterministic and synchronous in tests.
class StubImage {
  naturalWidth = 600;
  naturalHeight = 200;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  set src(_v: string) {
    // fire onload synchronously on next microtask
    queueMicrotask(() => {
      this.onload?.();
    });
  }
}
// @ts-expect-error replace global for test
globalThis.Image = StubImage as unknown as typeof Image;

// Stub FileReader to deliver a tiny data URL synchronously.
class StubFileReader {
  result: string | ArrayBuffer | null = null;
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  readAsDataURL(_blob: Blob) {
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const _ignore = _blob;
    this.result =
      "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";
    queueMicrotask(() => this.onload?.());
  }
}
// @ts-expect-error replace global for test
globalThis.FileReader = StubFileReader as unknown as typeof FileReader;

describe("Phase 27A — SignStampPanel preserve-aspect checkbox", () => {
  it("does not show the preserve-aspect checkbox before an image is chosen", () => {
    render(
      <SignStampPanel
        sessionId="s1"
        activePageIndex={0}
        onAddOverlay={() => {}}
      />,
    );
    expect(screen.queryByTestId("signature-preserve-aspect")).toBeNull();
  });

  it("defaults preserve-aspect to checked after an image is loaded", async () => {
    render(
      <SignStampPanel
        sessionId="s1"
        activePageIndex={0}
        onAddOverlay={() => {}}
      />,
    );
    const fileInput = screen.getByTestId("signature-file-input") as HTMLInputElement;
    const file = new File(["x"], "alice.png", { type: "image/png" });
    Object.defineProperty(fileInput, "files", { value: [file] });
    fireEvent.change(fileInput);

    // Wait for the StubFileReader / StubImage microtasks.
    await new Promise((r) => queueMicrotask(() => r(undefined)));
    await new Promise((r) => queueMicrotask(() => r(undefined)));

    const cb = await screen.findByTestId<HTMLInputElement>("signature-preserve-aspect");
    expect(cb).toBeDefined();
    expect(cb.checked).toBe(true);
  });

  it("emits an overlay with metadata.preserve_aspect set to the current checkbox state", async () => {
    const added: unknown[] = [];
    render(
      <SignStampPanel
        sessionId="s1"
        activePageIndex={3}
        onAddOverlay={(o) => added.push(o)}
      />,
    );
    const fileInput = screen.getByTestId("signature-file-input") as HTMLInputElement;
    const file = new File(["x"], "alice.png", { type: "image/png" });
    Object.defineProperty(fileInput, "files", { value: [file] });
    fireEvent.change(fileInput);

    await new Promise((r) => queueMicrotask(() => r(undefined)));
    await new Promise((r) => queueMicrotask(() => r(undefined)));

    // Toggle off, then place — overlay should carry preserve_aspect=false.
    const cb = await screen.findByTestId<HTMLInputElement>("signature-preserve-aspect");
    fireEvent.click(cb); // → unchecked
    expect(cb.checked).toBe(false);

    fireEvent.click(screen.getByTestId("signature-place"));
    expect(added).toHaveLength(1);
    const overlay = added[0] as { metadata: Record<string, unknown> };
    expect(overlay.metadata.preserve_aspect).toBe(false);
    expect(overlay.metadata.image_natural_width).toBe(600);
    expect(overlay.metadata.image_natural_height).toBe(200);
  });

  it("by default emits an overlay with preserve_aspect=true and shows the fitted-rect hint when shape mismatches", async () => {
    const added: unknown[] = [];
    render(
      <SignStampPanel
        sessionId="s1"
        activePageIndex={0}
        onAddOverlay={(o) => added.push(o)}
      />,
    );
    const fileInput = screen.getByTestId("signature-file-input") as HTMLInputElement;
    const file = new File(["x"], "alice.png", { type: "image/png" });
    Object.defineProperty(fileInput, "files", { value: [file] });
    fireEvent.change(fileInput);

    await new Promise((r) => queueMicrotask(() => r(undefined)));
    await new Promise((r) => queueMicrotask(() => r(undefined)));

    // The default sigRect is 220×60 → aspect 3.66; image is 600×200 → aspect 3.
    // Mismatch should trigger the fitted-rect hint.
    expect(await screen.findByTestId("signature-fitted-rect")).toBeDefined();

    fireEvent.click(screen.getByTestId("signature-place"));
    expect(added).toHaveLength(1);
    const overlay = added[0] as { metadata: Record<string, unknown> };
    expect(overlay.metadata.preserve_aspect).toBe(true);
  });
});

// Suppress unused-var noise.
void vi;
