//! Deterministic unit normalization for electrical engineering values.

use super::types::{EngineeringUnit, NormalizedValue, UnitParseResult};
use regex::Regex;

/// Parse a text string and extract all recognizable engineering values.
pub fn parse_units(text: &str) -> UnitParseResult {
    let mut values = Vec::new();
    let mut warnings = Vec::new();

    // Power: W, kW
    for cap in re_power_w().captures_iter(text) {
        if let Some(num) = cap.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
            let unit_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let (orig_unit, norm_val, norm_unit) = if unit_str.eq_ignore_ascii_case("kw") {
                (EngineeringUnit::KW, num, EngineeringUnit::KW)
            } else {
                (EngineeringUnit::W, num / 1000.0, EngineeringUnit::KW)
            };
            values.push(NormalizedValue {
                original_text: cap[0].to_string(),
                numeric_value: num,
                original_unit: orig_unit,
                normalized_value: norm_val,
                normalized_unit: norm_unit,
                confidence: 0.95,
            });
        }
    }

    // Apparent power: VA, kVA
    for cap in re_power_va().captures_iter(text) {
        if let Some(num) = cap.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
            let unit_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let (orig_unit, norm_val, norm_unit) = if unit_str.eq_ignore_ascii_case("kva") {
                (EngineeringUnit::KVA, num, EngineeringUnit::KVA)
            } else {
                (EngineeringUnit::VA, num / 1000.0, EngineeringUnit::KVA)
            };
            values.push(NormalizedValue {
                original_text: cap[0].to_string(),
                numeric_value: num,
                original_unit: orig_unit,
                normalized_value: norm_val,
                normalized_unit: norm_unit,
                confidence: 0.95,
            });
        }
    }

    // Current: A, amp, amps
    for cap in re_current().captures_iter(text) {
        if let Some(num) = cap.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
            values.push(NormalizedValue {
                original_text: cap[0].to_string(),
                numeric_value: num,
                original_unit: EngineeringUnit::A,
                normalized_value: num,
                normalized_unit: EngineeringUnit::A,
                confidence: 0.9,
            });
        }
    }

    // Voltage: V, volt, volts
    for cap in re_voltage().captures_iter(text) {
        if let Some(num) = cap.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
            values.push(NormalizedValue {
                original_text: cap[0].to_string(),
                numeric_value: num,
                original_unit: EngineeringUnit::V,
                normalized_value: num,
                normalized_unit: EngineeringUnit::V,
                confidence: 0.9,
            });
        }
    }

    // Power factor: PF, power factor, cosφ
    for cap in re_power_factor().captures_iter(text) {
        if let Some(num) = cap.get(1).and_then(|m| m.as_str().parse::<f64>().ok()) {
            if num > 0.0 && num <= 1.0 {
                values.push(NormalizedValue {
                    original_text: cap[0].to_string(),
                    numeric_value: num,
                    original_unit: EngineeringUnit::PowerFactor,
                    normalized_value: num,
                    normalized_unit: EngineeringUnit::PowerFactor,
                    confidence: 0.85,
                });
            } else {
                warnings.push(format!("Power factor {} is out of range (0-1)", num));
            }
        }
    }

    UnitParseResult { values, warnings }
}

fn re_power_w() -> Regex {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(kw|w)\b").unwrap()
}
fn re_power_va() -> Regex {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(kva|va)\b").unwrap()
}
fn re_current() -> Regex {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(?:amps?|a)\b").unwrap()
}
fn re_voltage() -> Regex {
    Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(?:volts?|v)\b").unwrap()
}
fn re_power_factor() -> Regex {
    Regex::new(r"(?i)(?:pf|power\s*factor|cos\s*[φϕ])\s*[:=]?\s*(\d+(?:\.\d+)?)").unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_1200_w() {
        let r = parse_units("1200 W");
        assert_eq!(r.values.len(), 1);
        assert!((r.values[0].normalized_value - 1.2).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::KW);
    }
    #[test]
    fn parse_1_5_kw() {
        let r = parse_units("1.5 kW");
        assert!((r.values[0].normalized_value - 1.5).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::KW);
    }
    #[test]
    fn parse_2500_va() {
        let r = parse_units("2500 VA");
        assert!((r.values[0].normalized_value - 2.5).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::KVA);
    }
    #[test]
    fn parse_3_2_kva() {
        let r = parse_units("3.2 kVA");
        assert!((r.values[0].normalized_value - 3.2).abs() < 0.001);
    }
    #[test]
    fn parse_16_a() {
        let r = parse_units("16 A");
        assert!((r.values[0].normalized_value - 16.0).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::A);
    }
    #[test]
    fn parse_230_v() {
        let r = parse_units("230 V");
        assert!((r.values[0].normalized_value - 230.0).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::V);
    }
    #[test]
    fn parse_pf_085() {
        let r = parse_units("PF 0.85");
        assert!((r.values[0].normalized_value - 0.85).abs() < 0.001);
        assert_eq!(r.values[0].normalized_unit, EngineeringUnit::PowerFactor);
    }
    #[test]
    fn reject_invalid_pf() {
        let r = parse_units("PF 1.5");
        assert!(r.values.is_empty());
        assert!(!r.warnings.is_empty());
    }
}
