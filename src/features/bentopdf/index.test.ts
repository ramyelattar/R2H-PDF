import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getBentoPdfStatus, openBentoPdfTools } from "./index";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe("BentoPDF Tauri IPC", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  it("opens the dedicated BentoPDF window through the registered command", async () => {
    mockedInvoke.mockResolvedValue({
      reusedExistingWindow: false,
      windowLabel: "bentopdf-tools",
      url: "http://127.0.0.1:41000/bentopdf/index.html",
      bundlePath: "generated/bentopdf-offline/2.8.6-21c924a3/bentopdf",
      bundleSource: "working-directory-development",
    });

    const result = await openBentoPdfTools();

    expect(mockedInvoke).toHaveBeenCalledWith("bentopdf_open");
    expect(result.windowLabel).toBe("bentopdf-tools");
  });

  it("reads BentoPDF installation and server status", async () => {
    mockedInvoke.mockResolvedValue({
      installed: true,
      running: false,
      bundlePath: "generated/bentopdf-offline/2.8.6-21c924a3/bentopdf",
      bundleSource: "working-directory-development",
      origin: null,
      checkedPaths: [],
      validationError: null,
    });

    const result = await getBentoPdfStatus();

    expect(mockedInvoke).toHaveBeenCalledWith("bentopdf_get_status");
    expect(result.installed).toBe(true);
  });
});
