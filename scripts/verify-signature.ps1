$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\signature-verification.md"
$exe = Join-Path $root "src-tauri\target\release\r2h-pdf.exe"
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\r2h-pdf_2.1.0_x64-setup.exe"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

$exeSig = if (Test-Path -LiteralPath $exe) { Get-AuthenticodeSignature -LiteralPath $exe } else { $null }
$installerSig = if (Test-Path -LiteralPath $installer) { Get-AuthenticodeSignature -LiteralPath $installer } else { $null }
$status = if ($exeSig -and $installerSig -and $exeSig.Status -eq "Valid" -and $installerSig.Status -eq "Valid") { "PASS" } else { "BLOCKED_BY_MISSING_CERTIFICATE" }

@"
# Signature Verification

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Executable: $exe
- Executable signature: $($exeSig.Status)
- Executable signer: $($exeSig.SignerCertificate.Subject)
- Installer: $installer
- Installer signature: $($installerSig.Status)
- Installer signer: $($installerSig.SignerCertificate.Subject)
- Verification command: Get-AuthenticodeSignature
- Blocker if any: $(if ($status -eq "PASS") { "none" } else { "No valid Authenticode signature found on executable and installer." })
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Signature verification: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 6
