import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("./useEngineering", () => ({
  useEngineering: () => ({
    loadRows: [],
    findings: [
      {
        finding_id: "f1",
        finding_type: "missing_power_factor",
        severity: "warning",
        title: "Missing power factor",
        description: "Power factor is not shown.",
        page_refs: [0],
        source_citations: [],
        calculation_trace_id: "calc-1",
        recommendation: "Confirm the value.",
        confidence: 0.8,
      },
    ],
    traces: [
      {
        calculation_id: "calc-1",
        calculation_type: "sum_connected_load",
        title: "Connected load",
        steps: [
          { step_id: "s1", description: "Sum loads", formula: "10 + 5", inputs: [], output_value: 15, output_unit: "kw" },
        ],
        final_value: 15,
        final_unit: "kw",
        warnings: [],
        source_citations: [],
      },
    ],
    pageTexts: [],
    loading: false,
    error: null,
    parseUnits: vi.fn(),
    extractLoadSchedule: vi.fn(),
    generateFindings: vi.fn(),
    getTrace: vi.fn(),
    clearState: vi.fn(),
  }),
}));

import { EngineeringPanel } from "./EngineeringPanel";

describe("Pass 2F — EngineeringPanel polish", () => {
  it("renders empty workflow guidance, severity findings, and readable traces", () => {
    render(<EngineeringPanel sessionId="doc-session-test" />);

    expect(screen.getByTestId("engineering-empty-state").textContent).toMatch(/Extract a load schedule first/);
    expect(screen.getByTestId("engineering-finding-warning").textContent).toMatch(/Missing power factor/);
    expect(screen.getByTestId("engineering-traces").textContent).toMatch(/10 \+ 5/);
  });
});
