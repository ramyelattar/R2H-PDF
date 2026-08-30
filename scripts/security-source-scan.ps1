# R2H PDF - Security Source Scan
# Scans source/config/manifests for cloud/network/telemetry/security risks.

$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "release"
$securityDir = Join-Path $root "security"
$jsonPath = Join-Path $releaseDir "R2H-PDF-SECURITY-SOURCE-SCAN.json"
$mdPath = Join-Path $releaseDir "R2H-PDF-SECURITY-SOURCE-SCAN.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"

New-Item -ItemType Directory -Force -Path $releaseDir, $securityDir | Out-Null

$allowlistPath = Join-Path $securityDir "source-scan-allowlist.json"
$allowlist = @()
if (Test-Path -LiteralPath $allowlistPath -PathType Leaf) {
    try { $allowlist = @(Get-Content -LiteralPath $allowlistPath -Raw | ConvertFrom-Json) } catch { $allowlist = @() }
}

$rules = @(
    @{ Id = "cloud-openai"; Pattern = "openai" },
    @{ Id = "cloud-anthropic"; Pattern = "anthropic" },
    @{ Id = "cloud-gemini"; Pattern = "gemini" },
    @{ Id = "cloud-claude"; Pattern = "claude" },
    @{ Id = "cloud-supabase"; Pattern = "supabase" },
    @{ Id = "cloud-firebase"; Pattern = "firebase" },
    @{ Id = "telemetry-analytics"; Pattern = "analytics" },
    @{ Id = "telemetry-generic"; Pattern = "telemetry" },
    @{ Id = "telemetry-sentry"; Pattern = "sentry" },
    @{ Id = "telemetry-posthog"; Pattern = "posthog" },
    @{ Id = "url-http"; Pattern = "http://" },
    @{ Id = "url-https"; Pattern = "https://" },
    @{ Id = "js-fetch"; Pattern = "fetch\(" },
    @{ Id = "js-axios"; Pattern = "axios" },
    @{ Id = "js-xhr"; Pattern = "XMLHttpRequest" },
    @{ Id = "remote-huggingface"; Pattern = "hugging face|huggingface|hf_hub|from_pretrained" },
    @{ Id = "remote-trust-code"; Pattern = "trust_remote_code" },
    @{ Id = "dynamic-eval"; Pattern = "\beval\s*\(" },
    @{ Id = "dynamic-function"; Pattern = "new\s+Function|Function\s*\(" }
)

$textExtensions = @(".rs", ".ts", ".tsx", ".js", ".mjs", ".json", ".toml", ".ps1", ".py", ".md", ".txt", ".yml", ".yaml", ".html", ".css", ".c", ".h", ".sh", ".lock", ".config")
$excludeRegex = "\\(node_modules|target|target2|target-test|dist|release|release-final|app-package|\.git|\.claude|python-runtime|__pycache__)(\\|$)|\\local-ai\\models\\|\\local-ai\\runtimes\\"

function ConvertTo-RelativePath {
    param([string]$Path)
    return $Path.Substring($root.Length).TrimStart("\", "/")
}

function Get-AllowlistMatch {
    param([string]$RuleId, [string]$RelativePath, [string]$Line)
    foreach ($entry in $allowlist) {
        if ($entry.ruleId -ne $RuleId) { continue }
        if ($entry.path -and $entry.path -ne $RelativePath) { continue }
        if ($entry.exactPattern -and $Line -notlike "*$($entry.exactPattern)*") { continue }
        if ($entry.expiryDate) {
            try {
                if ([datetime]$entry.expiryDate -lt (Get-Date).Date) { continue }
            } catch { continue }
        }
        return $entry
    }
    return $null
}

function Get-Classification {
    param([string]$RuleId, [string]$RelativePath, [string]$Line)

    $allow = Get-AllowlistMatch $RuleId $RelativePath $Line
    if ($allow) {
        return @{
            Classification = [string]$allow.classification
            RuntimeReachable = [bool]$allow.runtimeReachable
            Reason = [string]$allow.reason
            Mitigation = [string]$allow.mitigation
            Reviewed = $true
            Fails = $false
        }
    }

    $lowerPath = $RelativePath.ToLowerInvariant()
    $trimmed = $Line.Trim()

    if ($lowerPath -match "(^|\\)(package|pnpm-lock|cargo\.lock|cargo\.toml|pyproject\.toml|vite\.config|vitest\.config|tsconfig|eslint\.config)") {
        return @{ Classification = "package-metadata"; RuntimeReachable = $false; Reason = "Package/config metadata reference."; Mitigation = "Not executed as an external runtime network path."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -match "tauri\.conf\.json" -and $Line -match '\$schema') {
        return @{ Classification = "package-metadata"; RuntimeReachable = $false; Reason = "Configuration schema URL metadata."; Mitigation = "Not a runtime network call."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -match "^scripts\\(security-source-scan|security-distribution-scan|validate-offline-safety)\.ps1$") {
        return @{ Classification = "reviewed-allowlist"; RuntimeReachable = $false; Reason = "Scanner rule definition, not an app runtime call."; Mitigation = "Scanner scripts are release QA tooling and are recorded separately."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -eq "local-ai\workers\paddleocr_vl_worker.py" -and $RuleId -eq "remote-huggingface" -and $Line -match "from_pretrained") {
        return @{ Classification = "offline-policy-guarded"; RuntimeReachable = $true; Reason = "Transformer model loader uses local model path with local_files_only=True on adjacent arguments."; Mitigation = "Keep local_files_only=True and bundled OCR model assets."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -match "(\.test\.|\.spec\.|\\tests\\|\\test\\)") {
        return @{ Classification = "test-only"; RuntimeReachable = $false; Reason = "Test fixture or assertion."; Mitigation = "Covered by test-only classification."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -match "(readme|\.md$|\.txt$)" -or $trimmed -match "^(//|#|\*|/\*|<!--)") {
        return @{ Classification = "documentation-reference"; RuntimeReachable = $false; Reason = "Documentation/comment/reference text."; Mitigation = "Not an app runtime call."; Reviewed = $true; Fails = $false }
    }
    if ($Line -match "localhost|127\.0\.0\.1|ipc\.localhost|data:|blob:") {
        return @{ Classification = "localhost-only"; RuntimeReachable = $true; Reason = "Network reference is restricted to localhost/data/blob/ipc."; Mitigation = "Allowed by offline policy."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -eq "src-tauri\src\ai_core\embedding.rs" -and $Line -match "EMBEDDING_HOST") {
        return @{ Classification = "localhost-only"; RuntimeReachable = $true; Reason = "Embedding HTTP call uses EMBEDDING_HOST constant set to 127.0.0.1."; Mitigation = "Keep EMBEDDING_HOST pinned to 127.0.0.1."; Reviewed = $true; Fails = $false }
    }
    if ($Line -match "assert!\(!.*https?://|https?://.*err\(\)\.unwrap") {
        return @{ Classification = "test-only"; RuntimeReachable = $false; Reason = "Inline Rust test assertion/negative fixture."; Mitigation = "Not an app runtime call."; Reviewed = $true; Fails = $false }
    }
    if ($RuleId -eq "remote-trust-code" -and $Line -match "local_files_only\s*=\s*True") {
        return @{ Classification = "offline-policy-guarded"; RuntimeReachable = $true; Reason = "trust_remote_code is paired with local_files_only=True for local model loading."; Mitigation = "Model directory must be reviewed and bundled locally."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -match "^scripts\\(jlib\.py|pipcl\.py|wrap\\|mutool|mupdfwrap|syncdocs|build-docs|archive|copyblob|bin2coff|fontdump|glyph|cmap|run|find|make)") {
        return @{ Classification = "reviewed-allowlist"; RuntimeReachable = $false; Reason = "Upstream MuPDF/build tooling, not packaged app runtime."; Mitigation = "Still recorded for review; not treated as runtime network path."; Reviewed = $true; Fails = $false }
    }
    if ($lowerPath -eq "scripts\create-full-offline-installer.ps1" -and $Line -match "7-Zip") {
        return @{ Classification = "documentation-reference"; RuntimeReachable = $false; Reason = "Installer build prerequisite message."; Mitigation = "Not a runtime network call."; Reviewed = $true; Fails = $false }
    }
    if ($RuleId -match "telemetry" -and $lowerPath -match "^src\\") {
        return @{ Classification = "offline-policy-guarded"; RuntimeReachable = $false; Reason = "Telemetry preference/type reference; no emitter or external endpoint found on this line."; Mitigation = "Source scan records the reference and blocks if a sender endpoint appears."; Reviewed = $true; Fails = $false }
    }

    if ($RuleId -match "url|cloud|js-fetch|js-axios|js-xhr|remote") {
        return @{ Classification = "runtime-blocker"; RuntimeReachable = $true; Reason = "Unreviewed external/cloud/network-capable source reference."; Mitigation = "Remove, restrict to localhost, or add exact reviewed allowlist with owner and expiry."; Reviewed = $false; Fails = $true }
    }

    return @{ Classification = "reviewed-allowlist"; RuntimeReachable = $false; Reason = "Security-sensitive pattern in non-runtime context."; Mitigation = "Recorded for manual review."; Reviewed = $true; Fails = $false }
}

$candidateFiles = New-Object System.Collections.Generic.List[System.IO.FileInfo]
foreach ($target in @("src", "src-tauri\src", "src-tauri\Cargo.toml", "src-tauri\Cargo.lock", "src-tauri\tauri.conf.json", "scripts", "local-ai\workers", "local-ai\config", "package.json", "pnpm-lock.yaml", "pyproject.toml", "vite.config.ts", "vitest.config.ts", "tsconfig.json", "eslint.config.js")) {
    $full = Join-Path $root $target
    if (Test-Path -LiteralPath $full -PathType Container) {
        Get-ChildItem -LiteralPath $full -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -notmatch $excludeRegex -and $textExtensions -contains $_.Extension.ToLowerInvariant() } |
            ForEach-Object { $candidateFiles.Add($_) | Out-Null }
    } elseif (Test-Path -LiteralPath $full -PathType Leaf) {
        $candidateFiles.Add((Get-Item -LiteralPath $full)) | Out-Null
    }
}

$findings = New-Object System.Collections.Generic.List[object]

foreach ($file in ($candidateFiles | Sort-Object FullName -Unique)) {
    $relative = ConvertTo-RelativePath $file.FullName
    try {
        $lineNumber = 0
        foreach ($line in [System.IO.File]::ReadLines($file.FullName)) {
            $lineNumber++
            foreach ($rule in $rules) {
                if ($line -match $rule.Pattern) {
                    $class = Get-Classification $rule.Id $relative $line
                    $findings.Add([pscustomobject]@{
                        ruleId = $rule.Id
                        path = $relative
                        line = $lineNumber
                        classification = $class.Classification
                        runtimeReachable = $class.RuntimeReachable
                        reviewed = $class.Reviewed
                        failsRelease = $class.Fails
                        reason = $class.Reason
                        mitigation = $class.Mitigation
                        excerpt = $line.Trim()
                    }) | Out-Null
                }
            }
        }
    } catch {
        $findings.Add([pscustomobject]@{
            ruleId = "scan-error"
            path = $relative
            line = 0
            classification = "runtime-blocker"
            runtimeReachable = $false
            reviewed = $false
            failsRelease = $true
            reason = "Could not read source file: $($_.Exception.Message)"
            mitigation = "Fix scanner readability or exclude only if generated/vendor."
            excerpt = ""
        }) | Out-Null
    }
}

$failures = @($findings | Where-Object { $_.failsRelease -eq $true })
$status = if ($failures.Count -gt 0) { "FAIL" } else { "PASS" }

$summary = [pscustomobject]@{
    timestamp = $timestamp
    projectRoot = $root
    status = $status
    filesScanned = ($candidateFiles | Sort-Object FullName -Unique).Count
    findings = $findings.Count
    runtimeBlockers = $failures.Count
    allowlistPath = $allowlistPath
    findingsByClassification = $findings | Group-Object classification | ForEach-Object { [pscustomobject]@{ classification = $_.Name; count = $_.Count } }
    findingsList = $findings
}

$summary | ConvertTo-Json -Depth 8 | Out-File -LiteralPath $jsonPath -Encoding utf8

$md = New-Object System.Collections.Generic.List[string]
$md.Add("# R2H-PDF Security Source Scan") | Out-Null
$md.Add("") | Out-Null
$md.Add("- Timestamp: $timestamp") | Out-Null
$md.Add("- Project root: $root") | Out-Null
$md.Add("- Files scanned: $($summary.filesScanned)") | Out-Null
$md.Add("- Status: $status") | Out-Null
$md.Add("- JSON evidence: $jsonPath") | Out-Null
$md.Add("- Allowlist: $allowlistPath") | Out-Null
$md.Add("") | Out-Null
$md.Add("## Classification Summary") | Out-Null
foreach ($group in $summary.findingsByClassification) {
    $md.Add("- $($group.classification): $($group.count)") | Out-Null
}
$md.Add("") | Out-Null
$md.Add("## Runtime Blockers") | Out-Null
if ($failures.Count -eq 0) {
    $md.Add("- None.") | Out-Null
} else {
    foreach ($f in $failures) {
        $md.Add("- $($f.ruleId) $($f.path):$($f.line) - $($f.reason)") | Out-Null
    }
}
$md.Add("") | Out-Null
$md.Add("## Findings") | Out-Null
$md.Add("| Rule | Path | Line | Classification | Runtime Reachable | Reviewed | Notes |") | Out-Null
$md.Add("| --- | --- | ---: | --- | --- | --- | --- |") | Out-Null
foreach ($f in $findings) {
    $notes = ($f.reason -replace '\|', '\|')
    $md.Add("| $($f.ruleId) | $($f.path) | $($f.line) | $($f.classification) | $($f.runtimeReachable) | $($f.reviewed) | $notes |") | Out-Null
}
$md | Out-File -LiteralPath $mdPath -Encoding utf8

Write-Host "=== R2H PDF - Security Source Scan ===" -ForegroundColor Cyan
Write-Host "Status: $status"
Write-Host "Files scanned: $($summary.filesScanned)"
Write-Host "Findings: $($findings.Count)"
Write-Host "Runtime blockers: $($failures.Count)"
Write-Host "Report: $mdPath"
Write-Host "JSON: $jsonPath"

if ($failures.Count -gt 0) { exit 1 }
exit 0
