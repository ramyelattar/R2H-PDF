use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};

const WINDOW_LABEL: &str = "bentopdf-tools";
const BUNDLE_VERSION: &str = "2.8.6-21c924a3";
const BASE_PATH: &str = "/bentopdf/";
const R2H_THEME_CSS_PATH: &str = "/bentopdf/r2h/r2h-theme.css";
const R2H_SHELL_JS_PATH: &str = "/bentopdf/r2h/r2h-shell.js";
const R2H_THEME_CSS: &str = include_str!("../bentopdf-overlay/r2h-theme.css");
const R2H_SHELL_JS: &str = include_str!("../bentopdf-overlay/r2h-shell.js");
const R2H_THEME_MARKER: &str = "data-r2h-theme-bridge";
const R2H_SHELL_MARKER: &str = "data-r2h-shell-bridge";

const REQUIRED_BUNDLE_FILES: &[&str] = &[
    "index.html",
    "site.webmanifest",
    "merge-pdf.html",
    "compress-pdf.html",
    "ocr-pdf.html",
    "edit-pdf.html",
    "wasm/pymupdf/dist/index.js",
    "wasm/gs/gs.js",
    "wasm/gs/gs.wasm",
    "wasm/cpdf/coherentpdf.browser.min.js",
    "wasm/ocr/worker.min.js",
    "wasm/ocr/lang-data/ara.traineddata.gz",
    "wasm/ocr/lang-data/eng.traineddata.gz",
];

const CONTENT_SECURITY_POLICY: &str = concat!(
    "default-src 'self' data: blob:; ",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob:; ",
    "style-src 'self' 'unsafe-inline'; ",
    "img-src 'self' data: blob:; ",
    "font-src 'self' data: blob:; ",
    "connect-src 'self' data: blob:; ",
    "worker-src 'self' blob:; ",
    "child-src 'self' blob:; ",
    "frame-src 'self' blob:; ",
    "media-src 'self' data: blob:; ",
    "object-src 'self' blob:; ",
    "form-action 'self'; ",
    "base-uri 'self'"
);

#[derive(Debug, Clone)]
struct BundleLocation {
    path: PathBuf,
    source: String,
}

#[derive(Debug, Clone)]
struct ServerDescriptor {
    address: SocketAddr,
    root: PathBuf,
    source: String,
}

struct StaticServer {
    descriptor: ServerDescriptor,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StaticServer {
    fn start(bundle: BundleLocation) -> Result<Self, String> {
        validate_bundle_root(&bundle.path)?;

        let canonical_root = bundle.path.canonicalize().map_err(|error| {
            format!(
                "Unable to canonicalize BentoPDF bundle directory {}: {error}",
                bundle.path.display()
            )
        })?;

        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| format!("Unable to bind the BentoPDF local server: {error}"))?;

        listener
            .set_nonblocking(true)
            .map_err(|error| format!("Unable to configure the BentoPDF local server: {error}"))?;

        let address = listener.local_addr().map_err(|error| {
            format!("Unable to read the BentoPDF local server address: {error}")
        })?;

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_root = canonical_root.clone();
        let expected_host = format!("127.0.0.1:{}", address.port());

        let thread = thread::Builder::new()
            .name("r2h-bentopdf-static-server".to_string())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _peer)) => {
                            let root = thread_root.clone();
                            let host = expected_host.clone();

                            let _ = thread::Builder::new()
                                .name("r2h-bentopdf-static-client".to_string())
                                .spawn(move || {
                                    if let Err(error) = handle_connection(stream, &root, &host) {
                                        eprintln!("BentoPDF static request failed: {error}");
                                    }
                                });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(error) => {
                            eprintln!("BentoPDF static server accept failed: {error}");
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
            })
            .map_err(|error| {
                format!("Unable to start the BentoPDF local server thread: {error}")
            })?;

        Ok(Self {
            descriptor: ServerDescriptor {
                address,
                root: canonical_root,
                source: bundle.source,
            },
            stop,
            thread: Some(thread),
        })
    }

    fn descriptor(&self) -> ServerDescriptor {
        self.descriptor.clone()
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);

        let _ = TcpStream::connect_timeout(&self.descriptor.address, Duration::from_millis(100));

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Default)]
pub struct BentoPdfRuntimeState {
    server: Mutex<Option<StaticServer>>,
}

impl BentoPdfRuntimeState {
    fn ensure_started(&self) -> Result<ServerDescriptor, String> {
        let mut guard = self
            .server
            .lock()
            .map_err(|_| "BentoPDF runtime state lock was poisoned.".to_string())?;

        if let Some(server) = guard.as_ref() {
            if server
                .thread
                .as_ref()
                .map(|thread| !thread.is_finished())
                .unwrap_or(false)
            {
                return Ok(server.descriptor());
            }
        }

        if let Some(mut stale) = guard.take() {
            stale.stop();
        }

        let bundle = discover_bundle_root()?;
        let server = StaticServer::start(bundle)?;
        let descriptor = server.descriptor();
        *guard = Some(server);

        Ok(descriptor)
    }

    fn running_descriptor(&self) -> Result<Option<ServerDescriptor>, String> {
        let guard = self
            .server
            .lock()
            .map_err(|_| "BentoPDF runtime state lock was poisoned.".to_string())?;

        Ok(guard.as_ref().and_then(|server| {
            server
                .thread
                .as_ref()
                .filter(|thread| !thread.is_finished())
                .map(|_| server.descriptor())
        }))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BentoPdfStatus {
    installed: bool,
    running: bool,
    bundle_path: Option<String>,
    bundle_source: Option<String>,
    origin: Option<String>,
    checked_paths: Vec<String>,
    validation_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BentoPdfOpenResult {
    reused_existing_window: bool,
    window_label: String,
    url: String,
    bundle_path: String,
    bundle_source: String,
}

#[tauri::command]
pub fn bentopdf_get_status(
    state: State<'_, BentoPdfRuntimeState>,
) -> Result<BentoPdfStatus, String> {
    let candidates = bundle_candidates();
    let checked_paths = candidates
        .iter()
        .map(|candidate| candidate.path.display().to_string())
        .collect::<Vec<_>>();

    let (discovered, validation_error) = match discover_bundle_root() {
        Ok(location) => (Some(location), None),
        Err(error) => (None, Some(error)),
    };

    let running = state.running_descriptor()?;

    Ok(BentoPdfStatus {
        installed: discovered.is_some(),
        running: running.is_some(),
        bundle_path: discovered
            .as_ref()
            .map(|location| location.path.display().to_string()),
        bundle_source: discovered.as_ref().map(|location| location.source.clone()),
        origin: running
            .as_ref()
            .map(|descriptor| format!("http://{}", descriptor.address)),
        checked_paths,
        validation_error,
    })
}

#[tauri::command]
pub async fn bentopdf_open(
    app: AppHandle,
    state: State<'_, BentoPdfRuntimeState>,
) -> Result<BentoPdfOpenResult, String> {
    let descriptor = state.ensure_started()?;
    let origin = format!("http://{}", descriptor.address);
    let target_url = format!("{origin}{BASE_PATH}index.html");

    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        window
            .show()
            .map_err(|error| format!("Unable to show the BentoPDF window: {error}"))?;

        let _ = window.unminimize();

        window
            .set_focus()
            .map_err(|error| format!("Unable to focus the BentoPDF window: {error}"))?;

        return Ok(BentoPdfOpenResult {
            reused_existing_window: true,
            window_label: WINDOW_LABEL.to_string(),
            url: target_url,
            bundle_path: descriptor.root.display().to_string(),
            bundle_source: descriptor.source,
        });
    }

    let webview_url = tauri::WebviewUrl::External(
        target_url
            .parse()
            .map_err(|error| format!("Invalid BentoPDF local URL: {error}"))?,
    );

    let allowed_port = descriptor.address.port();

    let window = tauri::WebviewWindowBuilder::new(&app, WINDOW_LABEL, webview_url)
        .title("R2H PDF — BentoPDF Tools")
        .inner_size(1320.0, 860.0)
        .min_inner_size(960.0, 640.0)
        .resizable(true)
        .maximizable(true)
        .minimizable(true)
        .closable(true)
        .center()
        .focused(true)
        .devtools(cfg!(debug_assertions))
        .on_navigation(move |url| match url.scheme() {
            "about" | "blob" | "data" => true,
            "http" => {
                url.host_str() == Some("127.0.0.1")
                    && url.port_or_known_default() == Some(allowed_port)
                    && url.path().starts_with(BASE_PATH)
            }
            _ => false,
        })
        .on_new_window(|_url, _features| tauri::webview::NewWindowResponse::Deny)
        .on_download(|_webview, _event| true)
        .build()
        .map_err(|error| format!("Unable to create the BentoPDF tools window: {error}"))?;

    window
        .set_focus()
        .map_err(|error| format!("Unable to focus the BentoPDF tools window: {error}"))?;

    Ok(BentoPdfOpenResult {
        reused_existing_window: false,
        window_label: WINDOW_LABEL.to_string(),
        url: target_url,
        bundle_path: descriptor.root.display().to_string(),
        bundle_source: descriptor.source,
    })
}

fn bundle_candidates() -> Vec<BundleLocation> {
    let mut candidates = Vec::new();

    if let Some(value) = std::env::var_os("R2H_BENTOPDF_BUNDLE_DIR") {
        push_candidate(
            &mut candidates,
            PathBuf::from(value),
            "R2H_BENTOPDF_BUNDLE_DIR",
        );
    }

    if let Ok(executable) = std::env::current_exe() {
        if let Some(executable_dir) = executable.parent() {
            push_candidate(
                &mut candidates,
                executable_dir
                    .join("bentopdf-offline-runtime")
                    .join(BUNDLE_VERSION)
                    .join("bentopdf"),
                "executable-sibling",
            );

            if let Some(parent) = executable_dir.parent() {
                push_candidate(
                    &mut candidates,
                    parent
                        .join("bentopdf-offline-runtime")
                        .join(BUNDLE_VERSION)
                        .join("bentopdf"),
                    "executable-parent-sibling",
                );

                push_candidate(
                    &mut candidates,
                    parent
                        .join("R2H-PDF-BentoPDF")
                        .join(BUNDLE_VERSION)
                        .join("bentopdf"),
                    "separate-install-root",
                );
            }
        }
    }

    if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
        push_candidate(
            &mut candidates,
            PathBuf::from(program_data)
                .join("R2H-PDF")
                .join("BentoPDF")
                .join(BUNDLE_VERSION)
                .join("bentopdf"),
            "program-data",
        );
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_candidate(
            &mut candidates,
            current_dir
                .join("generated")
                .join("bentopdf")
                .join(BUNDLE_VERSION),
            "working-directory-phase1-build",
        );
        push_candidate(
            &mut candidates,
            current_dir
                .join("generated")
                .join("bentopdf-offline")
                .join(BUNDLE_VERSION)
                .join("bentopdf"),
            "working-directory-development",
        );
    }

    push_candidate(
        &mut candidates,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("generated")
            .join("bentopdf-offline")
            .join(BUNDLE_VERSION)
            .join("bentopdf"),
        "cargo-manifest-development",
    );

    candidates
}

fn push_candidate(candidates: &mut Vec<BundleLocation>, path: PathBuf, source: &str) {
    let normalized = if path.join("index.html").is_file() {
        path
    } else if path.join("bentopdf").join("index.html").is_file() {
        path.join("bentopdf")
    } else {
        path
    };

    if candidates
        .iter()
        .any(|existing| existing.path == normalized)
    {
        return;
    }

    candidates.push(BundleLocation {
        path: normalized,
        source: source.to_string(),
    });
}

fn discover_bundle_root() -> Result<BundleLocation, String> {
    let candidates = bundle_candidates();

    for candidate in &candidates {
        if validate_bundle_root(&candidate.path).is_ok() {
            return Ok(candidate.clone());
        }
    }

    let checked = candidates
        .iter()
        .map(|candidate| format!("  - {} ({})", candidate.path.display(), candidate.source))
        .collect::<Vec<_>>()
        .join("\n");

    Err(format!(
        "The local BentoPDF package is not installed or is incomplete.\n\
         Install the BentoPDF offline package or set R2H_BENTOPDF_BUNDLE_DIR.\n\
         Checked locations:\n{checked}"
    ))
}

fn validate_bundle_root(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!(
            "BentoPDF bundle directory does not exist: {}",
            root.display()
        ));
    }

    let missing = REQUIRED_BUNDLE_FILES
        .iter()
        .filter(|relative| !root.join(relative).is_file())
        .copied()
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        return Err(format!(
            "BentoPDF bundle is incomplete at {}. Missing: {}",
            root.display(),
            missing.join(", ")
        ));
    }

    Ok(())
}

fn configure_client_stream(stream: &TcpStream) -> Result<(), String> {
    stream.set_nonblocking(false).map_err(|error| {
        format!("Unable to configure the accepted BentoPDF connection for blocking I/O: {error}")
    })?;

    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| format!("Unable to set request timeout: {error}"))?;

    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|error| format!("Unable to set response timeout: {error}"))?;

    Ok(())
}

fn request_path_only(target: &str) -> &str {
    target.split(['?', '#']).next().unwrap_or(target)
}

fn embedded_r2h_asset(target: &str) -> Option<(&'static str, &'static [u8])> {
    match request_path_only(target) {
        R2H_THEME_CSS_PATH => Some(("text/css; charset=utf-8", R2H_THEME_CSS.as_bytes())),
        R2H_SHELL_JS_PATH => Some((
            "application/javascript; charset=utf-8",
            R2H_SHELL_JS.as_bytes(),
        )),
        _ => None,
    }
}

fn inject_r2h_shell(html: &str) -> String {
    if html.contains(R2H_THEME_MARKER) && html.contains(R2H_SHELL_MARKER) {
        return html.to_string();
    }

    let bridge_markup = format!(
        concat!(
            "<link rel=\"stylesheet\" href=\"{}\" {}=\"true\" />",
            "<script defer src=\"{}\" {}=\"true\"></script>"
        ),
        R2H_THEME_CSS_PATH, R2H_THEME_MARKER, R2H_SHELL_JS_PATH, R2H_SHELL_MARKER,
    );

    if let Some(head_end) = html.rfind("</head>") {
        let mut output = String::with_capacity(html.len() + bridge_markup.len());
        output.push_str(&html[..head_end]);
        output.push_str(&bridge_markup);
        output.push_str(&html[head_end..]);
        output
    } else {
        format!("{bridge_markup}{html}")
    }
}

fn write_memory_response(
    stream: &mut TcpStream,
    method: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    write_response_headers(
        stream,
        200,
        "OK",
        content_type,
        body.len() as u64,
        None,
        None,
    )?;

    if method != "HEAD" {
        stream
            .write_all(body)
            .map_err(|error| format!("Unable to write in-memory response body: {error}"))?;
    }

    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    root: &Path,
    expected_host: &str,
) -> Result<(), String> {
    configure_client_stream(&stream)?;

    let request = read_request_headers(&mut stream)?;
    let request_text = String::from_utf8(request)
        .map_err(|_| "HTTP request headers were not valid UTF-8.".to_string())?;

    let mut lines = request_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "HTTP request line was missing.".to_string())?;

    let request_parts = request_line.split_whitespace().collect::<Vec<_>>();

    if request_parts.len() != 3 {
        return send_error(&mut stream, 400, "Bad Request", "Malformed request line.");
    }

    let method = request_parts[0];
    let target = request_parts[1];

    let mut headers = HashMap::new();

    for line in lines {
        if line.is_empty() {
            break;
        }

        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    if headers.get("host").map(String::as_str) != Some(expected_host) {
        return send_error(
            &mut stream,
            421,
            "Misdirected Request",
            "Unexpected Host header.",
        );
    }

    if method == "OPTIONS" {
        return write_response_headers(
            &mut stream,
            204,
            "No Content",
            "text/plain; charset=utf-8",
            0,
            None,
            None,
        );
    }

    if method != "GET" && method != "HEAD" {
        return send_error(
            &mut stream,
            405,
            "Method Not Allowed",
            "Only GET, HEAD, and OPTIONS are supported.",
        );
    }

    if target == "/bentopdf" {
        let response = concat!(
            "HTTP/1.1 308 Permanent Redirect\r\n",
            "Location: /bentopdf/\r\n",
            "Content-Length: 0\r\n",
            "Connection: close\r\n",
            "Cache-Control: no-store\r\n",
            "\r\n"
        );

        stream
            .write_all(response.as_bytes())
            .map_err(|error| format!("Unable to write redirect response: {error}"))?;

        return Ok(());
    }

    if let Some((content_type, body)) = embedded_r2h_asset(target) {
        return write_memory_response(&mut stream, method, content_type, body);
    }

    let file_path = match resolve_request_path(root, target) {
        Ok(path) => path,
        Err(message) => {
            return send_error(&mut stream, 404, "Not Found", &message);
        }
    };

    let metadata = file_path
        .metadata()
        .map_err(|error| format!("Unable to read {} metadata: {error}", file_path.display()))?;

    let file_length = metadata.len();
    let content_type = content_type_for(&file_path);

    if content_type == "text/html; charset=utf-8" {
        let html = fs::read_to_string(&file_path).map_err(|error| {
            format!(
                "Unable to read {} as UTF-8 HTML: {error}",
                file_path.display()
            )
        })?;
        let themed_html = inject_r2h_shell(&html);

        return write_memory_response(&mut stream, method, content_type, themed_html.as_bytes());
    }

    let range = match headers.get("range") {
        Some(value) => match parse_single_range(value, file_length) {
            Ok(value) => Some(value),
            Err(()) => {
                let response = format!(
                    concat!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\n",
                        "Content-Range: bytes */{}\r\n",
                        "Content-Length: 0\r\n",
                        "Connection: close\r\n",
                        "\r\n"
                    ),
                    file_length
                );

                stream
                    .write_all(response.as_bytes())
                    .map_err(|error| format!("Unable to write range error: {error}"))?;

                return Ok(());
            }
        },
        None => None,
    };

    let (status, reason, start, end) = match range {
        Some((start, end)) => (206, "Partial Content", start, end),
        None if file_length > 0 => (200, "OK", 0, file_length - 1),
        None => (200, "OK", 0, 0),
    };

    let response_length = if file_length == 0 { 0 } else { end - start + 1 };

    write_response_headers(
        &mut stream,
        status,
        reason,
        content_type,
        response_length,
        range.map(|_| (start, end, file_length)),
        Some(file_last_modified(&metadata)),
    )?;

    if method == "HEAD" || response_length == 0 {
        return Ok(());
    }

    let mut file = File::open(&file_path)
        .map_err(|error| format!("Unable to open {}: {error}", file_path.display()))?;

    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("Unable to seek {}: {error}", file_path.display()))?;

    let mut remaining = response_length;
    let mut buffer = [0_u8; 64 * 1024];

    while remaining > 0 {
        let read_length = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| "Response chunk length overflowed usize.".to_string())?;

        let count = file
            .read(&mut buffer[..read_length])
            .map_err(|error| format!("Unable to read {}: {error}", file_path.display()))?;

        if count == 0 {
            return Err(format!(
                "Unexpected end of file while serving {}.",
                file_path.display()
            ));
        }

        stream
            .write_all(&buffer[..count])
            .map_err(|error| format!("Unable to write response body: {error}"))?;

        remaining -= count as u64;
    }

    Ok(())
}

fn read_request_headers(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    const MAX_HEADER_BYTES: usize = 64 * 1024;
    let mut request = Vec::with_capacity(4096);
    let mut buffer = [0_u8; 4096];

    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("Unable to read HTTP request: {error}"))?;

        if count == 0 {
            return Err("HTTP client closed before sending complete headers.".to_string());
        }

        request.extend_from_slice(&buffer[..count]);

        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(request);
        }

        if request.len() > MAX_HEADER_BYTES {
            return Err("HTTP request headers exceeded 64 KiB.".to_string());
        }
    }
}

fn resolve_request_path(root: &Path, target: &str) -> Result<PathBuf, String> {
    let path_only = target
        .split(['?', '#'])
        .next()
        .ok_or_else(|| "Request target was empty.".to_string())?;

    let relative = path_only
        .strip_prefix(BASE_PATH)
        .ok_or_else(|| format!("Request path is outside {BASE_PATH}: {path_only}"))?;

    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };

    let decoded = percent_decode(relative)?;

    if decoded.contains('\\') || decoded.contains('\0') {
        return Err("Request path contained a forbidden separator or null byte.".to_string());
    }

    let decoded_path = Path::new(&decoded);

    for component in decoded_path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err("Request path contained traversal or an absolute component.".to_string())
            }
        }
    }

    let mut candidate = root.join(decoded_path);

    if candidate.is_dir() {
        candidate = candidate.join("index.html");
    }

    let canonical = candidate
        .canonicalize()
        .map_err(|_| format!("Requested BentoPDF resource was not found: {path_only}"))?;

    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err("Resolved request escaped the BentoPDF bundle root.".to_string());
    }

    Ok(canonical)
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err("Request path contained an incomplete percent escape.".to_string());
                }

                let high = decode_hex(bytes[index + 1]).ok_or_else(|| {
                    "Request path contained an invalid percent escape.".to_string()
                })?;
                let low = decode_hex(bytes[index + 2]).ok_or_else(|| {
                    "Request path contained an invalid percent escape.".to_string()
                })?;

                output.push((high << 4) | low);
                index += 3;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(output).map_err(|_| "Request path was not valid UTF-8.".to_string())
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_single_range(value: &str, length: u64) -> Result<(u64, u64), ()> {
    if length == 0 {
        return Err(());
    }

    let range = value.strip_prefix("bytes=").ok_or(())?;

    if range.contains(',') {
        return Err(());
    }

    let (start_text, end_text) = range.split_once('-').ok_or(())?;

    if start_text.is_empty() {
        let suffix = end_text.parse::<u64>().map_err(|_| ())?;

        if suffix == 0 {
            return Err(());
        }

        let start = length.saturating_sub(suffix);
        return Ok((start, length - 1));
    }

    let start = start_text.parse::<u64>().map_err(|_| ())?;

    if start >= length {
        return Err(());
    }

    let end = if end_text.is_empty() {
        length - 1
    } else {
        end_text.parse::<u64>().map_err(|_| ())?.min(length - 1)
    };

    if end < start {
        return Err(());
    }

    Ok((start, end))
}

fn write_response_headers(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    content_length: u64,
    content_range: Option<(u64, u64, u64)>,
    last_modified: Option<u64>,
) -> Result<(), String> {
    let mut response = format!(
        concat!(
            "HTTP/1.1 {} {}\r\n",
            "Content-Type: {}\r\n",
            "Content-Length: {}\r\n",
            "Connection: close\r\n",
            "Accept-Ranges: bytes\r\n",
            "Cache-Control: no-store\r\n",
            "Cross-Origin-Opener-Policy: same-origin\r\n",
            "Cross-Origin-Embedder-Policy: require-corp\r\n",
            "Cross-Origin-Resource-Policy: same-origin\r\n",
            "Content-Security-Policy: {}\r\n",
            "Permissions-Policy: camera=(), microphone=(), geolocation=(), usb=(), serial=()\r\n",
            "Referrer-Policy: no-referrer\r\n",
            "X-Content-Type-Options: nosniff\r\n"
        ),
        status, reason, content_type, content_length, CONTENT_SECURITY_POLICY,
    );

    if let Some((start, end, total)) = content_range {
        response.push_str(&format!("Content-Range: bytes {start}-{end}/{total}\r\n"));
    }

    if let Some(modified) = last_modified {
        response.push_str(&format!("X-R2H-File-Modified-Unix: {modified}\r\n"));
    }

    response.push_str("\r\n");

    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("Unable to write HTTP response headers: {error}"))
}

fn send_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    message: &str,
) -> Result<(), String> {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{status} {reason}</title></head>\
         <body><h1>{status} {reason}</h1><p>{}</p></body></html>",
        html_escape(message)
    );

    write_response_headers(
        stream,
        status,
        reason,
        "text/html; charset=utf-8",
        body.len() as u64,
        None,
        None,
    )?;

    stream
        .write_all(body.as_bytes())
        .map_err(|error| format!("Unable to write HTTP error body: {error}"))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn file_last_modified(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "pdf" => "application/pdf",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "gz" => "application/gzip",
        "br" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        net::TcpStream,
    };

    fn temp_bundle() -> PathBuf {
        let unique = format!(
            "r2h-bentopdf-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let root = std::env::temp_dir().join(unique);

        for relative in REQUIRED_BUNDLE_FILES {
            let path = root.join(relative);

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            let content = if relative.ends_with(".html") {
                b"<!doctype html><html><body>ok</body></html>".as_slice()
            } else if *relative == "site.webmanifest" {
                br#"{"start_url":"/bentopdf/","scope":"/bentopdf/","icons":[]}"#
            } else {
                b"test".as_slice()
            };

            fs::write(path, content).unwrap();
        }

        root
    }

    #[test]
    fn validates_complete_bundle() {
        let root = temp_bundle();
        assert!(validate_bundle_root(&root).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_decodes_safe_paths() {
        let root = temp_bundle();
        let canonical = root.canonicalize().unwrap();

        let index = resolve_request_path(&canonical, "/bentopdf/index.html").unwrap();
        assert!(index.ends_with("index.html"));

        assert!(resolve_request_path(&canonical, "/bentopdf/%2e%2e/Cargo.toml").is_err());

        assert!(resolve_request_path(&canonical, "/bentopdf/..%5cCargo.toml").is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_supported_byte_ranges() {
        assert_eq!(parse_single_range("bytes=0-9", 100), Ok((0, 9)));
        assert_eq!(parse_single_range("bytes=90-", 100), Ok((90, 99)));
        assert_eq!(parse_single_range("bytes=-10", 100), Ok((90, 99)));
        assert!(parse_single_range("bytes=100-101", 100).is_err());
        assert!(parse_single_range("bytes=0-1,4-5", 100).is_err());
    }

    #[test]
    fn serves_json_and_webmanifest_with_distinct_content_types() {
        assert_eq!(
            content_type_for(Path::new("config.json")),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            content_type_for(Path::new("site.webmanifest")),
            "application/manifest+json; charset=utf-8"
        );
    }

    #[test]
    fn server_handles_delayed_requests_and_client_backpressure() {
        let root = temp_bundle();
        let large_body = vec![0x5a_u8; 2 * 1024 * 1024];
        fs::write(root.join("wasm/gs/gs.wasm"), &large_body).unwrap();

        let server = StaticServer::start(BundleLocation {
            path: root.clone(),
            source: "test".to_string(),
        })
        .unwrap();

        let address = server.descriptor.address;
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();

            thread::sleep(Duration::from_millis(150));

            let request = format!(
                "GET /bentopdf/wasm/gs/gs.wasm HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                address.port()
            );

            stream.write_all(request.as_bytes()).unwrap();
            thread::sleep(Duration::from_millis(150));

            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            response
        });

        let response = client.join().unwrap();
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
            .unwrap();

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert_eq!(&response[header_end..], large_body.as_slice());

        drop(server);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn server_emits_isolation_and_csp_headers() {
        let root = temp_bundle();
        let server = StaticServer::start(BundleLocation {
            path: root.clone(),
            source: "test".to_string(),
        })
        .unwrap();

        let mut stream = TcpStream::connect(server.descriptor.address).unwrap();
        let request = format!(
            "GET /bentopdf/index.html HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            server.descriptor.address.port()
        );

        stream.write_all(request.as_bytes()).unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Cross-Origin-Opener-Policy: same-origin"));
        assert!(response.contains("Cross-Origin-Embedder-Policy: require-corp"));
        assert!(response.contains("Cross-Origin-Resource-Policy: same-origin"));
        assert!(response.contains("Content-Security-Policy:"));
        assert!(response.contains(R2H_THEME_MARKER));
        assert!(response.contains(R2H_SHELL_MARKER));
        assert!(response.contains("<body>ok</body>"));

        drop(server);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn injects_the_theme_bridge_once() {
        let html = "<!doctype html><html><head><title>Tool</title></head><body>ok</body></html>";
        let once = inject_r2h_shell(html);
        let twice = inject_r2h_shell(&once);

        assert_eq!(once, twice);
        assert_eq!(once.matches(R2H_THEME_MARKER).count(), 1);
        assert_eq!(once.matches(R2H_SHELL_MARKER).count(), 1);
        assert!(once.contains("<body>ok</body>"));
    }

    #[test]
    fn exposes_local_theme_assets_without_bundle_files() {
        let (css_type, css_body) = embedded_r2h_asset(R2H_THEME_CSS_PATH).unwrap();
        let (js_type, js_body) = embedded_r2h_asset(R2H_SHELL_JS_PATH).unwrap();

        assert_eq!(css_type, "text/css; charset=utf-8");
        assert_eq!(js_type, "application/javascript; charset=utf-8");
        assert!(String::from_utf8_lossy(css_body).contains("--r2h-bg-app"));
        assert!(String::from_utf8_lossy(js_body).contains("BentoPDF 2.8.6 · AGPL-3.0"));
        assert!(embedded_r2h_asset("/bentopdf/r2h/unknown").is_none());
    }
}
