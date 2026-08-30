import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { PdfCanvasViewer } from "./PdfCanvasViewer";
import type { RenderResponse } from "../../lib/ipc";

vi.mock("../../lib/ipc", async () => {
  const actual = await vi.importActual<typeof import("../../lib/ipc")>(
    "../../lib/ipc",
  );
  return {
    ...actual,
    docRender: vi.fn(),
  };
});

// Pull the typed mock back out so we can program it per test.
import { docRender } from "../../lib/ipc";
const docRenderMock = docRender as unknown as ReturnType<typeof vi.fn>;

// happy-dom's HTMLCanvasElement.getContext() always returns null, which makes
// the viewer abort with "2D canvas context is unavailable." before it can
// paint anything. For these tests we install a minimal fake 2D context that
// records calls so we can verify the render actually reached putImageData.
const originalGetContext = HTMLCanvasElement.prototype.getContext;
let lastPutImageData: { width: number; height: number; bytes: number } | null = null;
function installFake2dContext() {
  lastPutImageData = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (HTMLCanvasElement.prototype as any).getContext = function (
    contextType: string,
  ) {
    if (contextType !== "2d") return null;
    return {
      clearRect() {},
      fillRect() {},
      strokeRect() {},
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      putImageData(imageData: ImageData) {
        lastPutImageData = {
          width: imageData.width,
          height: imageData.height,
          bytes: imageData.data.length,
        };
      },
    };
  };
}
function restoreCanvasContext() {
  HTMLCanvasElement.prototype.getContext = originalGetContext;
}

// happy-dom does not implement ImageData; install a minimal polyfill that
// the viewer can construct.
if (typeof globalThis.ImageData === "undefined") {
  class FakeImageData {
    data: Uint8ClampedArray;
    width: number;
    height: number;
    constructor(data: Uint8ClampedArray, width: number, height: number) {
      if (data.length !== width * height * 4) {
        throw new Error(
          `ImageData length mismatch: got ${data.length}, expected ${width * height * 4}`,
        );
      }
      this.data = data;
      this.width = width;
      this.height = height;
    }
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).ImageData = FakeImageData;
}

/**
 * Build a synthetic RenderResponse with a base64 pixel buffer of the requested
 * raw byte length. This lets us simulate the same multi-megabyte payloads the
 * real backend emits, which is what previously triggered the CSP-blocked
 * fetch(data:) fallback.
 */
function buildRenderResponse(width: number, height: number): RenderResponse {
  const len = width * height * 4;
  const raw = new Uint8Array(len);
  // Fill with a non-white pattern so a real putImageData would visibly paint.
  for (let i = 0; i < len; i += 4) {
    raw[i] = 0x33;
    raw[i + 1] = 0x66;
    raw[i + 2] = 0x99;
    raw[i + 3] = 0xff;
  }
  // Chunked base64 encoding to avoid call-stack overflow on large arrays.
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < raw.length; i += chunk) {
    binary += String.fromCharCode(...raw.subarray(i, i + chunk));
  }
  return {
    session_id: "doc-session-test",
    page_index: 0,
    width_px: width,
    height_px: height,
    width,
    height,
    zoom: 1,
    cache_hit: false,
    pixels_rgba: btoa(binary),
    render_time_ms: 5,
  };
}

beforeEach(() => {
  docRenderMock.mockReset();
  installFake2dContext();
});

afterEach(() => {
  restoreCanvasContext();
});

describe("PdfCanvasViewer render lifecycle", () => {
  it("clears the loading overlay and calls onRenderSuccess on a normal render", async () => {
    docRenderMock.mockResolvedValueOnce({
      ok: true,
      data: buildRenderResponse(64, 48),
    });

    const onRenderSuccess = vi.fn();
    const onRenderError = vi.fn();

    const { container } = render(
      <PdfCanvasViewer
        sessionId="doc-session-test"
        pageIndex={0}
        zoomPercent={100}
        onRenderError={onRenderError}
        onRenderSuccess={onRenderSuccess}
      />,
    );

    await waitFor(() => expect(onRenderSuccess).toHaveBeenCalledTimes(1));
    expect(onRenderError).not.toHaveBeenCalled();
    // Loading overlay must be gone — "Rendering page…" must not be stuck.
    expect(container.textContent ?? "").not.toContain("Rendering page");
    // Pixels were actually painted to the canvas.
    expect(lastPutImageData).not.toBeNull();
    expect(lastPutImageData!.width).toBe(64);
    expect(lastPutImageData!.height).toBe(48);
    expect(lastPutImageData!.bytes).toBe(64 * 48 * 4);
  });

  // REGRESSION: previously the >= 64 KB base64 path used fetch("data:...")
  // which is blocked by the packaged-app CSP. That blocked the render and
  // left "Rendering page…" stuck forever. This test feeds the viewer a
  // payload of the same size class and asserts a clean success path with
  // no fetch involvement.
  it("renders a multi-megabyte payload without touching fetch (CSP-safe)", async () => {
    // 612 × 792 × 4 bytes ≈ 1.94 MB raw → ~2.6 MB base64 — well past 64 KB.
    docRenderMock.mockResolvedValueOnce({
      ok: true,
      data: buildRenderResponse(612, 792),
    });

    const originalFetch = globalThis.fetch;
    let fetchCalled = false;
    globalThis.fetch = (() => {
      fetchCalled = true;
      throw new Error("PdfCanvasViewer must not call fetch during render");
    }) as typeof fetch;

    const onRenderSuccess = vi.fn();
    const onRenderError = vi.fn();

    try {
      render(
        <PdfCanvasViewer
          sessionId="doc-session-test"
          pageIndex={0}
          zoomPercent={100}
          onRenderError={onRenderError}
          onRenderSuccess={onRenderSuccess}
        />,
      );

      await waitFor(() => expect(onRenderSuccess).toHaveBeenCalledTimes(1));
    } finally {
      globalThis.fetch = originalFetch;
    }

    expect(fetchCalled).toBe(false);
    expect(onRenderError).not.toHaveBeenCalled();
    expect(lastPutImageData).not.toBeNull();
    expect(lastPutImageData!.bytes).toBe(612 * 792 * 4);
  });

  it("surfaces backend errors via onRenderError and clears the loading overlay", async () => {
    docRenderMock.mockResolvedValueOnce({
      ok: false,
      error: { code: "SESSION_NOT_FOUND", message: "session not found" },
    });

    const onRenderError = vi.fn();

    const { container } = render(
      <PdfCanvasViewer
        sessionId="doc-session-stale"
        pageIndex={0}
        zoomPercent={100}
        onRenderError={onRenderError}
      />,
    );

    await waitFor(() => expect(onRenderError).toHaveBeenCalledTimes(1));
    expect(onRenderError.mock.calls[0][0]).toContain("session not found");
    expect(container.textContent ?? "").not.toContain("Rendering page");
  });

  it("surfaces thrown exceptions via onRenderError instead of swallowing them", async () => {
    docRenderMock.mockImplementationOnce(() => {
      throw new Error("IPC bridge offline");
    });

    const onRenderError = vi.fn();

    const { container } = render(
      <PdfCanvasViewer
        sessionId="doc-session-test"
        pageIndex={0}
        zoomPercent={100}
        onRenderError={onRenderError}
      />,
    );

    await waitFor(() => expect(onRenderError).toHaveBeenCalledTimes(1));
    expect(onRenderError.mock.calls[0][0]).toMatch(/IPC bridge offline/);
    // Critical: the previous implementation left "Rendering page…" stuck on
    // any thrown exception. The new lifecycle must clear it in `.finally()`.
    expect(container.textContent ?? "").not.toContain("Rendering page");
  });

  it("surfaces zero-size renders rather than painting a blank canvas", async () => {
    docRenderMock.mockResolvedValueOnce({
      ok: true,
      data: { ...buildRenderResponse(1, 1), width_px: 0, height_px: 0 },
    });

    const onRenderError = vi.fn();

    render(
      <PdfCanvasViewer
        sessionId="doc-session-test"
        pageIndex={0}
        zoomPercent={100}
        onRenderError={onRenderError}
      />,
    );

    await waitFor(() => expect(onRenderError).toHaveBeenCalledTimes(1));
    expect(onRenderError.mock.calls[0][0]).toMatch(/zero-size/i);
  });
});
