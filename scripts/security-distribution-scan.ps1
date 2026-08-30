# R2H PDF - Security Distribution Scan
# Scans packaged/release output for stale artifacts, secrets, external endpoints, and unsafe references.

$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "release"
$jsonPath = Join-Path $releaseDir "R2H-PDF-SECURITY-DISTRIBUTION-SCAN.json"
$mdPath = Join-Path $releaseDir "R2H-PDF-SECURITY-DISTRIBUTION-SCAN.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"

New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

$targets = @(
    (Join-Path $root "release"),
    (Join-Path $root "release\v2.1.0-beta"),
    (Join-Path $root "release\v2.1.0-beta\full-offline-installer"),
    (Join-Path $root "src-tauri\target\release")
) | Where-Object { Test-Path -LiteralPath $_ -PathType Container } | Sort-Object -Unique

$textExtensions = @(".txt", ".md", ".ps1", ".json", ".iss", ".yml", ".yaml", ".toml", ".ini", ".config", ".xml", ".html", ".css", ".js", ".py", ".rs", ".lock")
$maxTextBytes = 5MB

$rules = @(
    @{ Id = "stale-v020"; Pattern = "v0\.2\.0|0\.2\.0|r2h-pdf_0\.2\.0|R2H-PDF-v0\.2\.0"; Fail = $true },
    @{ Id = "secret-openai"; Pattern = "sk-[A-Za-z0-9_-]{20,}"; Fail = $true },
    @{ Id = "secret-generic"; Pattern = "(api[_-]?key|token|secret)\s*[:=]\s*['""][^'""]{16,}['""]"; Fail = $true },
    @{ Id = "cloud-openai"; Pattern = "openai"; Fail = $true },
    @{ Id = "cloud-anthropic"; Pattern = "anthropic"; Fail = $true },
    @{ Id = "cloud-gemini"; Pattern = "gemini"; Fail = $true },
    @{ Id = "cloud-supabase"; Pattern = "supabase"; Fail = $true },
    @{ Id = "cloud-firebase"; Pattern = "firebase"; Fail = $true },
    @{ Id = "telemetry"; Pattern = "analytics|telemetry|sentry|posthog"; Fail = $true },
    @{ Id = "external-url"; Pattern = "https?://"; Fail = $true },
    @{ Id = "external-ai-provider"; Pattern = "hugging ?face|huggingface|claude|openai|anthropic|gemini"; Fail = $true }
)

function ConvertTo-RelativePath {
    param([string]$Path)
    return $Path.Substring($root.Length).TrimStart("\", "/")
}

function Get-DistributionClassification {
    param([string]$RuleId, [string]$RelativePath, [string]$Line)
    $lower = $RelativePath.ToLowerInvariant()
    $trimmed = $Line.Trim()

    if ($RuleId -eq "stale-v020") {
        return @{ Classification = "runtime-blocker"; RuntimeReachable = $true; Reason = "Stale v0.2.0 artifact/reference."; Fails = $true }
    }
    if ($RuleId -match "secret") {
        return @{ Classification = "runtime-blocker"; RuntimeReachable = $true; Reason = "Potential raw token/secret in distribution."; Fails = $true }
    }
    if ($Line -match "localhost|127\.0\.0\.1|ipc\.localhost|data:|blob:") {
        return @{ Classification = "localhost-only"; RuntimeReachable = $true; Reason = "Localhost/data/blob/ipc reference."; Fails = $false }
    }
    if ($lower -match "^src-tauri\\target\\release\\(build|\.fingerprint)\\" -or $lower -match "^src-tauri\\target\\release\\bundle\\") {
        return @{ Classification = "package-metadata"; RuntimeReachable = $false; Reason = "Generated Cargo/Tauri build metadata, not shipped runtime source."; Fails = $false }
    }
    if ($lower -match "local-ai\\models\\ocr\\paddleocr-vl\\image_processing_paddleocr_vl\.py$" -and $Line -match "OPENAI_CLIP_(MEAN|STD)") {
        return @{ Classification = "reviewed-allowlist"; RuntimeReachable = $false; Reason = "PaddleOCR-VL CLIP normalization constant name; no OpenAI API call, token, or network path."; Fails = $false }
    }
    if ($lower -match "local-ai\\models\\ocr\\paddleocr-vl\\modeling_paddleocr_vl\.py$" -and $RuleId -eq "external-url") {
        return @{ Classification = "documentation-reference"; RuntimeReachable = $false; Reason = "Documentation/comment URL inside required local PaddleOCR-VL model implementation."; Fails = $false }
    }
    if ($lower -match "(readme|\.md$|\.txt$|license|copying|notice)" -or $trimmed -match "^(//|#|\*|/\*|<!--)") {
        return @{ Classification = "documentation-reference"; RuntimeReachable = $false; Reason = "Documentation or comment in bundled artifact."; Fails = $false }
    }
    if ($lower -match "(package|cargo\.lock|pyproject|tokenizer|config|generation_config|preprocessor_config|processor_config|special_tokens|added_tokens)") {
        return @{ Classification = "package-metadata"; RuntimeReachable = $false; Reason = "Model/package metadata reference."; Fails = $false }
    }
    return @{ Classification = "runtime-blocker"; RuntimeReachable = $true; Reason = "Unreviewed external/cloud/telemetry reference in distribution."; Fails = $true }
}

$findings = New-Object System.Collections.Generic.List[object]
$binaryInventory = New-Object System.Collections.Generic.List[object]
$visitedFiles = New-Object "System.Collections.Generic.HashSet[string]"
$scannedFiles = 0
$skippedLargeText = 0

if ($targets.Count -eq 0) {
    $summary = [pscustomobject]@{
        timestamp = $timestamp
        projectRoot = $root
        status = "SKIPPED"
        reason = "No packaged/release output folder exists."
        targets = @()
        filesScanned = 0
        findings = 0
        runtimeBlockers = 0
        binaryInventory = @()
        findingsList = @()
    }
    $summary | ConvertTo-Json -Depth 8 | Out-File -LiteralPath $jsonPath -Encoding utf8
    "# R2H-PDF Security Distribution Scan`n`n- Timestamp: $timestamp`n- Status: SKIPPED`n- Reason: No packaged/release output folder exists.`n- JSON evidence: $jsonPath" | Out-File -LiteralPath $mdPath -Encoding utf8
    Write-Host "DISTRIBUTION SCAN STATUS: SKIPPED"
    exit 0
}

foreach ($target in $targets) {
    Get-ChildItem -LiteralPath $target -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
        $file = $_
        $relative = ConvertTo-RelativePath $file.FullName
        $fullKey = $file.FullName.ToLowerInvariant()

        if (-not $visitedFiles.Add($fullKey)) {
            return
        }

        if ($relative -match "^release\\(logs\\|smoke-results\\|R2H-PDF-SECURITY-|R2H-PDF-VALIDATION-|R2H-PDF-DISTRIBUTION-BLOCKER-|R2H-PDF-OFFLINE-PROOF|R2H-PDF-RELEASE-PROOF|R2H-PDF-KNOWN-LIMITATIONS|R2H-PDF-INSTALL-GUIDE)") {
            return
        }

        if ($file.Name -match "v0\.2\.0|0\.2\.0|r2h-pdf_0\.2\.0|R2H-PDF-v0\.2\.0") {
            $findings.Add([pscustomobject]@{
                ruleId = "stale-v020-filename"; path = $relative; line = 0; classification = "runtime-blocker";
                runtimeReachable = $true; failsRelease = $true; reason = "Stale v0.2.0 filename."; excerpt = $file.Name
            }) | Out-Null
        }

        if ($textExtensions -notcontains $file.Extension.ToLowerInvariant()) {
            if ($file.Extension.ToLowerInvariant() -in @(".exe", ".msi", ".bin", ".dll", ".gguf", ".safetensors", ".onnx", ".pyd", ".zip")) {
                $binaryInventory.Add([pscustomobject]@{
                    path = $relative
                    bytes = $file.Length
                    lastWriteTime = $file.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss K")
                    note = "Binary payload inventoried by filename/size/timestamp; deep binary string extraction not performed in Phase 36.1."
                }) | Out-Null
            }
            return
        }

        if ($file.Length -gt $maxTextBytes) {
            $skippedLargeText++
            $binaryInventory.Add([pscustomobject]@{
                path = $relative
                bytes = $file.Length
                lastWriteTime = $file.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss K")
                note = "Text-like file exceeded 5MB scanner limit; filename/size/timestamp recorded."
            }) | Out-Null
            return
        }

        $scannedFiles++
        try {
            $lineNumber = 0
            foreach ($line in [System.IO.File]::ReadLines($file.FullName)) {
                $lineNumber++
                foreach ($rule in $rules) {
                    if ($line -match $rule.Pattern) {
                        $class = Get-DistributionClassification $rule.Id $relative $line
                        $findings.Add([pscustomobject]@{
                            ruleId = $rule.Id
                            path = $relative
                            line = $lineNumber
                            classification = $class.Classification
                            runtimeReachable = $class.RuntimeReachable
                            failsRelease = $class.Fails
                            reason = $class.Reason
                            excerpt = $line.Trim()
                        }) | Out-Null
                    }
                }
            }
        } catch {
            $findings.Add([pscustomobject]@{
                ruleId = "scan-error"; path = $relative; line = 0; classification = "runtime-blocker";
                runtimeReachable = $false; failsRelease = $true; reason = "Could not read distribution text file: $($_.Exception.Message)"; excerpt = ""
            }) | Out-Null
        }
    }
}

$failures = @($findings | Where-Object { $_.failsRelease -eq $true })
$status = if ($failures.Count -gt 0) { "FAIL" } else { "PASS" }

$summary = [pscustomobject]@{
    timestamp = $timestamp
    projectRoot = $root
    status = $status
    targets = $targets
    filesScanned = $scannedFiles
    skippedLargeTextFiles = $skippedLargeText
    findings = $findings.Count
    runtimeBlockers = $failures.Count
    binaryInventory = $binaryInventory
    findingsByClassification = $findings | Group-Object classification | ForEach-Object { [pscustomobject]@{ classification = $_.Name; count = $_.Count } }
    findingsList = $findings
}

$summary | ConvertTo-Json -Depth 8 | Out-File -LiteralPath $jsonPath -Encoding utf8

$md = New-Object System.Collections.Generic.List[string]
$md.Add("# R2H-PDF Security Distribution Scan") | Out-Null
$md.Add("") | Out-Null
$md.Add("- Timestamp: $timestamp") | Out-Null
$md.Add("- Project root: $root") | Out-Null
$md.Add("- Status: $status") | Out-Null
$md.Add("- Targets: $($targets -join '; ')") | Out-Null
$md.Add("- Text files scanned: $scannedFiles") | Out-Null
$md.Add("- Large text-like files skipped: $skippedLargeText") | Out-Null
$md.Add("- Binary payload inventory count: $($binaryInventory.Count)") | Out-Null
$md.Add("- JSON evidence: $jsonPath") | Out-Null
$md.Add("") | Out-Null
$md.Add("## Runtime Blockers") | Out-Null
if ($failures.Count -eq 0) { $md.Add("- None.") | Out-Null } else {
    foreach ($f in $failures) { $md.Add("- $($f.ruleId) $($f.path):$($f.line) - $($f.reason)") | Out-Null }
}
$md.Add("") | Out-Null
$md.Add("## Findings") | Out-Null
$md.Add("| Rule | Path | Line | Classification | Runtime Reachable | Fails Release | Notes |") | Out-Null
$md.Add("| --- | --- | ---: | --- | --- | --- | --- |") | Out-Null
foreach ($f in $findings) {
    $notes = ($f.reason -replace '\|', '\|')
    $md.Add("| $($f.ruleId) | $($f.path) | $($f.line) | $($f.classification) | $($f.runtimeReachable) | $($f.failsRelease) | $notes |") | Out-Null
}
$md.Add("") | Out-Null
$md.Add("## Binary Payload Inventory") | Out-Null
$md.Add("| Path | Bytes | Last Write Time | Note |") | Out-Null
$md.Add("| --- | ---: | --- | --- |") | Out-Null
foreach ($b in $binaryInventory) {
    $note = ($b.note -replace '\|', '\|')
    $md.Add("| $($b.path) | $($b.bytes) | $($b.lastWriteTime) | $note |") | Out-Null
}
$md | Out-File -LiteralPath $mdPath -Encoding utf8

Write-Host "=== R2H PDF - Security Distribution Scan ===" -ForegroundColor Cyan
Write-Host "Status: $status"
Write-Host "Targets: $($targets -join '; ')"
Write-Host "Text files scanned: $scannedFiles"
Write-Host "Findings: $($findings.Count)"
Write-Host "Runtime blockers: $($failures.Count)"
Write-Host "Report: $mdPath"
Write-Host "JSON: $jsonPath"

if ($failures.Count -gt 0) { exit 1 }
exit 0
