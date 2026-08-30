//! Load schedule extraction from document text using regex patterns.

use regex::Regex;
use super::types::*;
use super::units::parse_units;

/// Extract load schedule rows from page texts using regex patterns.
pub fn extract_load_schedule(
    session_id: &str,
    page_texts: &[(usize, String, String)], // (page_index, text, source)
) -> LoadScheduleExtractionResult {
    let mut rows = Vec::new();
    let mut warnings = Vec::new();
    let mut pages_scanned = 0;

    let electrical_pattern = Regex::new(r"(?i)(kw|kva|va|amp|panel|circuit|load|mdb|smdb|mcc|db\b)").unwrap();

    for (page_index, text, source) in page_texts {
        pages_scanned += 1;
        if !electrical_pattern.is_match(text) { continue; }

        // Split into lines and look for rows with electrical values.
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.len() < 5 { continue; }
            if !electrical_pattern.is_match(trimmed) { continue; }

            let parsed = parse_units(trimmed);
            if parsed.values.is_empty() { continue; }

            let mut row = LoadScheduleRow {
                row_id: format!("row-{}-{}-{}", session_id, page_index, rows.len()),
                session_id: session_id.to_string(),
                page_index: *page_index,
                source: source.clone(),
                equipment_tag: extract_equipment_tag(trimmed),
                description: extract_description(trimmed),
                quantity: None, power_kw: None, apparent_power_kva: None,
                current_a: None, voltage_v: None, phase: None,
                power_factor: None, circuit_ref: extract_circuit_ref(trimmed),
                panel_ref: extract_panel_ref(trimmed),
                raw_text: trimmed.to_string(),
                confidence: if source == "ocr_text" { 0.7 } else { 0.85 },
                warnings: vec![],
            };

            for val in &parsed.values {
                match val.normalized_unit {
                    EngineeringUnit::KW => row.power_kw = Some(val.normalized_value),
                    EngineeringUnit::KVA => row.apparent_power_kva = Some(val.normalized_value),
                    EngineeringUnit::A => row.current_a = Some(val.normalized_value),
                    EngineeringUnit::V => row.voltage_v = Some(val.normalized_value),
                    EngineeringUnit::PowerFactor => row.power_factor = Some(val.normalized_value),
                    _ => {}
                }
            }

            // Extract phase.
            if let Some(ph) = extract_phase(trimmed) { row.phase = Some(ph); }

            // Only keep rows that have at least one power/current value.
            if row.power_kw.is_some() || row.apparent_power_kva.is_some() || row.current_a.is_some() {
                rows.push(row);
            }
        }
    }

    if rows.is_empty() {
        warnings.push("No electrical load schedule was detected in the indexed document text.".to_string());
    }

    let findings = generate_extraction_findings(&rows);

    LoadScheduleExtractionResult {
        session_id: session_id.to_string(),
        rows, findings, warnings, pages_scanned,
        extraction_mode: "regex".to_string(),
    }
}

fn extract_equipment_tag(line: &str) -> String {
    let re = Regex::new(r"(?i)\b([A-Z]{2,5}[-/]?\d{1,5}[A-Z]?)\b").unwrap();
    re.find(line).map(|m| m.as_str().to_string()).unwrap_or_default()
}

fn extract_description(line: &str) -> String {
    // Take first 60 chars as description, removing numbers-heavy parts.
    line.chars().take(60).collect::<String>().trim().to_string()
}

fn extract_circuit_ref(line: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(?:circuit|cct|ckt)\s*[-:]?\s*([A-Z0-9/-]+)").unwrap();
    re.captures(line).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

fn extract_panel_ref(line: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(?:panel|db|mdb|smdb|mcc)\s*[-:]?\s*([A-Z0-9/-]*)").unwrap();
    re.captures(line).and_then(|c| c.get(1)).map(|m| {
        let s = m.as_str().to_string();
        if s.is_empty() { line.split_whitespace().find(|w| Regex::new(r"(?i)^(mdb|smdb|db|mcc)").unwrap().is_match(w)).unwrap_or("").to_string() } else { s }
    })
}

fn extract_phase(line: &str) -> Option<String> {
    let re = Regex::new(r"(?i)\b(3[- ]?ph(?:ase)?|single[- ]?ph(?:ase)?|1[- ]?ph(?:ase)?|3P|1P)\b").unwrap();
    re.find(line).map(|m| m.as_str().to_string())
}

fn generate_extraction_findings(rows: &[LoadScheduleRow]) -> Vec<EngineeringFinding> {
    let mut findings = Vec::new();
    let mut tag_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut circuit_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for row in rows {
        if !row.equipment_tag.is_empty() {
            *tag_counts.entry(row.equipment_tag.as_str()).or_insert(0) += 1;
        }
        if let Some(ref cref) = row.circuit_ref {
            *circuit_counts.entry(cref.as_str()).or_insert(0) += 1;
        }
    }

    for (tag, count) in &tag_counts {
        if *count > 1 {
            findings.push(EngineeringFinding {
                finding_id: format!("dup-tag-{}", tag), finding_type: "duplicate_equipment_tag".into(),
                severity: if *count > 3 { FindingSeverity::Major } else { FindingSeverity::Warning },
                title: format!("Duplicate equipment tag: {}", tag),
                description: format!("Equipment tag '{}' appears {} times", tag, count),
                page_refs: vec![], source_citations: vec![], calculation_trace_id: None,
                recommendation: "Verify equipment tags are unique".into(), confidence: 0.9,
            });
        }
    }

    for (cref, count) in &circuit_counts {
        if *count > 1 {
            findings.push(EngineeringFinding {
                finding_id: format!("dup-cct-{}", cref), finding_type: "duplicate_circuit_ref".into(),
                severity: FindingSeverity::Warning,
                title: format!("Duplicate circuit reference: {}", cref),
                description: format!("Circuit '{}' appears {} times", cref, count),
                page_refs: vec![], source_citations: vec![], calculation_trace_id: None,
                recommendation: "Verify circuit references are unique".into(), confidence: 0.85,
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn extract_simple_row() {
        let pages = vec![(0, "AHU-1 Air Handling Unit 15 kW 230V 3-phase".to_string(), "native_text".to_string())];
        let result = extract_load_schedule("s1", &pages);
        assert!(!result.rows.is_empty());
        assert_eq!(result.rows[0].power_kw, Some(15.0));
        assert_eq!(result.rows[0].voltage_v, Some(230.0));
    }

    #[test] fn extract_multiple_rows() {
        let text = "AHU-1 15 kW\nPump-1 7.5 kW\nLighting 2.4 kW";
        let pages = vec![(0, text.to_string(), "native_text".to_string())];
        let result = extract_load_schedule("s1", &pages);
        assert_eq!(result.rows.len(), 3);
    }

    #[test] fn preserves_page_index() {
        let pages = vec![
            (2, "Motor 5 kW".to_string(), "native_text".to_string()),
            (5, "Pump 10 kW".to_string(), "native_text".to_string()),
        ];
        let result = extract_load_schedule("s1", &pages);
        assert_eq!(result.rows[0].page_index, 2);
        assert_eq!(result.rows[1].page_index, 5);
    }

    #[test] fn no_schedule_returns_warning() {
        let pages = vec![(0, "This is a legal document with no electrical content.".to_string(), "native_text".to_string())];
        let result = extract_load_schedule("s1", &pages);
        assert!(result.rows.is_empty());
        assert!(result.warnings.iter().any(|w| w.contains("No electrical load schedule")));
    }

    #[test] fn detects_duplicate_tags() {
        let text = "AHU-1 15 kW\nAHU-1 20 kW";
        let pages = vec![(0, text.to_string(), "native_text".to_string())];
        let result = extract_load_schedule("s1", &pages);
        assert!(result.findings.iter().any(|f| f.finding_type == "duplicate_equipment_tag"));
    }
}
