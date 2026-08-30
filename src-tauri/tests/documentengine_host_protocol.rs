use r2h_pdf_lib::document_engine_host::{handle_line, PROTOCOL_VERSION};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn response(line: &str) -> Value {
    serde_json::from_str(&handle_line(line)).expect("protocol response is JSON")
}

#[test]
fn hello_returns_exact_request_id_and_real_read_capabilities() {
    let value = response(&format!(
        r#"{{"protocolVersion":"{}","requestId":"req-1","method":"HELLO","params":{{}}}}"#,
        PROTOCOL_VERSION
    ));
    assert_eq!(value["requestId"], "req-1");
    assert!(value.get("result").is_some());
    assert!(value.get("error").is_none());
    assert_eq!(value["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(value["result"]["capabilities"]["nativeText"], true);
    assert_eq!(value["result"]["capabilities"]["pageRender"], true);
    assert!(value["result"]["capabilities"].get("ocr").is_none());
    assert_eq!(value["result"]["capabilities"]["vectorRead"], true);
    assert!(value["result"]["capabilities"]
        .get("imageExtraction")
        .is_none());
    assert!(value.to_string().find("C:\\").is_none());
}

#[test]
fn protocol_rejects_unknown_wrong_version_and_malformed_requests_without_panicking() {
    let unknown = response(&format!(
        r#"{{"protocolVersion":"{}","requestId":"req-2","method":"EDIT_DOCUMENT","params":{{}}}}"#,
        PROTOCOL_VERSION
    ));
    assert_eq!(unknown["error"]["code"], "UNSUPPORTED_METHOD");

    let wrong_version =
        response(r#"{"protocolVersion":"wrong","requestId":"req-3","method":"HELLO","params":{}}"#);
    assert_eq!(wrong_version["requestId"], "req-3");
    assert_eq!(
        wrong_version["error"]["code"],
        "UNSUPPORTED_PROTOCOL_VERSION"
    );

    let malformed = response("{not-json");
    assert_eq!(malformed["error"]["code"], "INVALID_REQUEST");
    assert!(malformed.get("result").is_none());
}

#[test]
fn health_is_stable_and_request_ids_are_transport_only() {
    let input = format!(
        r#"{{"protocolVersion":"{}","requestId":"transport-42","method":"HEALTH","params":{{}}}}"#,
        PROTOCOL_VERSION
    );
    let first = response(&input);
    let second = response(&input);
    assert_eq!(first, second);
    assert_eq!(first["requestId"], "transport-42");
    assert_eq!(first["result"]["status"], "READY");
}

static NEXT_TEST_ROOT: AtomicUsize = AtomicUsize::new(1);

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("e2-2b-text-rotated.pdf")
}

fn request(request_id: &str, method: &str, params: Value) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "method": method,
        "params": params,
    })
}

fn test_artifact_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "r2h-e22b-{}-{}-{}",
        label,
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("artifact root should be creatable");
    root
}

struct LiveHost {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl LiveHost {
    fn start(artifact_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_r2h-documentengine-host"))
            .env("R2H_DOCUMENTENGINE_ARTIFACT_ROOT", artifact_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("host executable should launch");
        let stdin = child.stdin.take().expect("host stdin should be available");
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .expect("host stdout should be available"),
        );
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, request: &Value) -> Value {
        serde_json::to_writer(&mut self.stdin, request).expect("request should serialize");
        self.stdin
            .write_all(b"\n")
            .expect("request newline should write");
        self.stdin.flush().expect("request should flush");
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("host response should read");
        assert!(!line.is_empty(), "host should return one response frame");
        serde_json::from_str(line.trim_end()).expect("host response should be JSON")
    }

    fn finish(mut self) {
        drop(self.stdin);
        let status = self.child.wait().expect("host should exit after stdin EOF");
        assert!(status.success(), "host should exit cleanly");
    }
}

fn run_real_host(requests: &[Value], artifact_root: &Path) -> Vec<Value> {
    let mut host = LiveHost::start(artifact_root);
    let responses = requests
        .iter()
        .map(|request| host.request(request))
        .collect();
    host.finish();
    responses
}

#[test]
fn real_host_sequential_document_flow_uses_core_and_contained_artifact() {
    let artifact_root = test_artifact_root("flow");
    let fixture = fixture_path();
    let outside_destination = artifact_root
        .parent()
        .expect("artifact root should have a parent")
        .join("e2-2b-arbitrary-destination.rgba");
    let _ = fs::remove_file(&outside_destination);

    let mut host = LiveHost::start(&artifact_root);
    let hello = host.request(&request("hello", "HELLO", json!({})));
    assert_eq!(hello["result"]["capabilities"]["nativeText"], true);
    assert_eq!(hello["result"]["capabilities"]["pageRender"], true);
    assert!(hello["result"]["capabilities"].get("ocr").is_none());
    assert_eq!(hello["result"]["capabilities"]["vectorRead"], true);
    assert!(hello["result"]["capabilities"]
        .get("imageExtraction")
        .is_none());

    let opened = host.request(&request(
        "open",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let session_id = opened["result"]["documentSessionId"]
        .as_str()
        .expect("OPEN_DOCUMENT should return a host session token")
        .to_string();
    assert!(!session_id.starts_with("doc-session-"));
    assert!(!opened
        .to_string()
        .contains(fixture.to_string_lossy().as_ref()));

    let info = host.request(&request(
        "info",
        "GET_DOCUMENT_INFO",
        json!({ "documentSessionId": session_id }),
    ));
    let page = host.request(&request(
        "page",
        "GET_PAGE_INFO",
        json!({ "documentSessionId": session_id, "pageIndex": 0 }),
    ));
    let bad_page = host.request(&request(
        "bad-page",
        "GET_PAGE_INFO",
        json!({ "documentSessionId": session_id, "pageIndex": 1 }),
    ));
    let negative_page = host.request(&request(
        "negative-page",
        "GET_PAGE_INFO",
        json!({ "documentSessionId": session_id, "pageIndex": -1 }),
    ));
    let text = host.request(&request(
        "text",
        "EXTRACT_NATIVE_TEXT",
        json!({ "documentSessionId": session_id, "pageIndex": 0 }),
    ));
    let render = host.request(&request(
        "render",
        "RENDER_PAGE",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "scale": 1.0,
            "devicePixelRatio": 1.0,
            "pixelFormat": "rgba8"
        }),
    ));
    let rejected_destination = host.request(&request(
        "rejected-destination",
        "RENDER_PAGE",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "scale": 1.0,
            "devicePixelRatio": 1.0,
            "pixelFormat": "rgba8",
            "destination": outside_destination
        }),
    ));
    let close = host.request(&request(
        "close",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_id }),
    ));
    let health = host.request(&request("health-after-close", "HEALTH", json!({})));
    host.finish();

    assert_eq!(info["result"]["pageCount"], 1);
    assert!(!info
        .to_string()
        .contains(fixture.to_string_lossy().as_ref()));

    assert_eq!(page["result"]["pageIndex"], 0);
    assert_eq!(page["result"]["widthPt"], 792.0);
    assert_eq!(page["result"]["heightPt"], 612.0);
    assert_eq!(page["result"]["rotation"], 90);
    assert_eq!(page["result"]["hasText"], true);
    assert_eq!(bad_page["error"]["code"], "PAGE_NOT_FOUND");
    assert_eq!(negative_page["error"]["code"], "PAGE_NOT_FOUND");

    assert_eq!(text["result"]["pageIndex"], 0);
    assert!(text["result"]["fullText"]
        .as_str()
        .expect("native text should be a string")
        .contains("E2.2b native text fixture"));
    assert_eq!(
        text["result"]["coordinateConvention"],
        "pdf_points_origin_bottom_left"
    );
    assert!(!text["result"]["spans"].as_array().unwrap().is_empty());

    let render_result = &render["result"];
    let artifact_token = render_result["artifactToken"]
        .as_str()
        .expect("render should return a relative artifact token");
    assert!(!Path::new(artifact_token).is_absolute());
    assert!(!artifact_token.contains(".."));
    assert!(render_result["pixelWidth"].as_u64().unwrap() > 0);
    assert!(render_result["pixelHeight"].as_u64().unwrap() > 0);
    assert!(render_result["byteLength"].as_u64().unwrap() > 0);
    assert_eq!(render_result["pixelFormat"], "rgba8");
    assert_eq!(render_result["pageWidthPt"], 792.0);
    assert_eq!(render_result["pageHeightPt"], 612.0);
    assert_eq!(render_result["rotation"], 90);
    let artifact_path = artifact_root.join(artifact_token);
    let artifact_bytes = fs::read(&artifact_path).expect("render artifact should exist");
    assert_eq!(
        artifact_bytes.len() as u64,
        render_result["byteLength"].as_u64().unwrap()
    );
    let hash = Sha256::digest(&artifact_bytes);
    assert_eq!(format!("{hash:x}"), render_result["sha256"]);
    assert!(artifact_path
        .canonicalize()
        .unwrap()
        .starts_with(artifact_root.canonicalize().unwrap()));
    assert!(!outside_destination.exists());
    assert_eq!(
        rejected_destination["error"]["code"],
        "UNSUPPORTED_OPERATION"
    );

    assert_eq!(close["result"]["closed"], true);
    assert_eq!(health["result"]["status"], "READY");
    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn real_host_document_errors_are_typed_and_recoverable() {
    let artifact_root = test_artifact_root("errors");
    let missing = artifact_root.join("missing.pdf");
    let invalid = artifact_root.join("invalid.pdf");
    fs::write(&invalid, b"not a PDF").expect("invalid fixture should write");

    let responses = run_real_host(
        &[
            request(
                "missing",
                "OPEN_DOCUMENT",
                json!({ "path": missing, "recoverIfDamaged": false }),
            ),
            request(
                "invalid",
                "OPEN_DOCUMENT",
                json!({ "path": invalid, "recoverIfDamaged": false }),
            ),
            request(
                "random-session",
                "GET_DOCUMENT_INFO",
                json!({ "documentSessionId": "r2h-session-does-not-exist" }),
            ),
            request(
                "bad-page",
                "GET_PAGE_INFO",
                json!({ "documentSessionId": "r2h-session-does-not-exist", "pageIndex": 1 }),
            ),
            request("unsupported", "RENDER_PAGE_TO_FILE", json!({})),
            request("health", "HEALTH", json!({})),
        ],
        &artifact_root,
    );

    assert_eq!(responses[0]["error"]["code"], "DOCUMENT_NOT_FOUND");
    assert_eq!(responses[1]["error"]["code"], "INVALID_DOCUMENT");
    assert_eq!(responses[2]["error"]["code"], "SESSION_NOT_FOUND");
    assert_eq!(responses[3]["error"]["code"], "SESSION_NOT_FOUND");
    assert_eq!(responses[4]["error"]["code"], "UNSUPPORTED_METHOD");
    assert_eq!(responses[5]["result"]["status"], "READY");
    assert!(!responses.iter().any(|response| response
        .to_string()
        .contains(missing.to_string_lossy().as_ref())));
    assert!(!responses.iter().any(|response| response
        .to_string()
        .contains(invalid.to_string_lossy().as_ref())));

    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn real_host_supports_two_opaque_sessions_and_closed_session_state() {
    let artifact_root = test_artifact_root("sessions");
    let fixture = fixture_path();
    let mut host = LiveHost::start(&artifact_root);
    let opened_a = host.request(&request(
        "open-a",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let opened_b = host.request(&request(
        "open-b",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let session_a = opened_a["result"]["documentSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let session_b = opened_b["result"]["documentSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(session_a, session_b);

    let close_a = host.request(&request(
        "close-a",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_a }),
    ));
    let close_a_again = host.request(&request(
        "close-a-again",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_a }),
    ));
    let use_closed_a = host.request(&request(
        "use-closed-a",
        "GET_DOCUMENT_INFO",
        json!({ "documentSessionId": session_a }),
    ));
    let info_b = host.request(&request(
        "info-b",
        "GET_DOCUMENT_INFO",
        json!({ "documentSessionId": session_b }),
    ));
    let close_b = host.request(&request(
        "close-b",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_b }),
    ));
    host.finish();

    assert_eq!(close_a["result"]["closed"], true);
    assert_eq!(close_a_again["error"]["code"], "SESSION_CLOSED");
    assert_eq!(use_closed_a["error"]["code"], "SESSION_CLOSED");
    assert_eq!(info_b["result"]["pageCount"], 1);
    assert_eq!(close_b["result"]["closed"], true);
    let _ = fs::remove_dir_all(artifact_root);
}

fn write_native_vector_fixture(root: &Path, rotation: i32) -> PathBuf {
    let path = root.join(format!("native-vector-{rotation}.pdf"));
    let content = concat!(
        "q\n",
        "1 0 0 RG\n2 w\n40 30 m\n260 30 l\nS\n",
        "0 1 0 rg\n100 80 60 40 re\nf\n",
        "0 0 1 RG\n1 w\n100 100 m\n120 160 180 0 200 100 c\nh\nS\n",
        "Q\n"
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Rotate {rotation} /Resources << /ProcSet [/PDF] >> /Contents 4 0 R >>"
        ),
        format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len() + 1);
    offsets.push(0usize);
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    fs::write(&path, pdf).expect("native vector fixture should write");
    path
}

#[test]
fn real_host_native_vector_read_preserves_geometry_metadata_and_order() {
    let artifact_root = test_artifact_root("native-vector");
    let fixture = write_native_vector_fixture(&artifact_root, 0);
    let mut host = LiveHost::start(&artifact_root);
    let opened = host.request(&request(
        "open-vector",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let session_id = opened["result"]["documentSessionId"]
        .as_str()
        .expect("vector fixture should open")
        .to_string();

    let first = host.request(&request(
        "vectors-1",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    let second = host.request(&request(
        "vectors-2",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    let close = host.request(&request(
        "close-vector",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_id }),
    ));
    let after_close = host.request(&request(
        "vectors-after-close",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    host.finish();

    assert!(first.get("error").is_none(), "vector read failed: {first}");
    assert_eq!(first["result"], second["result"]);
    let result = &first["result"];
    assert_eq!(result["pageIndex"], 0);
    assert_eq!(result["coordinateSpace"], "PDF_PAGE");
    assert_eq!(result["pageGeometry"]["widthPt"], 300.0);
    assert_eq!(result["pageGeometry"]["heightPt"], 200.0);
    assert_eq!(result["pageGeometry"]["rotation"], 0);
    assert_eq!(
        result["profile"]["profileId"],
        "documentengine-native-vector-read-v1"
    );
    assert_eq!(result["engineIdentity"]["product"], "R2H-PDF");
    assert!(!result
        .to_string()
        .contains(fixture.to_string_lossy().as_ref()));
    assert!(!result.to_string().to_ascii_lowercase().contains("pid"));
    assert!(!result.to_string().to_ascii_lowercase().contains("session"));

    let paths = result["paths"]
        .as_array()
        .expect("paths should be an array");
    assert!(!paths.is_empty(), "native vector fixture should have paths");
    println!(
        "E6B1_OBSERVED native_fixture_paths={} response_bytes={}",
        paths.len(),
        first["result"].to_string().len()
    );
    let mut command_kinds = Vec::new();
    for (expected_order, path) in paths.iter().enumerate() {
        assert_eq!(path["sourceOrder"], expected_order);
        let bounds = &path["bounds"];
        for key in ["x", "y", "width", "height"] {
            assert!(bounds[key].as_f64().unwrap().is_finite());
        }
        assert!(bounds["x"].as_f64().unwrap() >= 0.0);
        assert!(bounds["y"].as_f64().unwrap() >= 0.0);
        assert!(bounds["x"].as_f64().unwrap() + bounds["width"].as_f64().unwrap() <= 300.0);
        assert!(bounds["y"].as_f64().unwrap() + bounds["height"].as_f64().unwrap() <= 200.0);
        for command in path["commands"].as_array().unwrap() {
            let kind = command["type"].as_str().unwrap();
            command_kinds.push(kind.to_string());
            for value in command.as_object().unwrap().values() {
                if let Some(number) = value.as_f64() {
                    assert!(number.is_finite());
                    assert!((0.0..=300.0).contains(&number));
                }
            }
        }
    }
    assert!(command_kinds.iter().any(|kind| kind == "MOVE_TO"));
    assert!(command_kinds.iter().any(|kind| kind == "LINE_TO"));
    assert!(command_kinds.iter().any(|kind| kind == "CURVE_TO"));
    assert!(command_kinds.iter().any(|kind| kind == "CLOSE_PATH"));
    assert!(command_kinds.iter().any(|kind| kind == "RECT"));
    assert!(paths.iter().any(|path| path["stroke"]["present"] == true));
    assert!(paths.iter().any(|path| path["fill"]["present"] == true));
    assert_eq!(close["result"]["closed"], true);
    assert_eq!(after_close["error"]["code"], "SESSION_CLOSED");

    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn native_vector_read_allows_empty_native_path_pages() {
    let artifact_root = test_artifact_root("native-vector-empty");
    let fixture = fixture_path();
    let mut host = LiveHost::start(&artifact_root);
    let opened = host.request(&request(
        "open-empty-vector",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let session_id = opened["result"]["documentSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let vector = host.request(&request(
        "empty-vectors",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    let close = host.request(&request(
        "close-empty-vector",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_id }),
    ));
    host.finish();

    assert!(
        vector.get("error").is_none(),
        "empty vector read failed: {vector}"
    );
    assert!(vector["result"]["paths"].is_array());
    println!(
        "E6B1_OBSERVED existing_text_fixture_paths={} response_bytes={}",
        vector["result"]["paths"].as_array().unwrap().len(),
        vector["result"].to_string().len()
    );
    assert_eq!(close["result"]["closed"], true);
    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn native_vector_read_validates_profile_page_and_session() {
    let artifact_root = test_artifact_root("native-vector-validation");
    let fixture = write_native_vector_fixture(&artifact_root, 0);
    let mut host = LiveHost::start(&artifact_root);
    let opened = host.request(&request(
        "open-vector-validation",
        "OPEN_DOCUMENT",
        json!({ "path": fixture, "recoverIfDamaged": false }),
    ));
    let session_id = opened["result"]["documentSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let bad_profile = host.request(&request(
        "bad-profile",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 0,
            "profileVersion": "not-supported"
        }),
    ));
    let bad_page = host.request(&request(
        "bad-vector-page",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "documentSessionId": session_id,
            "pageIndex": 1,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    let missing_session = host.request(&request(
        "missing-vector-session",
        "EXTRACT_NATIVE_VECTORS",
        json!({
            "pageIndex": 0,
            "profileVersion": "documentengine-native-vector-read-v1"
        }),
    ));
    let close = host.request(&request(
        "close-vector-validation",
        "CLOSE_DOCUMENT",
        json!({ "documentSessionId": session_id }),
    ));
    host.finish();

    assert_eq!(bad_profile["error"]["code"], "VECTOR_INPUT_INVALID");
    assert_eq!(bad_page["error"]["code"], "PAGE_NOT_FOUND");
    assert_eq!(missing_session["error"]["code"], "VECTOR_INPUT_INVALID");
    assert_eq!(close["result"]["closed"], true);
    let _ = fs::remove_dir_all(artifact_root);
}

#[test]
fn native_vector_read_reports_page_rotation_without_display_coordinates() {
    let artifact_root = test_artifact_root("native-vector-rotation");
    let mut host = LiveHost::start(&artifact_root);
    for rotation in [0, 90, 180, 270] {
        let fixture = write_native_vector_fixture(&artifact_root, rotation);
        let opened = host.request(&request(
            &format!("open-rotation-{rotation}"),
            "OPEN_DOCUMENT",
            json!({ "path": fixture, "recoverIfDamaged": false }),
        ));
        let session_id = opened["result"]["documentSessionId"]
            .as_str()
            .unwrap()
            .to_string();
        let vector = host.request(&request(
            &format!("vector-rotation-{rotation}"),
            "EXTRACT_NATIVE_VECTORS",
            json!({
                "documentSessionId": session_id,
                "pageIndex": 0,
                "profileVersion": "documentengine-native-vector-read-v1"
            }),
        ));
        assert!(
            vector.get("error").is_none(),
            "rotation {rotation} failed: {vector}"
        );
        assert_eq!(vector["result"]["coordinateSpace"], "PDF_PAGE");
        assert_eq!(vector["result"]["pageGeometry"]["rotation"], rotation);
        let close = host.request(&request(
            &format!("close-rotation-{rotation}"),
            "CLOSE_DOCUMENT",
            json!({ "documentSessionId": session_id }),
        ));
        assert_eq!(close["result"]["closed"], true);
    }
    host.finish();
    let _ = fs::remove_dir_all(artifact_root);
}
