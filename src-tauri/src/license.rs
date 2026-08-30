use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TRIAL_DAYS: u64 = 14;
const MAX_LAUNCHES: u32 = 50;
const SIGNING_SECRET: &str = "r2h-pdf-phase-36-offline-license-test-key";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub mode: String,
    pub allowed: bool,
    pub reason: String,
    pub trial_days: u64,
    pub max_launches: u32,
    pub license_path: String,
    pub trial_state_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicensePayload {
    subject: String,
    expires_unix: u64,
    features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    payload: LicensePayload,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrialState {
    first_seen_unix: u64,
    last_seen_unix: u64,
    launches: u32,
    exports: u32,
    ocr_runs: u32,
    rag_runs: u32,
    compare_runs: u32,
    expired: bool,
    integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureProbe {
    pub feature: String,
    pub expected_allowed: bool,
    pub actual_allowed: bool,
    pub result: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseSmokeReport {
    pub mode: String,
    pub status: String,
    pub license_dir: String,
    pub trial_policy: String,
    pub probes: Vec<FeatureProbe>,
    pub notes: Vec<String>,
}

#[tauri::command]
pub fn license_get_status() -> Result<LicenseStatus, String> {
    current_status()
}

pub fn assert_feature_allowed(feature: &str) -> Result<(), String> {
    let status = current_status()?;
    if !status.allowed {
        return Err(format!(
            "licensed feature blocked: {feature}; mode={}; reason={}",
            status.mode, status.reason
        ));
    }
    record_feature_use(feature)?;
    Ok(())
}

pub fn run_license_smoke(mode: &str) -> Result<LicenseSmokeReport, String> {
    fs::create_dir_all(license_dir()).map_err(|e| format!("create license dir: {e}"))?;
    match mode {
        "offline" => run_offline_license_smoke(),
        "trial-expiry" => run_trial_expiry_smoke(),
        "tamper" => run_license_tamper_smoke(),
        other => Err(format!("unknown license smoke mode: {other}")),
    }
}

fn run_offline_license_smoke() -> Result<LicenseSmokeReport, String> {
    reset_store()?;
    write_license(true, &["export", "ocr", "rag", "compare"])?;
    let mut probes = Vec::new();
    for feature in ["export", "ocr", "rag", "compare"] {
        probes.push(probe_feature(feature, true));
    }
    fs::write(license_file_path(), "{\"payload\":{\"subject\":\"tampered\"},\"signature\":\"bad\"}")
        .map_err(|e| format!("write invalid license: {e}"))?;
    probes.push(probe_feature("export", false));
    Ok(report("offline", probes, vec![
        "Valid offline test license unlocks gated features without network access.".to_string(),
        "Invalid license is rejected and leaves gated features locked.".to_string(),
    ]))
}

fn run_trial_expiry_smoke() -> Result<LicenseSmokeReport, String> {
    reset_store()?;
    write_trial(false, false)?;
    let mut probes = vec![probe_feature("export", true)];
    write_trial(true, false)?;
    for feature in ["export", "ocr", "rag", "compare"] {
        probes.push(probe_feature(feature, false));
    }
    write_rollback_trial()?;
    probes.push(probe_feature("export", false));
    Ok(report("trial-expiry", probes, vec![
        "Fresh trial permits gated features within deterministic local limits.".to_string(),
        "Expired trial and date rollback state block gated features offline.".to_string(),
    ]))
}

fn run_license_tamper_smoke() -> Result<LicenseSmokeReport, String> {
    reset_store()?;
    write_license(true, &["export", "ocr", "rag", "compare"])?;
    let original = fs::read_to_string(license_file_path())
        .map_err(|e| format!("read generated license: {e}"))?;
    let tampered = original.replace("Phase 36.6 Test License", "Tampered Phase 36.6 License");
    fs::write(license_file_path(), tampered).map_err(|e| format!("write tampered license: {e}"))?;
    let mut probes = vec![probe_feature("export", false)];
    fs::remove_file(license_file_path()).map_err(|e| format!("remove tampered license: {e}"))?;
    write_trial(false, true)?;
    probes.push(probe_feature("ocr", false));
    Ok(report("tamper", probes, vec![
        "Modified license payload is rejected because the signature no longer matches.".to_string(),
        "Tampered trial integrity is rejected and treated as locked.".to_string(),
    ]))
}

fn probe_feature(feature: &str, expected_allowed: bool) -> FeatureProbe {
    let result = assert_feature_allowed(feature);
    let actual_allowed = result.is_ok();
    FeatureProbe {
        feature: feature.to_string(),
        expected_allowed,
        actual_allowed,
        result: if actual_allowed == expected_allowed { "PASS" } else { "FAIL" }.to_string(),
        reason: result.err().unwrap_or_else(|| "feature allowed".to_string()),
    }
}

fn report(mode: &str, probes: Vec<FeatureProbe>, notes: Vec<String>) -> LicenseSmokeReport {
    let failed = probes.iter().any(|p| p.result != "PASS");
    LicenseSmokeReport {
        mode: mode.to_string(),
        status: if failed { "FAIL" } else { "PASS" }.to_string(),
        license_dir: license_dir().display().to_string(),
        trial_policy: format!("{TRIAL_DAYS} days, {MAX_LAUNCHES} launches; export/OCR/RAG/compare gated"),
        probes,
        notes,
    }
}

fn current_status() -> Result<LicenseStatus, String> {
    let dir = license_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create license dir {}: {e}", dir.display()))?;
    let license_path = license_file_path();
    let trial_path = trial_file_path();

    if license_path.is_file() {
        let raw = fs::read_to_string(&license_path)
            .map_err(|e| format!("read license {}: {e}", license_path.display()))?;
        let license: LicenseFile = match serde_json::from_str(&raw) {
            Ok(license) => license,
            Err(e) => return Ok(status("locked", false, &format!("license parse failed: {e}"))),
        };
        let payload_json = serde_json::to_string(&license.payload)
            .map_err(|e| format!("serialize license payload: {e}"))?;
        if sign(&payload_json) != license.signature {
            return Ok(status("locked", false, "license signature mismatch"));
        }
        if now_unix() > license.payload.expires_unix {
            return Ok(status("locked", false, "license expired"));
        }
        return Ok(status("licensed", true, "valid offline license"));
    }

    let mut trial = if trial_path.is_file() {
        let raw = fs::read_to_string(&trial_path)
            .map_err(|e| format!("read trial state {}: {e}", trial_path.display()))?;
        match serde_json::from_str::<TrialState>(&raw) {
            Ok(trial) => trial,
            Err(e) => return Ok(status("locked", false, &format!("trial parse failed: {e}"))),
        }
    } else {
        TrialState {
            first_seen_unix: now_unix(),
            last_seen_unix: now_unix(),
            launches: 0,
            exports: 0,
            ocr_runs: 0,
            rag_runs: 0,
            compare_runs: 0,
            expired: false,
            integrity: String::new(),
        }
    };

    if trial.integrity.is_empty() {
        trial.integrity = trial_integrity(&trial)?;
        write_trial_state(&trial)?;
    } else if trial.integrity != trial_integrity(&trial)? {
        return Ok(status("locked", false, "trial integrity mismatch"));
    }

    let now = now_unix();
    if now.saturating_add(300) < trial.last_seen_unix {
        return Ok(status("locked", false, "system date rollback detected"));
    }
    if trial.expired {
        return Ok(status("trial-expired", false, "trial marked expired"));
    }
    if now.saturating_sub(trial.first_seen_unix) > TRIAL_DAYS * 24 * 60 * 60 {
        return Ok(status("trial-expired", false, "trial days exceeded"));
    }
    if trial.launches > MAX_LAUNCHES {
        return Ok(status("trial-expired", false, "trial launch count exceeded"));
    }
    Ok(status("trial", true, "valid offline trial"))
}

fn record_feature_use(feature: &str) -> Result<(), String> {
    if license_file_path().is_file() {
        return Ok(());
    }
    let trial_path = trial_file_path();
    let mut trial = if trial_path.is_file() {
        let raw = fs::read_to_string(&trial_path)
            .map_err(|e| format!("read trial state: {e}"))?;
        serde_json::from_str::<TrialState>(&raw).map_err(|e| format!("parse trial state: {e}"))?
    } else {
        TrialState {
            first_seen_unix: now_unix(),
            last_seen_unix: now_unix(),
            launches: 0,
            exports: 0,
            ocr_runs: 0,
            rag_runs: 0,
            compare_runs: 0,
            expired: false,
            integrity: String::new(),
        }
    };
    trial.last_seen_unix = now_unix();
    match feature {
        "export" => trial.exports += 1,
        "ocr" => trial.ocr_runs += 1,
        "rag" => trial.rag_runs += 1,
        "compare" => trial.compare_runs += 1,
        _ => {}
    }
    trial.integrity = trial_integrity(&trial)?;
    write_trial_state(&trial)
}

fn status(mode: &str, allowed: bool, reason: &str) -> LicenseStatus {
    LicenseStatus {
        mode: mode.to_string(),
        allowed,
        reason: reason.to_string(),
        trial_days: TRIAL_DAYS,
        max_launches: MAX_LAUNCHES,
        license_path: license_file_path().display().to_string(),
        trial_state_path: trial_file_path().display().to_string(),
    }
}

fn reset_store() -> Result<(), String> {
    let dir = license_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("remove license dir {}: {e}", dir.display()))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create license dir {}: {e}", dir.display()))
}

fn write_license(valid: bool, features: &[&str]) -> Result<(), String> {
    let payload = LicensePayload {
        subject: "Phase 36.6 Test License".to_string(),
        expires_unix: now_unix() + 365 * 24 * 60 * 60,
        features: features.iter().map(|s| s.to_string()).collect(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| format!("serialize license payload: {e}"))?;
    let signature = if valid { sign(&payload_json) } else { "invalid".to_string() };
    let license = LicenseFile { payload, signature };
    fs::write(
        license_file_path(),
        serde_json::to_string_pretty(&license).map_err(|e| format!("serialize license: {e}"))?,
    )
    .map_err(|e| format!("write license: {e}"))
}

fn write_trial(expired: bool, tampered: bool) -> Result<(), String> {
    let mut trial = TrialState {
        first_seen_unix: now_unix().saturating_sub(if expired { (TRIAL_DAYS + 2) * 24 * 60 * 60 } else { 0 }),
        last_seen_unix: now_unix(),
        launches: 1,
        exports: 0,
        ocr_runs: 0,
        rag_runs: 0,
        compare_runs: 0,
        expired,
        integrity: String::new(),
    };
    trial.integrity = if tampered { "tampered".to_string() } else { trial_integrity(&trial)? };
    write_trial_state(&trial)
}

fn write_rollback_trial() -> Result<(), String> {
    let mut trial = TrialState {
        first_seen_unix: now_unix(),
        last_seen_unix: now_unix() + 10 * 24 * 60 * 60,
        launches: 1,
        exports: 0,
        ocr_runs: 0,
        rag_runs: 0,
        compare_runs: 0,
        expired: false,
        integrity: String::new(),
    };
    trial.integrity = trial_integrity(&trial)?;
    write_trial_state(&trial)
}

fn write_trial_state(trial: &TrialState) -> Result<(), String> {
    fs::write(
        trial_file_path(),
        serde_json::to_string_pretty(trial).map_err(|e| format!("serialize trial state: {e}"))?,
    )
    .map_err(|e| format!("write trial state: {e}"))
}

fn trial_integrity(trial: &TrialState) -> Result<String, String> {
    let body = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        trial.first_seen_unix,
        trial.last_seen_unix,
        trial.launches,
        trial.exports,
        trial.ocr_runs,
        trial.rag_runs,
        trial.compare_runs,
        trial.expired
    );
    Ok(sign(&body))
}

fn license_dir() -> PathBuf {
    if let Ok(path) = std::env::var("R2H_LICENSE_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(path).join("R2H-PDF").join("license");
    }
    std::env::temp_dir().join("R2H-PDF").join("license")
}

fn license_file_path() -> PathBuf {
    license_dir().join("license.json")
}

fn trial_file_path() -> PathBuf {
    license_dir().join("trial-state.json")
}

fn sign(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SIGNING_SECRET.as_bytes());
    hasher.update(b"\n");
    hasher.update(input.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
