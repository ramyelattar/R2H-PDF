[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$ExpectedRoot = $Root.Replace("\", "/")
$ExpectedBranch = "feature/bentopdf-integration"
$RequiredAncestor = "df1e275548ebf342b3343cc62d59f885b0a92ab8"

$BentoDirectory = Join-Path $Root "thirdparty\bentopdf"
$BundleVersionRoot = Join-Path $Root "generated\bentopdf-offline\2.8.6-21c924a3"
$BundleDirectory = Join-Path $BundleVersionRoot "bentopdf"

$ProjectScriptDirectory = Join-Path $Root "scripts\bentopdf"
$InstalledScriptPath = Join-Path $ProjectScriptDirectory "audit-browser-network-isolation.ps1"
$IntegrationManifestPath = Join-Path $ProjectScriptDirectory "integration-manifest.json"

$RuntimeManifestPath = Join-Path `
    $Root `
    "local-packages\bentopdf-offline-runtime\2.8.6-21c924a3\runtime-manifest.json"

$EvidenceDirectory = Join-Path $Root "audit-output\bentopdf-browser-network-isolation-after-tauri"
$ReportPath = Join-Path $EvidenceDirectory "browser-network-isolation-report.json"
$SummaryPath = Join-Path $EvidenceDirectory "summary.json"

$TempRoot = Join-Path $env:TEMP "r2h-pdf-bentopdf-browser-audit"
$StaticServerScript = Join-Path $TempRoot "static-server.mjs"
$CdpAuditScript = Join-Path $TempRoot "cdp-network-audit.mjs"
$EdgeProfileDirectory = Join-Path $TempRoot "edge-profile"

New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $ProjectScriptDirectory | Out-Null

Set-Location $Root

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string]$FilePath,

        [Parameter()]
        [string[]]$Arguments = @(),

        [Parameter()]
        [string]$WorkingDirectory = $Root,

        [switch]$AllowFailure
    )

    $PreviousPreference = $ErrorActionPreference
    $Pushed = $false

    try {
        $ErrorActionPreference = "Continue"

        Push-Location -LiteralPath $WorkingDirectory
        $Pushed = $true

        $RawOutput = @(
            & $FilePath @Arguments 2>&1
        )

        $ExitCode = $LASTEXITCODE
    }
    finally {
        if ($Pushed) {
            Pop-Location
        }

        $ErrorActionPreference = $PreviousPreference
    }

    $Lines = @(
        foreach ($Item in $RawOutput) {
            $Item.ToString()
        }
    )

    $SafeName = $Name -replace '[^A-Za-z0-9._-]', '-'
    $LogPath = Join-Path $EvidenceDirectory "$SafeName.log"

    if ($Lines.Count -gt 0) {
        $Lines | Set-Content -LiteralPath $LogPath -Encoding UTF8
    }
    else {
        Set-Content -LiteralPath $LogPath -Value "" -Encoding UTF8
    }

    Set-Content `
        -LiteralPath "$EvidenceDirectory\$SafeName.exit-code.txt" `
        -Value $ExitCode `
        -Encoding ASCII

    Write-Host "`n=== $Name ===" -ForegroundColor Cyan
    Write-Host "$FilePath $($Arguments -join ' ')" -ForegroundColor DarkGray

    if ($Lines.Count -le 80) {
        foreach ($Line in $Lines) {
            Write-Host $Line
        }
    }
    else {
        Write-Host "Output lines: $($Lines.Count)"
        $Lines | Select-Object -First 10 | ForEach-Object { Write-Host $_ }
        Write-Host "... full output saved to $LogPath ..." -ForegroundColor DarkGray
        $Lines | Select-Object -Last 50 | ForEach-Object { Write-Host $_ }
    }

    if (-not $AllowFailure.IsPresent -and $ExitCode -ne 0) {
        throw @"
Native command failed: $Name

Working directory:
$WorkingDirectory

Command:
$FilePath $($Arguments -join " ")

Exit code:
$ExitCode

Log:
$LogPath
"@
    }

    return [PSCustomObject]@{
        ExitCode = $ExitCode
        Lines = $Lines
        LogPath = $LogPath
    }
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [Parameter()]
        [string]$WorkingDirectory = $Root,

        [switch]$AllowFailure
    )

    return Invoke-Native `
        -Name $Name `
        -FilePath "git.exe" `
        -Arguments $Arguments `
        -WorkingDirectory $WorkingDirectory `
        -AllowFailure:$AllowFailure
}

function Get-LastNonEmptyLine {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Lines,

        [Parameter(Mandatory = $true)]
        [string]$Operation
    )

    $NonEmpty = @(
        $Lines |
            Where-Object {
                -not [string]::IsNullOrWhiteSpace($_)
            }
    )

    if ($NonEmpty.Count -eq 0) {
        throw "$Operation returned no output."
    }

    return $NonEmpty[$NonEmpty.Count - 1].Trim()
}

function Get-FreeTcpPort {
    $Listener = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )

    try {
        $Listener.Start()
        return ([System.Net.IPEndPoint]$Listener.LocalEndpoint).Port
    }
    finally {
        $Listener.Stop()
    }
}

function Remove-SafeDirectory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$AllowedParent
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $ResolvedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd("\")
    $ResolvedParent = [System.IO.Path]::GetFullPath($AllowedParent).TrimEnd("\")

    if (
        $ResolvedPath -eq $ResolvedParent -or
        -not $ResolvedPath.StartsWith(
            $ResolvedParent + "\",
            [System.StringComparison]::OrdinalIgnoreCase
        )
    ) {
        throw "Refusing to remove directory outside approved scope: $ResolvedPath"
    }

    Remove-Item -LiteralPath $ResolvedPath -Recurse -Force
}

function Find-EdgeExecutable {
    $Candidates = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
        "$env:LOCALAPPDATA\Microsoft\Edge\Application\msedge.exe"
    )

    return $Candidates |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_) -and
            (Test-Path -LiteralPath $_ -PathType Leaf)
        } |
        Select-Object -First 1
}

function Set-ManifestProperty {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Object,

        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter()]
        [object]$Value
    )

    $Object |
        Add-Member `
            -MemberType NoteProperty `
            -Name $Name `
            -Value $Value `
            -Force
}

Write-Host "`n================================================" -ForegroundColor Cyan
Write-Host "R2H-PDF BENTOPDF BROWSER NETWORK AUDIT" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

# ============================================================
# 1. Preflight
# ============================================================

$RequiredCommands = @(
    "git.exe",
    "node.exe"
)

foreach ($RequiredCommand in $RequiredCommands) {
    $ResolvedCommand = Get-Command `
        $RequiredCommand `
        -ErrorAction SilentlyContinue

    if ($null -eq $ResolvedCommand) {
        throw "Required executable was not found: $RequiredCommand"
    }

    Write-Host "$RequiredCommand -> $($ResolvedCommand.Source)" -ForegroundColor Green
}

$NodeExecutable = (
    Get-Command "node.exe" -ErrorAction Stop
).Source

$EdgeExecutable = Find-EdgeExecutable

if ([string]::IsNullOrWhiteSpace($EdgeExecutable)) {
    throw "Microsoft Edge executable was not found."
}

Write-Host "Edge -> $EdgeExecutable" -ForegroundColor Green

$WebSocketCheck = Invoke-Native `
    -Name "01-node-global-websocket" `
    -FilePath $NodeExecutable `
    -Arguments @(
        "-e",
        "process.stdout.write(typeof WebSocket)"
    )

$WebSocketType = Get-LastNonEmptyLine `
    -Lines $WebSocketCheck.Lines `
    -Operation "Node global WebSocket check"

if ($WebSocketType -ne "function") {
    throw @"
This audit requires Node's built-in WebSocket client.

Detected:
typeof WebSocket = $WebSocketType

No npm ws package is required when the built-in client is available.
"@
}

Write-Host "Node built-in WebSocket: available" -ForegroundColor Green

$RootResult = Invoke-Git `
    -Name "02-parent-root" `
    -Arguments @(
        "rev-parse",
        "--show-toplevel"
    )

$ResolvedRoot = (
    Get-LastNonEmptyLine `
        -Lines $RootResult.Lines `
        -Operation "Parent repository root"
).Replace("\", "/")

if ($ResolvedRoot -ne $ExpectedRoot) {
    throw "Wrong repository root. Expected $ExpectedRoot, found $ResolvedRoot"
}

$BranchResult = Invoke-Git `
    -Name "03-parent-branch" `
    -Arguments @(
        "branch",
        "--show-current"
    )

$CurrentBranch = Get-LastNonEmptyLine `
    -Lines $BranchResult.Lines `
    -Operation "Current branch"

if ($CurrentBranch -ne $ExpectedBranch) {
    throw "Wrong branch. Expected $ExpectedBranch, found $CurrentBranch"
}

$AncestorResult = Invoke-Git `
    -Name "04-required-ancestor" `
    -Arguments @(
        "merge-base",
        "--is-ancestor",
        $RequiredAncestor,
        "HEAD"
    ) `
    -AllowFailure

if ($AncestorResult.ExitCode -ne 0) {
    throw "Required offline-runtime commit is not an ancestor of HEAD."
}

$ParentStatusBefore = Invoke-Git `
    -Name "05-parent-status-before" `
    -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=all"
    )

$ParentChangesBefore = @(
    $ParentStatusBefore.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($ParentChangesBefore.Count -gt 0) {
    Write-Host "Parent repository changes:" -ForegroundColor Red
    $ParentChangesBefore | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Parent repository must be clean before the browser audit."
}

$BentoStatusBefore = Invoke-Git `
    -Name "06-bentopdf-status-before" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=all"
    )

$BentoChangesBefore = @(
    $BentoStatusBefore.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($BentoChangesBefore.Count -gt 0) {
    Write-Host "BentoPDF submodule changes:" -ForegroundColor Red
    $BentoChangesBefore | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "BentoPDF submodule must be clean before the browser audit."
}

$RequiredBundleFiles = @(
    "index.html",
    "merge-pdf.html",
    "compress-pdf.html",
    "ocr-pdf.html",
    "edit-pdf.html",
    "ar\index.html",
    "wasm\pymupdf\dist\index.js",
    "wasm\gs\gs.js",
    "wasm\gs\gs.wasm",
    "wasm\cpdf\coherentpdf.browser.min.js",
    "wasm\ocr\worker.min.js",
    "wasm\ocr\lang-data\ara.traineddata.gz",
    "wasm\ocr\lang-data\eng.traineddata.gz",
    "wasm\ocr\fonts\NotoSans-Regular.ttf",
    "wasm\ocr\fonts\NotoNaskhArabic-Regular.ttf"
)

foreach ($RelativePath in $RequiredBundleFiles) {
    $FullPath = Join-Path $BundleDirectory $RelativePath

    if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) {
        throw "Required offline bundle file is missing: $RelativePath"
    }
}

Write-Host "Repository and offline bundle preflight: passed" -ForegroundColor Green

# ============================================================
# 2. Prepare temporary audit scripts
# ============================================================

$TempParent = Split-Path -Parent $TempRoot

if (Test-Path -LiteralPath $TempRoot) {
    Remove-SafeDirectory `
        -Path $TempRoot `
        -AllowedParent $TempParent
}

New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
New-Item -ItemType Directory -Force -Path $EdgeProfileDirectory | Out-Null

@'
import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(process.argv[2]);
const port = Number(process.argv[3]);

const mimeTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
  ".gz": "application/gzip",
  ".ttf": "font/ttf",
  ".otf": "font/otf",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".ico": "image/x-icon",
  ".pdf": "application/pdf",
  ".xml": "application/xml; charset=utf-8",
};

const server = http.createServer((request, response) => {
  try {
    const requestUrl = new URL(request.url || "/", "http://127.0.0.1");
    const relativePath = decodeURIComponent(requestUrl.pathname)
      .replace(/^[/\\]+/, "");

    let targetPath = path.resolve(root, relativePath);

    if (targetPath !== root && !targetPath.startsWith(root + path.sep)) {
      response.writeHead(403);
      response.end("Forbidden");
      return;
    }

    if (fs.existsSync(targetPath) && fs.statSync(targetPath).isDirectory()) {
      targetPath = path.join(targetPath, "index.html");
    }

    if (!fs.existsSync(targetPath) || !fs.statSync(targetPath).isFile()) {
      response.writeHead(404);
      response.end("Not found");
      return;
    }

    const stat = fs.statSync(targetPath);
    const extension = path.extname(targetPath).toLowerCase();

    response.setHeader(
      "Content-Type",
      mimeTypes[extension] || "application/octet-stream"
    );
    response.setHeader("Content-Length", String(stat.size));
    response.setHeader("Cache-Control", "no-store");
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    response.setHeader("X-Content-Type-Options", "nosniff");

    if ((request.method || "GET").toUpperCase() === "HEAD") {
      response.writeHead(200);
      response.end();
      return;
    }

    response.writeHead(200);
    fs.createReadStream(targetPath).pipe(response);
  } catch (error) {
    response.writeHead(500);
    response.end(String(error));
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`READY http://127.0.0.1:${port}`);
});
'@ |
    Set-Content `
        -LiteralPath $StaticServerScript `
        -Encoding UTF8

@'
import fs from "node:fs";

const staticPort = Number(process.argv[2]);
const devToolsPort = Number(process.argv[3]);
const reportPath = process.argv[4];

if (typeof WebSocket !== "function") {
  throw new Error("Node global WebSocket is unavailable.");
}

const localOrigin = `http://127.0.0.1:${staticPort}`;
const devToolsOrigin = `http://127.0.0.1:${devToolsPort}`;

const pages = [
  "/bentopdf/index.html",
  "/bentopdf/ar/index.html",
  "/bentopdf/merge-pdf.html",
  "/bentopdf/compress-pdf.html",
  "/bentopdf/ocr-pdf.html",
  "/bentopdf/edit-pdf.html",
];

const runtimeEndpoints = [
  "/bentopdf/wasm/pymupdf/dist/index.js",
  "/bentopdf/wasm/gs/gs.js",
  "/bentopdf/wasm/gs/gs.wasm",
  "/bentopdf/wasm/cpdf/coherentpdf.browser.min.js",
  "/bentopdf/wasm/ocr/worker.min.js",
  "/bentopdf/wasm/ocr/lang-data/ara.traineddata.gz",
  "/bentopdf/wasm/ocr/lang-data/eng.traineddata.gz",
  "/bentopdf/wasm/ocr/fonts/NotoSans-Regular.ttf",
  "/bentopdf/wasm/ocr/fonts/NotoNaskhArabic-Regular.ttf",
];

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function unique(values) {
  return Array.from(new Set(values));
}

function normalizeError(error) {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack || null,
    };
  }

  return {
    name: "Error",
    message: String(error),
    stack: null,
  };
}

function isAllowedUrl(urlString) {
  try {
    const parsed = new URL(urlString);

    if (parsed.origin === localOrigin) {
      return true;
    }

    return [
      "data:",
      "blob:",
      "about:",
      "chrome:",
      "edge:",
      "devtools:",
    ].includes(parsed.protocol);
  } catch {
    return false;
  }
}

class CdpClient {
  constructor(webSocketUrl) {
    this.webSocketUrl = webSocketUrl;
    this.socket = null;
    this.nextId = 1;
    this.pending = new Map();
    this.waiters = new Map();
    this.listeners = new Map();
  }

  async connect() {
    this.socket = new WebSocket(this.webSocketUrl);

    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error("Timed out while opening the CDP WebSocket."));
      }, 15000);

      this.socket.addEventListener("open", () => {
        clearTimeout(timeout);
        resolve();
      }, { once: true });

      this.socket.addEventListener("error", () => {
        clearTimeout(timeout);
        reject(new Error("CDP WebSocket connection failed."));
      }, { once: true });
    });

    this.socket.addEventListener("message", (event) => {
      const text =
        typeof event.data === "string"
          ? event.data
          : Buffer.from(event.data).toString("utf8");

      const message = JSON.parse(text);

      if (message.id) {
        const pending = this.pending.get(message.id);

        if (!pending) {
          return;
        }

        this.pending.delete(message.id);
        clearTimeout(pending.timeout);

        if (message.error) {
          pending.reject(
            new Error(
              `${pending.method} failed: ${JSON.stringify(message.error)}`
            )
          );
        } else {
          pending.resolve(message.result || {});
        }

        return;
      }

      if (!message.method) {
        return;
      }

      const listeners = this.listeners.get(message.method) || [];

      for (const listener of listeners) {
        try {
          listener(message.params || {});
        } catch (error) {
          console.error(
            `Listener failure for ${message.method}:`,
            normalizeError(error)
          );
        }
      }

      const waiters = this.waiters.get(message.method) || [];

      if (waiters.length > 0) {
        this.waiters.delete(message.method);

        for (const waiter of waiters) {
          clearTimeout(waiter.timeout);
          waiter.resolve(message.params || {});
        }
      }
    });
  }

  on(method, listener) {
    const listeners = this.listeners.get(method) || [];
    listeners.push(listener);
    this.listeners.set(method, listeners);
  }

  waitFor(method, timeoutMilliseconds = 20000) {
    return new Promise((resolve, reject) => {
      const waiters = this.waiters.get(method) || [];

      const timeout = setTimeout(() => {
        const current = this.waiters.get(method) || [];
        this.waiters.set(
          method,
          current.filter((entry) => entry.resolve !== resolve)
        );
        reject(new Error(`Timed out waiting for CDP event ${method}.`));
      }, timeoutMilliseconds);

      waiters.push({
        resolve,
        reject,
        timeout,
      });

      this.waiters.set(method, waiters);
    });
  }

  send(method, params = {}, timeoutMilliseconds = 20000) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("CDP WebSocket is not open."));
    }

    const id = this.nextId++;

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Timed out while executing ${method}.`));
      }, timeoutMilliseconds);

      this.pending.set(id, {
        resolve,
        reject,
        timeout,
        method,
      });

      this.socket.send(
        JSON.stringify({
          id,
          method,
          params,
        })
      );
    });
  }

  close() {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.close();
    }
  }
}

async function createPageTarget() {
  const targetResponse = await fetch(
    `${devToolsOrigin}/json/new?${encodeURIComponent("about:blank")}`,
    {
      method: "PUT",
    }
  );

  if (!targetResponse.ok) {
    throw new Error(
      `Unable to create Edge page target: HTTP ${targetResponse.status}`
    );
  }

  const target = await targetResponse.json();

  if (!target.webSocketDebuggerUrl) {
    throw new Error("Edge target has no webSocketDebuggerUrl.");
  }

  return target;
}

async function evaluate(client, expression, awaitPromise = false) {
  const result = await client.send("Runtime.evaluate", {
    expression,
    awaitPromise,
    returnByValue: true,
    userGesture: true,
  });

  if (result.exceptionDetails) {
    throw new Error(
      `Runtime.evaluate failed: ${JSON.stringify(result.exceptionDetails)}`
    );
  }

  return result.result ? result.result.value : undefined;
}

const report = {
  schemaVersion: 1,
  auditedAtUtc: new Date().toISOString(),
  localOrigin,
  pages: [],
  runtimeEndpointResults: [],
  externalRequests: [],
  blockedExternalRequests: [],
  localHttpFailures: [],
  localLoadingFailures: [],
  uncaughtExceptions: [],
  consoleErrors: [],
  logErrors: [],
  crossOriginIsolationFailures: [],
  performanceExternalResources: [],
  auditInfrastructureErrors: [],
};

let client = null;

try {
  const target = await createPageTarget();
  client = new CdpClient(target.webSocketDebuggerUrl);
  await client.connect();

  const requestUrls = new Map();
  let currentPage = "about:blank";

  client.on("Network.requestWillBeSent", (params) => {
    const url = params.request?.url || "";
    requestUrls.set(params.requestId, url);

    if (
      url &&
      !isAllowedUrl(url) &&
      /^(https?|wss?):/i.test(url)
    ) {
      report.externalRequests.push({
        page: currentPage,
        url,
        method: params.request?.method || null,
        resourceType: params.type || null,
        initiator: params.initiator || null,
      });
    }
  });

  client.on("Network.responseReceived", (params) => {
    const response = params.response || {};
    const url = response.url || "";

    if (
      url.startsWith(localOrigin) &&
      Number(response.status) >= 400
    ) {
      report.localHttpFailures.push({
        page: currentPage,
        url,
        status: response.status,
        statusText: response.statusText || null,
        resourceType: params.type || null,
      });
    }
  });

  client.on("Network.loadingFailed", (params) => {
    const url = requestUrls.get(params.requestId) || "";

    if (url.startsWith(localOrigin)) {
      report.localLoadingFailures.push({
        page: currentPage,
        url,
        errorText: params.errorText || null,
        blockedReason: params.blockedReason || null,
        canceled: Boolean(params.canceled),
      });
    }
  });

  client.on("Runtime.exceptionThrown", (params) => {
    report.uncaughtExceptions.push({
      page: currentPage,
      exceptionDetails: params.exceptionDetails || params,
    });
  });

  client.on("Runtime.consoleAPICalled", (params) => {
    if (!["error", "assert"].includes(params.type)) {
      return;
    }

    report.consoleErrors.push({
      page: currentPage,
      type: params.type,
      args: (params.args || []).map((argument) => ({
        type: argument.type || null,
        value:
          Object.prototype.hasOwnProperty.call(argument, "value")
            ? argument.value
            : argument.description || null,
      })),
      stackTrace: params.stackTrace || null,
    });
  });

  client.on("Log.entryAdded", (params) => {
    const entry = params.entry || {};

    if (entry.level !== "error") {
      return;
    }

    report.logErrors.push({
      page: currentPage,
      source: entry.source || null,
      text: entry.text || null,
      url: entry.url || null,
      lineNumber: entry.lineNumber || null,
    });
  });

  client.on("Fetch.requestPaused", async (params) => {
    const url = params.request?.url || "";

    try {
      if (
        /^(https?|wss?):/i.test(url) &&
        !isAllowedUrl(url)
      ) {
        report.blockedExternalRequests.push({
          page: currentPage,
          url,
          method: params.request?.method || null,
          resourceType: params.resourceType || null,
        });

        await client.send("Fetch.failRequest", {
          requestId: params.requestId,
          errorReason: "BlockedByClient",
        });

        return;
      }

      await client.send("Fetch.continueRequest", {
        requestId: params.requestId,
      });
    } catch (error) {
      report.auditInfrastructureErrors.push({
        phase: "Fetch.requestPaused",
        page: currentPage,
        url,
        error: normalizeError(error),
      });
    }
  });

  await client.send("Network.enable");
  await client.send("Network.setCacheDisabled", {
    cacheDisabled: true,
  });
  await client.send("Network.setBypassServiceWorker", {
    bypass: true,
  });
  await client.send("Page.enable");
  await client.send("Runtime.enable");
  await client.send("Log.enable");
  await client.send("Fetch.enable", {
    patterns: [
      {
        urlPattern: "*",
        requestStage: "Request",
      },
    ],
  });

  for (const pagePath of pages) {
    currentPage = pagePath;
    const targetUrl = `${localOrigin}${pagePath}`;

    const loadPromise = client.waitFor("Page.loadEventFired", 30000);

    await client.send("Page.navigate", {
      url: targetUrl,
    });

    await loadPromise;
    await delay(2500);

    const state = await evaluate(
      client,
      `(() => ({
        href: location.href,
        title: document.title,
        readyState: document.readyState,
        crossOriginIsolated: globalThis.crossOriginIsolated === true,
        secureContext: globalThis.isSecureContext === true,
        resourceUrls: performance
          .getEntriesByType("resource")
          .map((entry) => entry.name)
      }))()`
    );

    const externalPerformanceResources = (
      state.resourceUrls || []
    ).filter((url) => {
      try {
        const parsed = new URL(url);
        return (
          /^(https?|wss?):/i.test(parsed.protocol) &&
          parsed.origin !== localOrigin
        );
      } catch {
        return false;
      }
    });

    for (const url of externalPerformanceResources) {
      report.performanceExternalResources.push({
        page: pagePath,
        url,
      });
    }

    if (!state.crossOriginIsolated) {
      report.crossOriginIsolationFailures.push({
        page: pagePath,
        href: state.href,
        secureContext: state.secureContext,
      });
    }

    report.pages.push({
      path: pagePath,
      href: state.href,
      title: state.title,
      readyState: state.readyState,
      crossOriginIsolated: state.crossOriginIsolated,
      secureContext: state.secureContext,
      resourceCount: (state.resourceUrls || []).length,
      externalPerformanceResourceCount:
        externalPerformanceResources.length,
    });
  }

  currentPage = "runtime-endpoint-fetch";

  const runtimeResults = await evaluate(
    client,
    `(async () => {
      const paths = ${JSON.stringify(runtimeEndpoints)};
      const results = [];

      for (const path of paths) {
        try {
          const response = await fetch(path, {
            cache: "no-store",
          });

          const buffer = await response.arrayBuffer();

          results.push({
            path,
            ok: response.ok,
            status: response.status,
            contentType: response.headers.get("content-type"),
            byteLength: buffer.byteLength,
          });
        } catch (error) {
          results.push({
            path,
            ok: false,
            status: 0,
            contentType: null,
            byteLength: 0,
            error: String(error),
          });
        }
      }

      return results;
    })()`,
    true
  );

  report.runtimeEndpointResults = runtimeResults;

  await delay(1500);
} catch (error) {
  report.auditInfrastructureErrors.push({
    phase: "main",
    error: normalizeError(error),
  });
} finally {
  if (client) {
    client.close();
  }
}

report.externalRequests = report.externalRequests.filter(
  (entry, index, all) =>
    all.findIndex(
      (candidate) =>
        candidate.page === entry.page &&
        candidate.url === entry.url &&
        candidate.method === entry.method
    ) === index
);

report.blockedExternalRequests = report.blockedExternalRequests.filter(
  (entry, index, all) =>
    all.findIndex(
      (candidate) =>
        candidate.page === entry.page &&
        candidate.url === entry.url &&
        candidate.method === entry.method
    ) === index
);

report.performanceExternalResources =
  report.performanceExternalResources.filter(
    (entry, index, all) =>
      all.findIndex(
        (candidate) =>
          candidate.page === entry.page &&
          candidate.url === entry.url
      ) === index
  );

const endpointFailures = report.runtimeEndpointResults.filter(
  (entry) =>
    !entry.ok ||
    entry.status !== 200 ||
    !Number.isFinite(entry.byteLength) ||
    entry.byteLength <= 0
);

report.summary = {
  auditedPageCount: report.pages.length,
  expectedPageCount: pages.length,
  runtimeEndpointCount: report.runtimeEndpointResults.length,
  expectedRuntimeEndpointCount: runtimeEndpoints.length,

  externalRequestCount: report.externalRequests.length,
  blockedExternalRequestCount: report.blockedExternalRequests.length,
  performanceExternalResourceCount:
    report.performanceExternalResources.length,

  localHttpFailureCount: report.localHttpFailures.length,
  localLoadingFailureCount: report.localLoadingFailures.length,
  runtimeEndpointFailureCount: endpointFailures.length,

  uncaughtExceptionCount: report.uncaughtExceptions.length,
  consoleErrorCount: report.consoleErrors.length,
  logErrorCount: report.logErrors.length,
  crossOriginIsolationFailureCount:
    report.crossOriginIsolationFailures.length,
  auditInfrastructureErrorCount:
    report.auditInfrastructureErrors.length,
};

report.summary.pass =
  report.summary.auditedPageCount === report.summary.expectedPageCount &&
  report.summary.runtimeEndpointCount ===
    report.summary.expectedRuntimeEndpointCount &&
  report.summary.externalRequestCount === 0 &&
  report.summary.blockedExternalRequestCount === 0 &&
  report.summary.performanceExternalResourceCount === 0 &&
  report.summary.localHttpFailureCount === 0 &&
  report.summary.localLoadingFailureCount === 0 &&
  report.summary.runtimeEndpointFailureCount === 0 &&
  report.summary.uncaughtExceptionCount === 0 &&
  report.summary.crossOriginIsolationFailureCount === 0 &&
  report.summary.auditInfrastructureErrorCount === 0;

fs.writeFileSync(
  reportPath,
  JSON.stringify(report, null, 2),
  "utf8"
);

console.log(
  JSON.stringify(
    {
      reportPath,
      summary: report.summary,
      externalUrls: unique(
        report.externalRequests.map((entry) => entry.url)
      ),
      blockedExternalUrls: unique(
        report.blockedExternalRequests.map((entry) => entry.url)
      ),
      endpointFailures,
    },
    null,
    2
  )
);

process.exit(report.summary.pass ? 0 : 1);
'@ |
    Set-Content `
        -LiteralPath $CdpAuditScript `
        -Encoding UTF8

# ============================================================
# 3. Start static server and Edge
# ============================================================

$StaticPort = Get-FreeTcpPort
$DevToolsPort = Get-FreeTcpPort

$ServerStdout = Join-Path $EvidenceDirectory "static-server.stdout.log"
$ServerStderr = Join-Path $EvidenceDirectory "static-server.stderr.log"
$EdgeStdout = Join-Path $EvidenceDirectory "edge.stdout.log"
$EdgeStderr = Join-Path $EvidenceDirectory "edge.stderr.log"

$ServerProcess = $null
$EdgeProcess = $null

try {
    $ServerProcess = Start-Process `
        -FilePath $NodeExecutable `
        -ArgumentList @(
            $StaticServerScript,
            $BundleVersionRoot,
            "$StaticPort"
        ) `
        -WorkingDirectory $Root `
        -RedirectStandardOutput $ServerStdout `
        -RedirectStandardError $ServerStderr `
        -PassThru `
        -NoNewWindow

    $ServerReady = $false

    for ($Attempt = 1; $Attempt -le 40; $Attempt++) {
        Start-Sleep -Milliseconds 250

        if ($ServerProcess.HasExited) {
            break
        }

        try {
            $Response = Invoke-WebRequest `
                -Uri "http://127.0.0.1:$StaticPort/bentopdf/index.html" `
                -Method Head `
                -UseBasicParsing `
                -TimeoutSec 2

            if ($Response.StatusCode -eq 200) {
                $ServerReady = $true
                break
            }
        }
        catch {
            # Continue until bounded timeout.
        }
    }

    if (-not $ServerReady) {
        $ServerError = ""

        if (Test-Path -LiteralPath $ServerStderr) {
            $ServerError = Get-Content -LiteralPath $ServerStderr -Raw
        }

        throw "Static audit server did not become ready. $ServerError"
    }

    Write-Host "Static server ready: http://127.0.0.1:$StaticPort" -ForegroundColor Green

    $EdgeArguments = @(
        "--headless=new",
        "--remote-debugging-address=127.0.0.1",
        "--remote-debugging-port=$DevToolsPort",
        "--user-data-dir=$EdgeProfileDirectory",
        "--disable-gpu",
        "--disable-extensions",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-default-apps",
        "--disable-domain-reliability",
        "--disable-client-side-phishing-detection",
        "--disable-sync",
        "--metrics-recording-only",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-features=Translate,MediaRouter,OptimizationHints,AutofillServerCommunication",
        "about:blank"
    )

    $EdgeProcess = Start-Process `
        -FilePath $EdgeExecutable `
        -ArgumentList $EdgeArguments `
        -WorkingDirectory $Root `
        -RedirectStandardOutput $EdgeStdout `
        -RedirectStandardError $EdgeStderr `
        -PassThru `
        -NoNewWindow

    $DevToolsReady = $false

    for ($Attempt = 1; $Attempt -le 60; $Attempt++) {
        Start-Sleep -Milliseconds 250

        if ($EdgeProcess.HasExited) {
            break
        }

        try {
            $VersionResponse = Invoke-WebRequest `
                -Uri "http://127.0.0.1:$DevToolsPort/json/version" `
                -UseBasicParsing `
                -TimeoutSec 2

            if ($VersionResponse.StatusCode -eq 200) {
                $DevToolsReady = $true
                break
            }
        }
        catch {
            # Continue until bounded timeout.
        }
    }

    if (-not $DevToolsReady) {
        $EdgeError = ""

        if (Test-Path -LiteralPath $EdgeStderr) {
            $EdgeError = Get-Content -LiteralPath $EdgeStderr -Raw
        }

        throw "Edge DevTools endpoint did not become ready. $EdgeError"
    }

    Write-Host "Edge DevTools ready: http://127.0.0.1:$DevToolsPort" -ForegroundColor Green

    $AuditResult = Invoke-Native `
        -Name "07-browser-network-isolation-audit" `
        -FilePath $NodeExecutable `
        -Arguments @(
            $CdpAuditScript,
            "$StaticPort",
            "$DevToolsPort",
            $ReportPath
        ) `
        -WorkingDirectory $Root `
        -AllowFailure

    if (-not (Test-Path -LiteralPath $ReportPath -PathType Leaf)) {
        throw "Browser audit did not create its JSON report."
    }

    $AuditReport = Get-Content `
        -LiteralPath $ReportPath `
        -Raw |
        ConvertFrom-Json

    $AuditReport.summary |
        ConvertTo-Json `
            -Depth 5 |
        Set-Content `
            -LiteralPath $SummaryPath `
            -Encoding UTF8

    Write-Host "`n=== BROWSER AUDIT SUMMARY ===" -ForegroundColor Cyan
    $AuditReport.summary | ConvertTo-Json -Depth 5

    if ($AuditResult.ExitCode -ne 0 -or -not $AuditReport.summary.pass) {
        Write-Host "`nBrowser network-isolation audit failed." -ForegroundColor Red
        Write-Host "Report: $ReportPath" -ForegroundColor Yellow

        if ($AuditReport.externalRequests.Count -gt 0) {
            Write-Host "`nExternal requests:" -ForegroundColor Red

            $AuditReport.externalRequests |
                Select-Object -First 30 |
                ForEach-Object {
                    Write-Host "$($_.page) -> $($_.url)" -ForegroundColor Yellow
                }
        }

        if ($AuditReport.uncaughtExceptions.Count -gt 0) {
            Write-Host "`nUncaught exceptions: $($AuditReport.uncaughtExceptions.Count)" -ForegroundColor Red
        }

        if ($AuditReport.crossOriginIsolationFailures.Count -gt 0) {
            Write-Host "`nCross-origin isolation failures:" -ForegroundColor Red

            $AuditReport.crossOriginIsolationFailures |
                ForEach-Object {
                    Write-Host "$($_.page) -> $($_.href)" -ForegroundColor Yellow
                }
        }

        throw "BentoPDF browser network-isolation audit did not pass."
    }
}
finally {
    if ($null -ne $EdgeProcess -and -not $EdgeProcess.HasExited) {
        Stop-Process -Id $EdgeProcess.Id -Force
        $EdgeProcess.WaitForExit()
    }

    if ($null -ne $ServerProcess -and -not $ServerProcess.HasExited) {
        Stop-Process -Id $ServerProcess.Id -Force
        $ServerProcess.WaitForExit()
    }
}

# ============================================================
# 4. Record successful audit in manifests
# ============================================================

if (-not (Test-Path -LiteralPath $IntegrationManifestPath -PathType Leaf)) {
    throw "Tracked BentoPDF integration manifest is missing."
}

$IntegrationManifest = Get-Content `
    -LiteralPath $IntegrationManifestPath `
    -Raw |
    ConvertFrom-Json

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserNetworkAuditPassed" `
    -Value $true

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserNetworkAuditReport" `
    -Value "audit-output/bentopdf-browser-network-isolation-after-tauri/browser-network-isolation-report.json"

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserNetworkAuditPages" `
    -Value $AuditReport.summary.auditedPageCount

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserExternalRequestCount" `
    -Value $AuditReport.summary.externalRequestCount

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserCrossOriginIsolationFailures" `
    -Value $AuditReport.summary.crossOriginIsolationFailureCount

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "fullyOffline" `
    -Value $false

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "tauriUiWired" `
    -Value $true

$IntegrationManifest |
    ConvertTo-Json `
        -Depth 10 |
    Set-Content `
        -LiteralPath $IntegrationManifestPath `
        -Encoding UTF8

if (Test-Path -LiteralPath $RuntimeManifestPath -PathType Leaf) {
    $RuntimeManifest = Get-Content `
        -LiteralPath $RuntimeManifestPath `
        -Raw |
        ConvertFrom-Json

    Set-ManifestProperty `
        -Object $RuntimeManifest `
        -Name "browserNetworkIsolationAuditPassed" `
        -Value $true

    Set-ManifestProperty `
        -Object $RuntimeManifest `
        -Name "browserNetworkIsolationReport" `
        -Value $ReportPath

    Set-ManifestProperty `
        -Object $RuntimeManifest `
        -Name "fullyOfflineClaimed" `
        -Value $false

    $RuntimeManifest |
        ConvertTo-Json `
            -Depth 10 |
        Set-Content `
            -LiteralPath $RuntimeManifestPath `
            -Encoding UTF8
}

if ($PSCommandPath -ne $InstalledScriptPath) {
    Copy-Item `
        -LiteralPath $PSCommandPath `
        -Destination $InstalledScriptPath `
        -Force
}

# ============================================================
# 5. Commit reproducible audit pipeline
# ============================================================

Invoke-Git `
    -Name "08-stage-browser-audit-pipeline" `
    -Arguments @(
        "add",
        "--",
        "scripts/bentopdf/audit-browser-network-isolation.ps1",
        "scripts/bentopdf/integration-manifest.json"
    ) | Out-Null

$StagedResult = Invoke-Git `
    -Name "09-staged-browser-audit-pipeline" `
    -Arguments @(
        "-c",
        "core.quotepath=false",
        "diff",
        "--cached",
        "--name-only"
    )

$StagedFiles = @(
    $StagedResult.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        } |
        ForEach-Object {
            $_.Replace("\", "/")
        }
)

$AllowedStagedFiles = @(
    "scripts/bentopdf/audit-browser-network-isolation.ps1",
    "scripts/bentopdf/integration-manifest.json"
)

$UnexpectedStagedFiles = @(
    $StagedFiles |
        Where-Object {
            $_ -notin $AllowedStagedFiles
        }
)

if ($UnexpectedStagedFiles.Count -gt 0) {
    Write-Host "Unexpected staged files:" -ForegroundColor Red
    $UnexpectedStagedFiles | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Refusing to commit unexpected files."
}

if ($StagedFiles.Count -gt 0) {
    Invoke-Git `
        -Name "10-commit-browser-audit-pipeline" `
        -Arguments @(
            "commit",
            "-m",
            "test(pdf): verify BentoPDF browser network isolation"
        ) | Out-Null
}
else {
    Write-Host "Tracked browser-audit files are unchanged; no new commit required." -ForegroundColor Yellow
}

# ============================================================
# 6. Final verification
# ============================================================

$FinalHeadResult = Invoke-Git `
    -Name "11-final-head" `
    -Arguments @(
        "rev-parse",
        "--verify",
        "HEAD^{commit}"
    )

$FinalCommit = Get-LastNonEmptyLine `
    -Lines $FinalHeadResult.Lines `
    -Operation "Final parent commit"

$FinalParentStatus = Invoke-Git `
    -Name "12-final-parent-status" `
    -Arguments @(
        "status",
        "--short"
    )

$RemainingParentChanges = @(
    $FinalParentStatus.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($RemainingParentChanges.Count -gt 0) {
    throw "Parent repository is not clean after browser audit."
}

$FinalBentoStatus = Invoke-Git `
    -Name "13-final-bentopdf-status" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--short"
    )

$RemainingBentoChanges = @(
    $FinalBentoStatus.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($RemainingBentoChanges.Count -gt 0) {
    throw "BentoPDF submodule is not clean after browser audit."
}

Write-Host "`n================================================" -ForegroundColor Green
Write-Host "BENTOPDF BROWSER NETWORK AUDIT PASSED" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Commit: $FinalCommit"
Write-Host "Audited pages: $($AuditReport.summary.auditedPageCount)"
Write-Host "Runtime endpoints: $($AuditReport.summary.runtimeEndpointCount)"
Write-Host "External requests: $($AuditReport.summary.externalRequestCount)"
Write-Host "Blocked external requests: $($AuditReport.summary.blockedExternalRequestCount)"
Write-Host "Local HTTP failures: $($AuditReport.summary.localHttpFailureCount)"
Write-Host "Runtime endpoint failures: $($AuditReport.summary.runtimeEndpointFailureCount)"
Write-Host "Uncaught exceptions: $($AuditReport.summary.uncaughtExceptionCount)"
Write-Host "Cross-origin isolation failures: $($AuditReport.summary.crossOriginIsolationFailureCount)"
Write-Host "Report: $ReportPath"
Write-Host "Parent working tree: clean"
Write-Host "BentoPDF submodule: clean"
Write-Host ""
Write-Host "Tauri UI wiring: integrated"
Write-Host "Fully offline release status: not claimed until real Tauri WebView smoke passes"
