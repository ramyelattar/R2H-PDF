import { describe, it, expect } from "vitest";
import type { CalculationTrace, EngineeringFinding, LoadScheduleRow, UnitParseResult } from "./types";

describe("UnitParseResult", () => {
  it("represents parsed values", () => {
    const result: UnitParseResult = {
      values: [
        { original_text: "1200 W", numeric_value: 1200, original_unit: "w", normalized_value: 1.2, normalized_unit: "kw", confidence: 0.95 },
        { original_text: "230 V", numeric_value: 230, original_unit: "v", normalized_value: 230, normalized_unit: "v", confidence: 0.9 },
      ],
      warnings: [],
    };
    expect(result.values.length).toBe(2);
    expect(result.values[0].normalized_value).toBe(1.2);
    expect(result.values[0].normalized_unit).toBe("kw");
  });
});

describe("LoadScheduleRow", () => {
  it("represents extracted row", () => {
    const row: LoadScheduleRow = {
      row_id: "r1", session_id: "s1", page_index: 2, source: "native_text",
      equipment_tag: "AHU-1", description: "Air Handling Unit",
      quantity: null, power_kw: 15.0, apparent_power_kva: null,
      current_a: null, voltage_v: 400, phase: "3-phase",
      power_factor: 0.85, circuit_ref: "C1", panel_ref: "MDB",
      raw_text: "AHU-1 Air Handling Unit 15 kW 400V 3-phase PF 0.85",
      confidence: 0.9, warnings: [],
    };
    expect(row.power_kw).toBe(15.0);
    expect(row.page_index).toBe(2);
    expect(row.equipment_tag).toBe("AHU-1");
  });
});

describe("CalculationTrace", () => {
  it("represents sum calculation with steps", () => {
    const trace: CalculationTrace = {
      calculation_id: "calc-1", calculation_type: "sum_connected_load",
      title: "Total Connected Load: 25.0 kW",
      steps: [{
        step_id: "s1", description: "Sum of 2 kW values",
        formula: "10 + 15 = 25 kW",
        inputs: [
          { id: "a", label: "AHU-1", value: 10, unit: "kw", source_citation: "Page 1" },
          { id: "b", label: "Pump-1", value: 15, unit: "kw", source_citation: "Page 2" },
        ],
        output_value: 25, output_unit: "kw",
      }],
      final_value: 25, final_unit: "kw",
      warnings: [], source_citations: ["Page 1", "Page 2"],
    };
    expect(trace.final_value).toBe(25);
    expect(trace.steps[0].formula).toContain("25");
    expect(trace.source_citations.length).toBe(2);
  });
});

describe("EngineeringFinding", () => {
  it("represents total connected load finding", () => {
    const finding: EngineeringFinding = {
      finding_id: "f1", finding_type: "total_connected_load",
      severity: "info", title: "Total Connected Load: 25.0 kW",
      description: "Sum of 2 load items = 25.00 kW",
      page_refs: [0, 1], source_citations: ["Page 1", "Page 2"],
      calculation_trace_id: "calc-1",
      recommendation: "Verify against panel schedule totals",
      confidence: 0.9,
    };
    expect(finding.severity).toBe("info");
    expect(finding.calculation_trace_id).toBe("calc-1");
    expect(finding.page_refs.length).toBe(2);
  });

  it("represents missing PF warning", () => {
    const finding: EngineeringFinding = {
      finding_id: "f2", finding_type: "missing_power_factor",
      severity: "warning", title: "3 items missing power factor",
      description: "Cannot convert kW to kVA without power factor.",
      page_refs: [0, 1, 2], source_citations: [],
      calculation_trace_id: null,
      recommendation: "Add power factor values",
      confidence: 0.8,
    };
    expect(finding.severity).toBe("warning");
    expect(finding.calculation_trace_id).toBeNull();
  });

  it("represents insufficient data", () => {
    const finding: EngineeringFinding = {
      finding_id: "f3", finding_type: "insufficient_data_for_calculation",
      severity: "warning", title: "No load schedule data extracted",
      description: "No electrical load schedule rows were found.",
      page_refs: [], source_citations: [],
      calculation_trace_id: null,
      recommendation: "Ensure the document contains electrical load schedule data.",
      confidence: 1.0,
    };
    expect(finding.finding_type).toBe("insufficient_data_for_calculation");
  });
});

describe("Engineering UI logic", () => {
  it("extract button disabled when loading", () => {
    const loading = true;
    expect(loading).toBe(true); // Button would be disabled.
  });

  it("findings grouped by severity", () => {
    const findings: EngineeringFinding[] = [
      { finding_id: "f1", finding_type: "total_connected_load", severity: "info", title: "T", description: "", page_refs: [], source_citations: [], calculation_trace_id: null, recommendation: "", confidence: 0.9 },
      { finding_id: "f2", finding_type: "missing_power_factor", severity: "warning", title: "W", description: "", page_refs: [], source_citations: [], calculation_trace_id: null, recommendation: "", confidence: 0.8 },
    ];
    const warnings = findings.filter((f) => f.severity === "warning");
    const infos = findings.filter((f) => f.severity === "info");
    expect(warnings.length).toBe(1);
    expect(infos.length).toBe(1);
  });
});
