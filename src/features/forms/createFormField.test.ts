import { describe, it, expect, vi, beforeEach } from "vitest";

// Tests for the frontend wrapper of the Phase 23A form-field creation IPC.
// We mock the Tauri `invoke` bridge and exercise the type contract.

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  pdfCreateFormField,
  pdfDeleteFormField,
  pdfUpdateFormFieldProperties,
  type CreateFormFieldRequest,
  type FormFieldOperationResult,
} from "../../lib/ipc";

describe("pdfCreateFormField IPC wrapper", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("forwards the request payload under `request` and returns success", async () => {
    const expected: FormFieldOperationResult = {
      success: true,
      field_name: "Greeting",
      page_index: 0,
      action: "create",
      warnings: [],
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const req: CreateFormFieldRequest = {
      session_id: "doc-session-1",
      page_index: 0,
      field_type: "text",
      name: "Greeting",
      value: "Hello",
      rect: [50, 50, 250, 80],
      required: true,
      font_size: 14,
    };

    const result = await pdfCreateFormField(req);

    expect(invoke).toHaveBeenCalledWith("pdf_create_form_field", { request: req });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.field_name).toBe("Greeting");
      expect(result.data.action).toBe("create");
    }
  });

  it("propagates auto-suffix warning to the caller", async () => {
    const expected: FormFieldOperationResult = {
      success: true,
      field_name: "Greeting_2",
      page_index: 0,
      action: "create",
      warnings: ["duplicate field name 'Greeting' was auto-suffixed to 'Greeting_2'"],
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const result = await pdfCreateFormField({
      session_id: "s",
      page_index: 0,
      field_type: "text",
      name: "Greeting",
      rect: [0, 0, 100, 30],
    });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.field_name).toBe("Greeting_2");
      expect(result.data.warnings[0]).toContain("auto-suffixed");
    }
  });

  it("returns an error envelope when the backend rejects an invalid rect", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("invalid PDF: rect must satisfy x1>x0 and y1>y0");

    const result = await pdfCreateFormField({
      session_id: "s",
      page_index: 0,
      field_type: "text",
      name: "Bad",
      rect: [100, 100, 50, 50],
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.message.toLowerCase()).toContain("rect");
    }
  });
});

describe("pdfDeleteFormField IPC wrapper", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("invokes pdf_delete_form_field with the request payload", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      success: true, field_name: "Greeting", page_index: 0, action: "delete", warnings: [],
    } satisfies FormFieldOperationResult);

    const result = await pdfDeleteFormField({ session_id: "s", field_name: "Greeting" });

    expect(invoke).toHaveBeenCalledWith("pdf_delete_form_field", {
      request: { session_id: "s", field_name: "Greeting" },
    });
    expect(result.ok).toBe(true);
  });
});

describe("pdfUpdateFormFieldProperties IPC wrapper", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("supports toggling required/read_only/value/font_size", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      success: true, field_name: "Foo", page_index: 0, action: "update_properties", warnings: [],
    } satisfies FormFieldOperationResult);

    const result = await pdfUpdateFormFieldProperties({
      session_id: "s",
      field_name: "Foo",
      value: "new-value",
      required: true,
      read_only: false,
      font_size: 16,
    });

    expect(invoke).toHaveBeenCalledWith("pdf_update_form_field_properties", {
      request: {
        session_id: "s",
        field_name: "Foo",
        value: "new-value",
        required: true,
        read_only: false,
        font_size: 16,
      },
    });
    expect(result.ok).toBe(true);
  });
});
