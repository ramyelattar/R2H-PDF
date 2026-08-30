use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringUnit {
    W,
    KW,
    VA,
    // Electrical-engineering SI unit spellings (kilowatt, kilovolt-ampere);
    // the uppercase forms are the correct domain notation, not acronyms.
    #[allow(clippy::upper_case_acronyms)]
    KVA,
    A,
    V,
    Phase,
    PowerFactor,
    Quantity,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedValue {
    pub original_text: String,
    pub numeric_value: f64,
    pub original_unit: EngineeringUnit,
    pub normalized_value: f64,
    pub normalized_unit: EngineeringUnit,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitParseResult {
    pub values: Vec<NormalizedValue>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationInput {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub unit: EngineeringUnit,
    pub source_citation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationStep {
    pub step_id: String,
    pub description: String,
    pub formula: String,
    pub inputs: Vec<CalculationInput>,
    pub output_value: f64,
    pub output_unit: EngineeringUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationTrace {
    pub calculation_id: String,
    pub calculation_type: String,
    pub title: String,
    pub steps: Vec<CalculationStep>,
    pub final_value: f64,
    pub final_unit: EngineeringUnit,
    pub warnings: Vec<String>,
    pub source_citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Major,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineeringFinding {
    pub finding_id: String,
    pub finding_type: String,
    pub severity: FindingSeverity,
    pub title: String,
    pub description: String,
    pub page_refs: Vec<usize>,
    pub source_citations: Vec<String>,
    pub calculation_trace_id: Option<String>,
    pub recommendation: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadScheduleRow {
    pub row_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub source: String,
    pub equipment_tag: String,
    pub description: String,
    pub quantity: Option<f64>,
    pub power_kw: Option<f64>,
    pub apparent_power_kva: Option<f64>,
    pub current_a: Option<f64>,
    pub voltage_v: Option<f64>,
    pub phase: Option<String>,
    pub power_factor: Option<f64>,
    pub circuit_ref: Option<String>,
    pub panel_ref: Option<String>,
    pub raw_text: String,
    pub confidence: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadScheduleExtractionResult {
    pub session_id: String,
    pub rows: Vec<LoadScheduleRow>,
    pub findings: Vec<EngineeringFinding>,
    pub warnings: Vec<String>,
    pub pages_scanned: usize,
    pub extraction_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculateRequest {
    pub calculation_type: String,
    pub inputs: Vec<CalculationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SumLoadsRequest {
    pub session_id: String,
    pub unit: String,
}
