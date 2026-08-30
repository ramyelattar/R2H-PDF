export type EngineeringUnit = "w" | "kw" | "va" | "kva" | "a" | "v" | "phase" | "power_factor" | "quantity" | "unknown";
export type FindingSeverity = "info" | "warning" | "major" | "critical";

export interface NormalizedValue {
  original_text: string;
  numeric_value: number;
  original_unit: EngineeringUnit;
  normalized_value: number;
  normalized_unit: EngineeringUnit;
  confidence: number;
}

export interface UnitParseResult {
  values: NormalizedValue[];
  warnings: string[];
}

export interface CalculationInput {
  id: string;
  label: string;
  value: number;
  unit: EngineeringUnit;
  source_citation: string | null;
}

export interface CalculationStep {
  step_id: string;
  description: string;
  formula: string;
  inputs: CalculationInput[];
  output_value: number;
  output_unit: EngineeringUnit;
}

export interface CalculationTrace {
  calculation_id: string;
  calculation_type: string;
  title: string;
  steps: CalculationStep[];
  final_value: number;
  final_unit: EngineeringUnit;
  warnings: string[];
  source_citations: string[];
}

export interface EngineeringFinding {
  finding_id: string;
  finding_type: string;
  severity: FindingSeverity;
  title: string;
  description: string;
  page_refs: number[];
  source_citations: string[];
  calculation_trace_id: string | null;
  recommendation: string;
  confidence: number;
}

export interface LoadScheduleRow {
  row_id: string;
  session_id: string;
  page_index: number;
  source: string;
  equipment_tag: string;
  description: string;
  quantity: number | null;
  power_kw: number | null;
  apparent_power_kva: number | null;
  current_a: number | null;
  voltage_v: number | null;
  phase: string | null;
  power_factor: number | null;
  circuit_ref: string | null;
  panel_ref: string | null;
  raw_text: string;
  confidence: number;
  warnings: string[];
}

export interface LoadScheduleExtractionResult {
  session_id: string;
  rows: LoadScheduleRow[];
  findings: EngineeringFinding[];
  warnings: string[];
  pages_scanned: number;
  extraction_mode: string;
}
