#![allow(dead_code)]

pub mod calculator;
pub mod extraction;
pub mod findings;
pub mod types;
pub mod units;

use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::State;

use types::*;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct EngineeringState {
    pub rows: Mutex<Vec<LoadScheduleRow>>,
    pub findings: Mutex<Vec<EngineeringFinding>>,
    pub traces: Mutex<Vec<CalculationTrace>>,
}

impl EngineeringState {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(Vec::new()),
            findings: Mutex::new(Vec::new()),
            traces: Mutex::new(Vec::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn engineering_parse_units(text: String) -> UnitParseResult {
    units::parse_units(&text)
}

#[tauri::command]
pub fn engineering_calculate(request: CalculateRequest) -> Result<CalculationTrace, String> {
    match request.calculation_type.as_str() {
        "sum_connected_load" => Ok(calculator::sum_connected_load(&request.inputs)),
        "sum_apparent_load" => Ok(calculator::sum_apparent_load(&request.inputs)),
        "kw_to_kva" => {
            let kw = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::KW)).map(|i| i.value).unwrap_or(0.0);
            let pf = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::PowerFactor)).map(|i| i.value);
            Ok(calculator::kw_to_kva(kw, pf))
        }
        "kva_to_kw" => {
            let kva = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::KVA)).map(|i| i.value).unwrap_or(0.0);
            let pf = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::PowerFactor)).map(|i| i.value);
            Ok(calculator::kva_to_kw(kva, pf))
        }
        "three_phase_kva" => {
            let v = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::V)).map(|i| i.value).unwrap_or(0.0);
            let a = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::A)).map(|i| i.value).unwrap_or(0.0);
            Ok(calculator::three_phase_kva(v, a))
        }
        "single_phase_kva" => {
            let v = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::V)).map(|i| i.value).unwrap_or(0.0);
            let a = request.inputs.iter().find(|i| matches!(i.unit, EngineeringUnit::A)).map(|i| i.value).unwrap_or(0.0);
            Ok(calculator::single_phase_kva(v, a))
        }
        other => Err(format!("Unknown calculation type: {other}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractLoadScheduleRequest {
    pub session_id: String,
    pub page_texts: Vec<(usize, String, String)>,
    pub include_ocr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEngineeringText {
    pub page_index: usize,
    pub native_text: String,
    pub ocr_text: String,
    pub source: String,
}

#[tauri::command]
pub fn engineering_get_page_texts(
    doc_state: State<'_, crate::document_core::DocumentCoreState>,
    ocr_state: State<'_, crate::document_core::ocr::OcrState>,
    session_id: String,
    include_ocr: bool,
) -> Result<Vec<PageEngineeringText>, String> {
    // Get native text for all pages.
    let native_texts = doc_state.store.extract_all_pages_text(session_id.clone())
        .map_err(|e| e.to_string())?;

    // Get OCR texts if requested.
    let ocr_texts: Vec<(usize, String)> = if include_ocr {
        let engine = ocr_state.engine.lock().map_err(|_| "OCR lock poisoned".to_string())?;
        engine.cache.all_texts_for_session(&session_id)
    } else {
        Vec::new()
    };

    let ocr_map: std::collections::HashMap<usize, String> = ocr_texts.into_iter().collect();

    let mut results = Vec::new();
    for (page_index, native) in native_texts.iter().enumerate() {
        let ocr = ocr_map.get(&page_index).cloned().unwrap_or_default();
        let source = if !native.trim().is_empty() && !ocr.is_empty() {
            "mixed"
        } else if !native.trim().is_empty() {
            "native_text"
        } else if !ocr.is_empty() {
            "ocr_text"
        } else {
            "empty"
        };

        results.push(PageEngineeringText {
            page_index,
            native_text: native.clone(),
            ocr_text: ocr,
            source: source.to_string(),
        });
    }

    Ok(results)
}

#[tauri::command]
pub fn engineering_extract_load_schedule(
    state: State<'_, EngineeringState>,
    request: ExtractLoadScheduleRequest,
) -> LoadScheduleExtractionResult {
    let result = extraction::extract_load_schedule(&request.session_id, &request.page_texts);
    let mut rows = state.rows.lock().unwrap();
    *rows = result.rows.clone();
    result
}

#[tauri::command]
pub fn engineering_get_load_rows(state: State<'_, EngineeringState>) -> Vec<LoadScheduleRow> {
    state.rows.lock().unwrap().clone()
}

#[tauri::command]
pub fn engineering_clear_load_rows(state: State<'_, EngineeringState>) {
    state.rows.lock().unwrap().clear();
    state.findings.lock().unwrap().clear();
    state.traces.lock().unwrap().clear();
}

#[tauri::command]
pub fn engineering_generate_findings(state: State<'_, EngineeringState>) -> Vec<EngineeringFinding> {
    let rows = state.rows.lock().unwrap();
    let (new_findings, new_traces) = findings::generate_findings(&rows);
    let mut findings_store = state.findings.lock().unwrap();
    *findings_store = new_findings.clone();
    let mut traces_store = state.traces.lock().unwrap();
    *traces_store = new_traces;
    new_findings
}

#[tauri::command]
pub fn engineering_get_findings(state: State<'_, EngineeringState>) -> Vec<EngineeringFinding> {
    state.findings.lock().unwrap().clone()
}

#[tauri::command]
pub fn engineering_get_calculation_trace(state: State<'_, EngineeringState>, calculation_id: String) -> Option<CalculationTrace> {
    state.traces.lock().unwrap().iter().find(|t| t.calculation_id == calculation_id).cloned()
}
