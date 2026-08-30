$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\signature-verification.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
$exe = Join-Path $root "src-tauri\target\release\r2h-pdf.exe"
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\r2h-pdf_2.1.0_x64-setup.exe"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

$cert = $null
$certSource = "none"
if ($env:R2H_SIGN_CERT_THUMBPRINT) {
    $thumbprint = ($env:R2H_SIGN_CERT_THUMBPRINT -replace '\s', '').ToUpperInvariant()
    $cert = Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
        Where-Object { ($_.Thumbprint -replace '\s', '').ToUpperInvariant() -eq $thumbprint -and $_.HasPrivateKey } |
        Select-Object -First 1
    $certSource = "certificate store thumbprint $thumbprint"
}

if (-not $cert) {
    @"
# Signature Verification

- Timestamp: $timestamp
- Project root: $root
- Status: BLOCKED_BY_MISSING_CERTIFICATE
- Signing command: Set-AuthenticodeSignature
- Certificate source: $certSource
- Executable: $exe
- Installer: $installer
- Required certificate: Windows Authenticode code-signing certificate with private key available to the build agent.
- Required variable: R2H_SIGN_CERT_THUMBPRINT pointing to a CurrentUser or LocalMachine personal certificate with private key.
- Reason: No usable signing certificate was configured. Unsigned artifacts must not be marked as signed.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8
    Write-Host "Signing blocked by missing certificate ($reportPath)"
    exit 6
}

$targets = @($exe, $installer) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }
$signResults = foreach ($target in $targets) {
    if ($env:R2H_TIMESTAMP_SERVER) {
        Set-AuthenticodeSignature -FilePath $target -Certificate $cert -TimestampServer $env:R2H_TIMESTAMP_SERVER
    } else {
        Set-AuthenticodeSignature -FilePath $target -Certificate $cert
    }
}
$invalid = @($signResults | Where-Object { $_.Status -ne "Valid" })
$status = if ($invalid.Count -eq 0 -and $targets.Count -eq 2) { "PASS" } else { "FAIL" }

@"
# Signature Verification

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Signing command: Set-AuthenticodeSignature
- Certificate source: $certSource
- Executable: $exe
- Installer: $installer
- Results:
$($signResults | ForEach-Object { "- $($_.Path): $($_.Status) $($_.StatusMessage)" } | Out-String)
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Signing result: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
