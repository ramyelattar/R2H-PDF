//! Deterministic engineering calculator with full calculation traces.

use super::types::*;

/// Sum connected load (kW values).
pub fn sum_connected_load(inputs: &[CalculationInput]) -> CalculationTrace {
    let kw_inputs: Vec<&CalculationInput> = inputs.iter()
        .filter(|i| matches!(i.unit, EngineeringUnit::KW))
        .collect();

    let total: f64 = kw_inputs.iter().map(|i| i.value).sum();
    let citations: Vec<String> = kw_inputs.iter().filter_map(|i| i.source_citation.clone()).collect();

    CalculationTrace {
        calculation_id: format!("calc-sum-kw-{}", epoch_ms()),
        calculation_type: "sum_connected_load".to_string(),
        title: "Total Connected Load (kW)".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".to_string(),
            description: format!("Sum of {} kW values", kw_inputs.len()),
            formula: format!("{} = {} kW", kw_inputs.iter().map(|i| format!("{}", i.value)).collect::<Vec<_>>().join(" + "), total),
            inputs: kw_inputs.iter().map(|i| (*i).clone()).collect(),
            output_value: total,
            output_unit: EngineeringUnit::KW,
        }],
        final_value: total,
        final_unit: EngineeringUnit::KW,
        warnings: if kw_inputs.is_empty() { vec!["No kW values provided".to_string()] } else { vec![] },
        source_citations: citations,
    }
}

/// Sum apparent load (kVA values).
pub fn sum_apparent_load(inputs: &[CalculationInput]) -> CalculationTrace {
    let kva_inputs: Vec<&CalculationInput> = inputs.iter()
        .filter(|i| matches!(i.unit, EngineeringUnit::KVA))
        .collect();

    let total: f64 = kva_inputs.iter().map(|i| i.value).sum();
    let citations: Vec<String> = kva_inputs.iter().filter_map(|i| i.source_citation.clone()).collect();

    CalculationTrace {
        calculation_id: format!("calc-sum-kva-{}", epoch_ms()),
        calculation_type: "sum_apparent_load".to_string(),
        title: "Total Apparent Load (kVA)".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".to_string(),
            description: format!("Sum of {} kVA values", kva_inputs.len()),
            formula: format!("{} = {} kVA", kva_inputs.iter().map(|i| format!("{}", i.value)).collect::<Vec<_>>().join(" + "), total),
            inputs: kva_inputs.iter().map(|i| (*i).clone()).collect(),
            output_value: total,
            output_unit: EngineeringUnit::KVA,
        }],
        final_value: total,
        final_unit: EngineeringUnit::KVA,
        warnings: if kva_inputs.is_empty() { vec!["No kVA values provided".to_string()] } else { vec![] },
        source_citations: citations,
    }
}

/// Convert kW to kVA: kVA = kW / PF
pub fn kw_to_kva(kw: f64, pf: Option<f64>) -> CalculationTrace {
    let mut warnings = Vec::new();
    let (result, formula) = match pf {
        Some(pf_val) if pf_val > 0.0 && pf_val <= 1.0 => {
            let kva = kw / pf_val;
            (kva, format!("{} kW / {} = {:.2} kVA", kw, pf_val, kva))
        }
        _ => {
            warnings.push("Power factor is missing or invalid. Cannot convert kW to kVA without PF.".to_string());
            (0.0, "kW / PF = kVA (PF missing)".to_string())
        }
    };

    CalculationTrace {
        calculation_id: format!("calc-kw-kva-{}", epoch_ms()),
        calculation_type: "kw_to_kva".to_string(),
        title: "Convert kW to kVA".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".to_string(), description: "kVA = kW / PF".to_string(),
            formula, inputs: vec![
                CalculationInput { id: "kw".into(), label: "Power".into(), value: kw, unit: EngineeringUnit::KW, source_citation: None },
            ],
            output_value: result, output_unit: EngineeringUnit::KVA,
        }],
        final_value: result, final_unit: EngineeringUnit::KVA, warnings, source_citations: vec![],
    }
}

/// Convert kVA to kW: kW = kVA × PF
pub fn kva_to_kw(kva: f64, pf: Option<f64>) -> CalculationTrace {
    let mut warnings = Vec::new();
    let (result, formula) = match pf {
        Some(pf_val) if pf_val > 0.0 && pf_val <= 1.0 => {
            let kw = kva * pf_val;
            (kw, format!("{} kVA × {} = {:.2} kW", kva, pf_val, kw))
        }
        _ => {
            warnings.push("Power factor is missing or invalid. Cannot convert kVA to kW without PF.".to_string());
            (0.0, "kVA × PF = kW (PF missing)".to_string())
        }
    };

    CalculationTrace {
        calculation_id: format!("calc-kva-kw-{}", epoch_ms()),
        calculation_type: "kva_to_kw".to_string(),
        title: "Convert kVA to kW".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".into(), description: "kW = kVA × PF".into(), formula,
            inputs: vec![CalculationInput { id: "kva".into(), label: "Apparent Power".into(), value: kva, unit: EngineeringUnit::KVA, source_citation: None }],
            output_value: result, output_unit: EngineeringUnit::KW,
        }],
        final_value: result, final_unit: EngineeringUnit::KW, warnings, source_citations: vec![],
    }
}

/// Three-phase apparent power: kVA = √3 × V × A / 1000
pub fn three_phase_kva(voltage: f64, current: f64) -> CalculationTrace {
    let sqrt3 = 3.0_f64.sqrt();
    let kva = sqrt3 * voltage * current / 1000.0;
    CalculationTrace {
        calculation_id: format!("calc-3ph-{}", epoch_ms()),
        calculation_type: "three_phase_kva".to_string(),
        title: "Three-Phase Apparent Power".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".into(), description: "kVA = √3 × V × A / 1000".into(),
            formula: format!("√3 × {} V × {} A / 1000 = {:.2} kVA", voltage, current, kva),
            inputs: vec![
                CalculationInput { id: "v".into(), label: "Voltage".into(), value: voltage, unit: EngineeringUnit::V, source_citation: None },
                CalculationInput { id: "a".into(), label: "Current".into(), value: current, unit: EngineeringUnit::A, source_citation: None },
            ],
            output_value: kva, output_unit: EngineeringUnit::KVA,
        }],
        final_value: kva, final_unit: EngineeringUnit::KVA, warnings: vec![], source_citations: vec![],
    }
}

/// Single-phase apparent power: kVA = V × A / 1000
pub fn single_phase_kva(voltage: f64, current: f64) -> CalculationTrace {
    let kva = voltage * current / 1000.0;
    CalculationTrace {
        calculation_id: format!("calc-1ph-{}", epoch_ms()),
        calculation_type: "single_phase_kva".to_string(),
        title: "Single-Phase Apparent Power".to_string(),
        steps: vec![CalculationStep {
            step_id: "s1".into(), description: "kVA = V × A / 1000".into(),
            formula: format!("{} V × {} A / 1000 = {:.2} kVA", voltage, current, kva),
            inputs: vec![
                CalculationInput { id: "v".into(), label: "Voltage".into(), value: voltage, unit: EngineeringUnit::V, source_citation: None },
                CalculationInput { id: "a".into(), label: "Current".into(), value: current, unit: EngineeringUnit::A, source_citation: None },
            ],
            output_value: kva, output_unit: EngineeringUnit::KVA,
        }],
        final_value: kva, final_unit: EngineeringUnit::KVA, warnings: vec![], source_citations: vec![],
    }
}

fn epoch_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inp(id: &str, val: f64, unit: EngineeringUnit) -> CalculationInput {
        CalculationInput { id: id.into(), label: id.into(), value: val, unit, source_citation: Some(format!("page-{}", id)) }
    }

    #[test] fn sum_kw() {
        let t = sum_connected_load(&[inp("a", 1.5, EngineeringUnit::KW), inp("b", 2.5, EngineeringUnit::KW)]);
        assert!((t.final_value - 4.0).abs() < 0.001);
        assert_eq!(t.source_citations.len(), 2);
    }
    #[test] fn sum_kva() {
        let t = sum_apparent_load(&[inp("a", 3.0, EngineeringUnit::KVA), inp("b", 7.0, EngineeringUnit::KVA)]);
        assert!((t.final_value - 10.0).abs() < 0.001);
    }
    #[test] fn kw_to_kva_with_pf() {
        let t = kw_to_kva(10.0, Some(0.8));
        assert!((t.final_value - 12.5).abs() < 0.001);
        assert!(t.warnings.is_empty());
    }
    #[test] fn kw_to_kva_missing_pf() {
        let t = kw_to_kva(10.0, None);
        assert!(!t.warnings.is_empty());
        assert!(t.warnings[0].contains("missing"));
    }
    #[test] fn kva_to_kw_with_pf() {
        let t = kva_to_kw(12.5, Some(0.8));
        assert!((t.final_value - 10.0).abs() < 0.001);
    }
    #[test] fn three_phase() {
        let t = three_phase_kva(400.0, 100.0);
        // √3 × 400 × 100 / 1000 = 69.28 kVA
        assert!((t.final_value - 69.28).abs() < 0.1);
        assert!(t.steps[0].formula.contains("√3"));
    }
    #[test] fn single_phase() {
        let t = single_phase_kva(230.0, 16.0);
        // 230 × 16 / 1000 = 3.68 kVA
        assert!((t.final_value - 3.68).abs() < 0.001);
    }
    #[test] fn trace_includes_inputs() {
        let t = sum_connected_load(&[inp("x", 5.0, EngineeringUnit::KW)]);
        assert!(!t.steps.is_empty());
        assert!(!t.steps[0].inputs.is_empty());
    }
}
