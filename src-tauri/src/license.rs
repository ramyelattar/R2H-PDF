// Offline licensing trust model (schema 2, Ed25519).
//
// The application embeds ONLY the issuer's 32-byte public verification key.
// Licenses are signed offline by the license issuer with the private Ed25519
// key (never shipped, never committed; see security/license-issuer/README.md
// and scripts/license/). A signature over the canonical payload bytes is the
// only root of trust for the "licensed" mode.
//
// Legacy schema-1 licenses (HMAC over payload with an embedded shared
// secret) are detected and rejected explicitly. Trial-state files written by
// older builds are migrated in place: counters and the trial start are
// preserved, and the state is re-sealed with the current scheme.
//
// Trial-state integrity is a domain-separated SHA-256 digest. This is a
// casual-tamper deterrent only: a locally stored state file can always be
// deleted to start a fresh trial, and no embedded secret can change that.
// Authoritative entitlement comes exclusively from a signed license file.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TRIAL_DAYS: u64 = 14;
const MAX_LAUNCHES: u32 = 50;
const LICENSE_SCHEMA: u64 = 2;
const TRIAL_SCHEMA: u32 = 2;
const TRIAL_INTEGRITY_DOMAIN: &str = "R2H-PDF/trial-state/v2";

/// Ed25519 public verification key of the license issuer (raw, 32 bytes).
/// The matching private signing key is held by the license issuer only.
const ISSUER_PUBLIC_KEY_HEX: &str = "0223b5981e87f74837a2cdbf0276388a8442302d01061317035c8920ced4e311";

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
pub struct LicensePayload {
    pub license_id: String,
    pub subject: String,
    pub issued_unix: u64,
    pub expires_unix: u64,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    schema: u64,
    payload: LicensePayload,
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrialState {
    #[serde(default)]
    schema: u32,
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
    if license_file_path().is_file() {
        let payload = load_valid_license()?;
        let granted = payload
            .features
            .iter()
            .any(|f| f == feature || f == "*");
        if !granted {
            return Err(format!(
                "feature '{feature}' is not included in license {}",
                payload.license_id
            ));
        }
        return Ok(());
    }
    let trial_status = trial_status_and_record(Some(feature))?;
    if !trial_status.allowed {
        return Err(format!(
            "licensed feature blocked: {feature}; mode={}; reason={}",
            trial_status.mode, trial_status.reason
        ));
    }
    Ok(())
}

/// Verify and load the license file against the embedded issuer key.
/// Returns the validated payload or the exact rejection reason.
fn load_valid_license() -> Result<LicensePayload, String> {
    let path = license_file_path();
    let raw = fs::read_to_string(&path).map_err(|e| format!("read license {}: {e}", path.display()))?;
    let payload = verify_license_with_key(&raw, &verification_key())?;
    if is_expired(now_unix(), payload.expires_unix) {
        return Err("license expired".to_string());
    }
    Ok(payload)
}

/// Verify a serialized license document against the embedded issuer key.
/// Public so support tooling (e.g. examples/verify_license_file.rs) can
/// diagnose customer license files without shipping any signing capability.
pub fn verify_license_document(raw: &str) -> Result<LicensePayload, String> {
    verify_license_with_key(raw, &verification_key())
}

/// Verify a serialized license document against a public key.
/// Detects the legacy (schema-1, HMAC-based) format and rejects it explicitly.
fn verify_license_with_key(raw: &str, public_key: &[u8; 32]) -> Result<LicensePayload, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("license parse failed: {e}"))?;
    match value.get("schema") {
        None => {
            return Err(
                "legacy license format (schema 1) is no longer accepted; request a reissued schema-2 license"
                    .to_string(),
            )
        }
        Some(schema) => {
            let schema = schema.as_u64().ok_or_else(|| "license schema is not a number".to_string())?;
            if schema != LICENSE_SCHEMA {
                return Err(format!(
                    "unsupported license schema {schema} (supported: {LICENSE_SCHEMA})"
                ));
            }
        }
    }
    let file: LicenseFile =
        serde_json::from_str(raw).map_err(|e| format!("license parse failed: {e}"))?;
    let payload_json = serde_json::to_vec(&file.payload)
        .map_err(|e| format!("serialize license payload: {e}"))?;
    let sig_bytes = general_purpose::STANDARD
        .decode(file.signature.as_bytes())
        .map_err(|_| "license signature is not valid base64".to_string())?;
    let sig_array: [u8; SIGNATURE_LENGTH] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("license signature has invalid length {} (expected {SIGNATURE_LENGTH})", sig_bytes.len()))?;
    let signature = Signature::from_bytes(&sig_array);
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| "embedded issuer public key is invalid".to_string())?;
    verifying_key
        .verify_strict(&payload_json, &signature)
        .map_err(|_| "license signature mismatch".to_string())?;
    Ok(file.payload)
}

/// License validity boundary: the license remains valid through its
/// expiration second (`now == expires_unix` is still valid).
fn is_expired(now_unix: u64, expires_unix: u64) -> bool {
    now_unix > expires_unix
}

fn current_status() -> Result<LicenseStatus, String> {
    let dir = license_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create license dir {}: {e}", dir.display()))?;
    if license_file_path().is_file() {
        return Ok(match load_valid_license() {
            Ok(_) => status("licensed", true, "valid offline license"),
            Err(reason) => status("locked", false, &reason),
        });
    }
    trial_status_and_record(None)
}

/// Evaluate the trial state and, when a feature name is provided, record its
/// use — only after the trial has been judged valid, so a rejected trial
/// never mutates state. Trial-state files written by older builds (no schema
/// field) are migrated in place with counters and the trial start preserved.
fn trial_status_and_record(feature: Option<&str>) -> Result<LicenseStatus, String> {
    let trial_path = trial_file_path();
    let mut trial = if trial_path.is_file() {
        let raw = fs::read_to_string(&trial_path)
            .map_err(|e| format!("read trial state {}: {e}", trial_path.display()))?;
        match serde_json::from_str::<TrialState>(&raw) {
            Ok(trial) => trial,
            Err(e) => return Ok(status("locked", false, &format!("trial parse failed: {e}"))),
        }
    } else {
        fresh_trial()
    };

    if trial.schema == 0 {
        // Legacy pre-schema trial state: migrate, preserving the original
        // trial start and counters. Grants no additional trial time.
        trial.schema = TRIAL_SCHEMA;
        trial.integrity = String::new();
    } else if trial.schema != TRIAL_SCHEMA {
        return Ok(status(
            "locked",
            false,
            &format!("unsupported trial state schema {} (supported: {TRIAL_SCHEMA})", trial.schema),
        ));
    }

    if trial.integrity.is_empty() {
        trial.integrity = trial_integrity(&trial)?;
        write_trial_state(&trial)?;
    } else if trial.integrity != trial_integrity(&trial)? {
        return Ok(status("locked", false, "trial integrity mismatch"));
    }

    let now = now_unix();
    let evaluated = if now.saturating_add(300) < trial.last_seen_unix {
        status("locked", false, "system date rollback detected")
    } else if trial.expired {
        status("trial-expired", false, "trial marked expired")
    } else if now.saturating_sub(trial.first_seen_unix) > TRIAL_DAYS * 24 * 60 * 60 {
        status("trial-expired", false, "trial days exceeded")
    } else if trial.launches > MAX_LAUNCHES {
        status("trial-expired", false, "trial launch count exceeded")
    } else {
        status("trial", true, "valid offline trial")
    };

    if evaluated.allowed {
        if let Some(feature) = feature {
            trial.last_seen_unix = now_unix();
            match feature {
                "export" => trial.exports += 1,
                "ocr" => trial.ocr_runs += 1,
                "rag" => trial.rag_runs += 1,
                "compare" => trial.compare_runs += 1,
                _ => {}
            }
            trial.integrity = trial_integrity(&trial)?;
            write_trial_state(&trial)?;
        }
    }

    Ok(evaluated)
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

/// Rejection-path and trial smoke. In the asymmetric trust model the
/// application cannot mint a valid license (that requires the issuer's
/// private key); the valid-license path is covered by unit tests that supply
/// their own test keypair and verification-key override.
fn run_offline_license_smoke() -> Result<LicenseSmokeReport, String> {
    reset_store()?;
    let mut probes = vec![probe_feature("export", true)];
    write_license_file(r#"{"payload":{"subject":"garbage"},"signature":"bad"}"#)?;
    probes.push(probe_feature("export", false));
    // Structurally valid schema-2 license with a zeroed signature: rejected
    // because the application can only verify, never trust unlicensed data.
    write_license_file(
        r#"{"schema":2,"payload":{"license_id":"smoke","subject":"unsigned","issued_unix":0,"expires_unix":9999999999,"features":["export"]},"signature":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="}"#,
    )?;
    probes.push(probe_feature("export", false));
    write_license_file(r#"{"payload":{"subject":"legacy"},"signature":"deadbeef"}"#)?;
    probes.push(probe_feature("export", false));
    remove_license_file()?;
    probes.push(probe_feature("export", true));
    Ok(report(
        "offline",
        probes,
        vec![
            "Fresh trial permits gated features without network access.".to_string(),
            "Garbage, unsigned, and legacy licenses are each rejected and leave gated features locked.".to_string(),
            "Valid-license acceptance is covered by unit tests with an isolated test keypair.".to_string(),
        ],
    ))
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
    Ok(report(
        "trial-expiry",
        probes,
        vec![
            "Fresh trial permits gated features within deterministic local limits.".to_string(),
            "Expired trial and date rollback state block gated features offline.".to_string(),
        ],
    ))
}

fn run_license_tamper_smoke() -> Result<LicenseSmokeReport, String> {
    reset_store()?;
    write_license_file(
        r#"{"schema":2,"payload":{"license_id":"tamper-smoke","subject":"Tamper Probe","issued_unix":0,"expires_unix":9999999999,"features":["export","ocr","rag","compare"]},"signature":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=="}"#,
    )?;
    let mut probes = vec![probe_feature("export", false)];
    remove_license_file()?;
    write_trial(false, true)?;
    probes.push(probe_feature("ocr", false));
    Ok(report(
        "tamper",
        probes,
        vec![
            "License payload with an invalid signature is rejected.".to_string(),
            "Tampered trial integrity is rejected and treated as locked.".to_string(),
        ],
    ))
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

fn fresh_trial() -> TrialState {
    TrialState {
        schema: TRIAL_SCHEMA,
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
}

fn write_license_file(contents: &str) -> Result<(), String> {
    fs::write(license_file_path(), contents).map_err(|e| format!("write license: {e}"))
}

fn remove_license_file() -> Result<(), String> {
    let path = license_file_path();
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("remove license: {e}"))?;
    }
    Ok(())
}

fn write_trial(expired: bool, tampered: bool) -> Result<(), String> {
    let mut trial = TrialState {
        schema: TRIAL_SCHEMA,
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
        schema: TRIAL_SCHEMA,
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
    let mut hasher = Sha256::new();
    hasher.update(TRIAL_INTEGRITY_DOMAIN.as_bytes());
    hasher.update(b"\n");
    hasher.update(body.as_bytes());
    Ok(to_hex(&hasher.finalize()))
}

fn reset_store() -> Result<(), String> {
    let dir = license_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("remove license dir {}: {e}", dir.display()))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create license dir {}: {e}", dir.display()))
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

fn verification_key() -> [u8; 32] {
    #[cfg(test)]
    {
        if let Ok(guard) = TEST_VERIFICATION_KEY_OVERRIDE.lock() {
            if let Some(key) = *guard {
                return key;
            }
        }
    }
    let mut key = [0u8; 32];
    let hex = ISSUER_PUBLIC_KEY_HEX;
    if hex.len() != 64 {
        panic!("embedded issuer public key must be 64 hex characters");
    }
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .unwrap_or_else(|e| panic!("embedded issuer public key is not valid hex: {e}"));
    }
    key
}

/// Test-only override of the verification key so unit tests can exercise the
/// full licensing flow with their own keypair. This static does not exist in
/// production builds.
#[cfg(test)]
static TEST_VERIFICATION_KEY_OVERRIDE: std::sync::Mutex<Option<[u8; 32]>> = std::sync::Mutex::new(None);

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

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::sync::{Mutex, MutexGuard};

    /// License tests mutate process-global state (env dir + key override) and
    /// therefore run serialized under this lock.
    static LICENSE_TEST_LOCK: Mutex<()> = Mutex::new(());

    const TEST_SEED_A: [u8; 32] = [7u8; 32];
    const TEST_SEED_B: [u8; 32] = [11u8; 32];

    struct TestEnv {
        _guard: MutexGuard<'static, ()>,
    }

    impl TestEnv {
        fn new(label: &str) -> Self {
            let guard = LICENSE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let dir = std::env::temp_dir()
                .join(format!("r2h-license-test-{}-{}", std::process::id(), label));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create test license dir");
            std::env::set_var("R2H_LICENSE_DIR", &dir);
            TestEnv { _guard: guard }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            std::env::remove_var("R2H_LICENSE_DIR");
        }
    }

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&TEST_SEED_A)
    }

    fn set_verification_key(key: [u8; 32]) {
        *TEST_VERIFICATION_KEY_OVERRIDE.lock().unwrap() = Some(key);
    }

    fn clear_verification_key() {
        *TEST_VERIFICATION_KEY_OVERRIDE.lock().unwrap() = None;
    }

    fn signed_license_json(key: &SigningKey, payload: &LicensePayload) -> String {
        let payload_json = serde_json::to_vec(payload).unwrap();
        let signature = key.sign(&payload_json);
        let file = LicenseFile {
            schema: LICENSE_SCHEMA,
            payload: serde_json::from_str(&serde_json::to_string(payload).unwrap()).unwrap(),
            signature: general_purpose::STANDARD.encode(signature.to_bytes()),
        };
        serde_json::to_string(&file).unwrap()
    }

    fn payload(expires_unix: u64, features: &[&str]) -> LicensePayload {
        LicensePayload {
            license_id: "test-license-0001".to_string(),
            subject: "Unit Test Licensee".to_string(),
            issued_unix: now_unix().saturating_sub(60),
            expires_unix,
            features: features.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn expiry_boundary_is_inclusive() {
        // A license is valid through its expiration second.
        assert!(!is_expired(1000, 1000));
        assert!(is_expired(1001, 1000));
    }

    #[test]
    fn valid_signed_license_unlocks_features() {
        let _env = TestEnv::new("valid");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let license = signed_license_json(
            &test_signing_key(),
            &payload(now_unix() + 365 * 24 * 60 * 60, &["export", "ocr", "rag", "compare"]),
        );
        write_license_file(&license).unwrap();
        let status = current_status().unwrap();
        assert_eq!(status.mode, "licensed");
        assert!(status.allowed);
        assert_eq!(status.reason, "valid offline license");
        assert_feature_allowed("export").unwrap();
        assert_feature_allowed("compare").unwrap();
        clear_verification_key();
    }

    #[test]
    fn feature_not_in_license_is_blocked() {
        let _env = TestEnv::new("feature-scope");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let license = signed_license_json(&test_signing_key(), &payload(now_unix() + 3600, &["ocr"]));
        write_license_file(&license).unwrap();
        assert_feature_allowed("ocr").unwrap();
        let err = assert_feature_allowed("export").unwrap_err();
        assert!(err.contains("not included in license"), "unexpected error: {err}");
        clear_verification_key();
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let _env = TestEnv::new("invalid-sig");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        // Structurally valid file, but the signature is a zeroed value that
        // matches no real signature.
        let zeroed_signature = general_purpose::STANDARD.encode([0u8; SIGNATURE_LENGTH]);
        let mut file: serde_json::Value =
            serde_json::from_str(&signed_license_json(&test_signing_key(), &payload(now_unix() + 3600, &["export"]))).unwrap();
        file["signature"] = serde_json::Value::String(zeroed_signature);
        write_license_file(&file.to_string()).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert!(status.reason.contains("signature mismatch"), "unexpected reason: {}", status.reason);
        clear_verification_key();
    }

    #[test]
    fn modified_payload_is_rejected() {
        let _env = TestEnv::new("modified-payload");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let mut tampered = payload(now_unix() + 365 * 24 * 60 * 60, &["export"]);
        tampered.subject = "Tampered Subject".to_string();
        // Signed with the key, but the payload was modified afterwards.
        let signature = test_signing_key().sign(&serde_json::to_vec(&payload(now_unix() + 3600, &["export"])).unwrap());
        let file = LicenseFile {
            schema: LICENSE_SCHEMA,
            payload: serde_json::from_str(&serde_json::to_string(&tampered).unwrap()).unwrap(),
            signature: general_purpose::STANDARD.encode(signature.to_bytes()),
        };
        write_license_file(&serde_json::to_string(&file).unwrap()).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert_eq!(status.reason, "license signature mismatch");
        clear_verification_key();
    }

    #[test]
    fn wrong_public_key_is_rejected() {
        let _env = TestEnv::new("wrong-key");
        // Verification is pinned to key A; the license is signed with key B.
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let other_key = SigningKey::from_bytes(&TEST_SEED_B);
        let license = signed_license_json(&other_key, &payload(now_unix() + 3600, &["export"]));
        write_license_file(&license).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert_eq!(status.reason, "license signature mismatch");
        clear_verification_key();
    }

    #[test]
    fn expired_license_is_rejected() {
        let _env = TestEnv::new("expired");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let license = signed_license_json(&test_signing_key(), &payload(now_unix().saturating_sub(10), &["export"]));
        write_license_file(&license).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert_eq!(status.reason, "license expired");
        clear_verification_key();
    }

    #[test]
    fn malformed_license_is_rejected() {
        let _env = TestEnv::new("malformed");
        write_license_file("this is not json {{{").unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert!(status.reason.starts_with("license parse failed"));
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let _env = TestEnv::new("unsupported-schema");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        let license = signed_license_json(&test_signing_key(), &payload(now_unix() + 3600, &["export"]));
        let upgraded = license.replace("\"schema\":2", "\"schema\":3");
        assert_ne!(license, upgraded);
        write_license_file(&upgraded).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert!(status.reason.contains("unsupported license schema 3"), "unexpected reason: {}", status.reason);
        clear_verification_key();
    }

    #[test]
    fn legacy_license_is_rejected_explicitly() {
        let _env = TestEnv::new("legacy");
        write_license_file(
            r#"{"payload":{"subject":"legacy","expires_unix":9999999999,"features":["export"]},"signature":"0123456789abcdef"}"#,
        )
        .unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert!(
            status.reason.contains("legacy license format"),
            "unexpected reason: {}",
            status.reason
        );
    }

    #[test]
    fn license_missing_required_fields_is_rejected() {
        let _env = TestEnv::new("missing-fields");
        set_verification_key(test_signing_key().verifying_key().to_bytes());
        // Structurally signed, but the payload is missing required fields.
        let key = test_signing_key();
        let signature = key.sign(br#"{"license_id":"x"}"#);
        let file = format!(
            r#"{{"schema":2,"payload":{{"license_id":"x"}},"signature":"{}"}}"#,
            general_purpose::STANDARD.encode(signature.to_bytes())
        );
        write_license_file(&file).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert!(status.reason.starts_with("license parse failed"), "unexpected reason: {}", status.reason);
        clear_verification_key();
    }

    #[test]
    fn fresh_trial_allows_gated_features_and_persists_state() {
        let _env = TestEnv::new("trial-fresh");
        let status = current_status().unwrap();
        assert_eq!(status.mode, "trial");
        assert!(status.allowed);
        assert_feature_allowed("ocr").unwrap();
        let state = fs::read_to_string(trial_file_path()).unwrap();
        assert!(state.contains("\"schema\": 2"), "trial state should carry schema 2: {state}");
        assert!(state.contains("\"ocr_runs\": 1"));
    }

    #[test]
    fn trial_launch_count_exceeded_expires_trial() {
        let _env = TestEnv::new("trial-launches");
        let mut trial = fresh_trial();
        trial.launches = MAX_LAUNCHES + 1;
        trial.integrity = trial_integrity(&trial).unwrap();
        write_trial_state(&trial).unwrap();
        let status = current_status().unwrap();
        assert_eq!(status.mode, "trial-expired");
        assert_eq!(status.reason, "trial launch count exceeded");
    }

    #[test]
    fn trial_date_rollback_is_detected() {
        let _env = TestEnv::new("trial-rollback");
        write_rollback_trial().unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert_eq!(status.reason, "system date rollback detected");
    }

    #[test]
    fn rejected_trial_feature_use_does_not_mutate_state() {
        let _env = TestEnv::new("trial-rollback-record");
        write_rollback_trial().unwrap();
        let before = fs::read_to_string(trial_file_path()).unwrap();
        let err = assert_feature_allowed("export").unwrap_err();
        assert!(err.contains("rollback"), "unexpected error: {err}");
        let after = fs::read_to_string(trial_file_path()).unwrap();
        assert_eq!(before, after, "a rejected feature use must not rewrite trial state");
    }

    #[test]
    fn trial_integrity_mismatch_is_locked() {
        let _env = TestEnv::new("trial-tamper");
        write_trial(false, true).unwrap();
        let status = current_status().unwrap();
        assert!(!status.allowed);
        assert_eq!(status.reason, "trial integrity mismatch");
    }

    #[test]
    fn legacy_trial_state_migrates_preserving_first_seen() {
        let _env = TestEnv::new("trial-legacy");
        let first_seen = now_unix().saturating_sub(3 * 24 * 60 * 60);
        let legacy = format!(
            r#"{{"first_seen_unix":{first_seen},"last_seen_unix":{},"launches":2,"exports":1,"ocr_runs":0,"rag_runs":0,"compare_runs":0,"expired":false,"integrity":"deadbeef"}}"#,
            now_unix()
        );
        write_trial_file_contents(&legacy).unwrap();
        let status = current_status().unwrap();
        assert_eq!(status.mode, "trial", "legacy trial should migrate, not lock: {}", status.reason);
        let migrated = fs::read_to_string(trial_file_path()).unwrap();
        assert!(migrated.contains("\"schema\": 2"));
        assert!(
            migrated.contains(&format!("\"first_seen_unix\": {first_seen}")),
            "migration must preserve the original trial start: {migrated}"
        );
        // A second evaluation must succeed against the re-sealed state.
        let status = current_status().unwrap();
        assert_eq!(status.mode, "trial");
    }

    #[test]
    fn trial_integrity_is_stable_for_known_input() {
        let trial = fresh_trial();
        let digest = trial_integrity(&trial).unwrap();
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, trial_integrity(&trial).unwrap());
    }

    fn write_trial_file_contents(contents: &str) -> Result<(), String> {
        fs::write(trial_file_path(), contents).map_err(|e| format!("write trial state: {e}"))
    }
}
