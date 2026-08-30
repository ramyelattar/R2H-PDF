use std::path::Path;

use super::errors::DocumentCoreError;
use super::types::RecoveryReport;

pub struct RecoveryManager;

impl RecoveryManager {
    pub fn recover_pdf_bytes(path: &Path, original_bytes: &[u8]) -> Result<(Vec<u8>, RecoveryReport), DocumentCoreError> {
        let mut warnings = Vec::new();

        let header_pos = find_subsequence(original_bytes, b"%PDF-");
        let eof_pos = find_last_subsequence(original_bytes, b"%%EOF");

        let (start, end) = match (header_pos, eof_pos) {
            (Some(h), Some(e)) if e > h => (h, e + 5),
            (Some(h), None) => {
                warnings.push("Missing EOF marker; appended synthetic EOF.".to_string());
                (h, original_bytes.len())
            }
            _ => {
                return Err(DocumentCoreError::RecoveryFailed(format!(
                    "Unable to locate valid PDF envelope for {}",
                    path.display()
                )))
            }
        };

        let mut repaired = original_bytes[start..end].to_vec();
        if !repaired.ends_with(b"%%EOF") {
            repaired.extend_from_slice(b"\n%%EOF\n");
        }

        let report = RecoveryReport {
            recovered: true,
            method: "header+eof-envelope-salvage".to_string(),
            warnings,
            repaired_bytes: repaired.len(),
        };

        Ok((repaired, report))
    }

    pub fn recover_file(path: &Path) -> Result<RecoveryReport, DocumentCoreError> {
        let bytes = std::fs::read(path)?;
        let (_, report) = Self::recover_pdf_bytes(path, &bytes)?;
        Ok(report)
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack.windows(needle.len()).position(|window| window == needle)
}

fn find_last_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}
