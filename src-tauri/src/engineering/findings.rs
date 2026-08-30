//! Engineering findings generation from extracted load schedule data.

use super::calculator;
use super::types::*;

/// Generate engineering findings from extracted load schedule rows.
pub fn generate_findings(rows: &[LoadScheduleRow]) -> (Vec<EngineeringFinding>, Vec<CalculationTrace>) {
    let mut findings = Vec::new();
    let mut traces = Vec::new();

    // 1. Total connected load (kW).
    let kw_inputs: Vec<CalculationInput> = rows.iter()
        .filter_map(|r| r.power_kw.map(|kw| CalculationInput {
            id: r.row_id.clone(), label: r.equipment_tag.clone(),
            value: kw, unit: EngineeringUnit::KW,
            source_citation: Some(format!("Page {}", r.page_index + 1)),
        }))
        .collect();

    if !kw_inputs.is_empty() {
        let trace = calculator::sum_connected_load(&kw_inputs);
        findings.push(EngineeringFinding {
            finding_id: format!("finding-total-kw"), finding_type: "total_connected_load".into(),
            severity: FindingSeverity::Info,
            title: format!("Total Connected Load: {:.1} kW", trace.final_value),
            description: format!("Sum of {} load items = {:.2} kW", kw_inputs.len(), trace.final_value),
            page_refs: rows.iter().map(|r| r.page_index).collect(),
            source_citations: trace.source_citations.clone(),
            calculation_trace_id: Some(trace.calculation_id.clone()),
            recommendation: "Verify against panel schedule totals".into(), confidence: 0.9,
        });
        traces.push(trace);
    }

    // 2. Total apparent load (kVA).
    let kva_inputs: Vec<CalculationInput> = rows.iter()
        .filter_map(|r| r.apparent_power_kva.map(|kva| CalculationInput {
            id: r.row_id.clone(), label: r.equipment_tag.clone(),
            value: kva, unit: EngineeringUnit::KVA,
            source_citation: Some(format!("Page {}", r.page_index + 1)),
        }))
        .collect();

    if !kva_inputs.is_empty() {
        let trace = calculator::sum_apparent_load(&kva_inputs);
        findings.push(EngineeringFinding {
            finding_id: format!("finding-total-kva"), finding_type: "total_apparent_load".into(),
            severity: FindingSeverity::Info,
            title: format!("Total Apparent Load: {:.1} kVA", trace.final_value),
            description: format!("Sum of {} apparent load items = {:.2} kVA", kva_inputs.len(), trace.final_value),
            page_refs: rows.iter().map(|r| r.page_index).collect(),
            source_citations: trace.source_citations.clone(),
            calculation_trace_id: Some(trace.calculation_id.clone()),
            recommendation: "Verify against transformer/generator capacity".into(), confidence: 0.9,
        });
        traces.push(trace);
    }

    // 3. Missing power factor warnings.
    let rows_needing_pf: Vec<&LoadScheduleRow> = rows.iter()
        .filter(|r| r.power_kw.is_some() && r.apparent_power_kva.is_none() && r.power_factor.is_none())
        .collect();
    if !rows_needing_pf.is_empty() {
        findings.push(EngineeringFinding {
            finding_id: "finding-missing-pf".into(), finding_type: "missing_power_factor".into(),
            severity: FindingSeverity::Warning,
            title: format!("{} items missing power factor", rows_needing_pf.len()),
            description: "Cannot convert kW to kVA without power factor for these items.".into(),
            page_refs: rows_needing_pf.iter().map(|r| r.page_index).collect(),
            source_citations: vec![], calculation_trace_id: None,
            recommendation: "Add power factor values or assume 0.85 for general loads".into(), confidence: 0.8,
        });
    }

    // 4. Low confidence OCR rows.
    let low_conf: Vec<&LoadScheduleRow> = rows.iter().filter(|r| r.confidence < 0.75).collect();
    if !low_conf.is_empty() {
        findings.push(EngineeringFinding {
            finding_id: "finding-low-conf".into(), finding_type: "low_confidence_extraction".into(),
            severity: FindingSeverity::Warning,
            title: format!("{} rows with low extraction confidence", low_conf.len()),
            description: "These rows were extracted with low confidence and should be manually verified.".into(),
            page_refs: low_conf.iter().map(|r| r.page_index).collect(),
            source_citations: vec![], calculation_trace_id: None,
            recommendation: "Manually verify extracted values against source document".into(), confidence: 0.7,
        });
    }

    // 5. Insufficient data.
    if rows.is_empty() {
        findings.push(EngineeringFinding {
            finding_id: "finding-no-data".into(), finding_type: "insufficient_data_for_calculation".into(),
            severity: FindingSeverity::Warning,
            title: "No load schedule data extracted".into(),
            description: "No electrical load schedule rows were found. Calculations cannot be performed.".into(),
            page_refs: vec![], source_citations: vec![], calculation_trace_id: None,
            recommendation: "Ensure the document contains electrical load schedule data and rebuild the index.".into(),
            confidence: 1.0,
        });
    }

    (findings, traces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(kw: Option<f64>, kva: Option<f64>, pf: Option<f64>, page: usize) -> LoadScheduleRow {
        LoadScheduleRow {
            row_id: format!("r-{}", page), session_id: "s1".into(), page_index: page,
            source: "native_text".into(), equipment_tag: format!("EQ-{}", page),
            description: "Test".into(), quantity: None, power_kw: kw,
            apparent_power_kva: kva, current_a: None, voltage_v: None,
            phase: None, power_factor: pf, circuit_ref: None, panel_ref: None,
            raw_text: "test".into(), confidence: 0.9, warnings: vec![],
        }
    }

    #[test] fn total_connected_load_finding() {
        let rows = vec![make_row(Some(10.0), None, None, 0), make_row(Some(5.0), None, None, 1)];
        let (findings, traces) = generate_findings(&rows);
        let f = findings.iter().find(|f| f.finding_type == "total_connected_load").unwrap();
        assert!(f.title.contains("15.0 kW"));
        assert!(f.calculation_trace_id.is_some());
        assert!(!traces.is_empty());
    }

    #[test] fn total_apparent_load_finding() {
        let rows = vec![make_row(None, Some(12.5), None, 0)];
        let (findings, _) = generate_findings(&rows);
        assert!(findings.iter().any(|f| f.finding_type == "total_apparent_load"));
    }

    #[test] fn missing_pf_finding() {
        let rows = vec![make_row(Some(10.0), None, None, 0)];
        let (findings, _) = generate_findings(&rows);
        assert!(findings.iter().any(|f| f.finding_type == "missing_power_factor"));
    }

    #[test] fn insufficient_data_finding() {
        let (findings, _) = generate_findings(&[]);
        assert!(findings.iter().any(|f| f.finding_type == "insufficient_data_for_calculation"));
    }

    #[test] fn low_confidence_finding() {
        let mut row = make_row(Some(5.0), None, None, 0);
        row.confidence = 0.5;
        let (findings, _) = generate_findings(&[row]);
        assert!(findings.iter().any(|f| f.finding_type == "low_confidence_extraction"));
    }

    #[test] fn findings_include_citations() {
        let rows = vec![make_row(Some(10.0), None, None, 3)];
        let (findings, _) = generate_findings(&rows);
        let f = findings.iter().find(|f| f.finding_type == "total_connected_load").unwrap();
        assert!(f.page_refs.contains(&3));
    }
}
