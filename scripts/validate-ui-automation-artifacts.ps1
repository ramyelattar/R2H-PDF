$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportDir = Join-Path $root "release\smoke-results"
$jsonPath = Join-Path $reportDir "full-ui-workflow-smoke.json"
$artifactDir = Join-Path $reportDir "ui-automation"
$reportPath = Join-Path $reportDir "ui-automation-artifacts-validation.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $reportDir, $artifactDir | Out-Null

$status = "FAIL"
$reason = ""
$workflowCount = 0
$backendHookCountedAsUi = $false
$screenshots = @()
$traces = @()

if (-not (Test-Path -LiteralPath $jsonPath -PathType Leaf)) {
    $reason = "Full UI workflow JSON report is missing: $jsonPath"
} else {
    try {
        $data = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
        $workflowCount = @($data.workflowsValidated).Count
        $backendHookCountedAsUi = [bool]$data.backendHookCountedAsUi
        $screenshots = @($data.screenshots)
        $traces = @($data.traces)
        if ($data.status -eq "PASS" -and $workflowCount -ge 15 -and -not $backendHookCountedAsUi -and ($screenshots.Count -gt 0 -or $traces.Count -gt 0)) {
            $status = "PASS"
        } else {
            $reason = "UI automation did not record a PASS with 15+ installed-app workflows, screenshots/traces, and backendHookCountedAsUi=false."
        }
    } catch {
        $reason = "Could not parse ${jsonPath}: $($_.Exception.Message)"
    }
}

@"
# UI Automation Artifacts Validation

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Full UI workflow JSON: $jsonPath
- Artifact directory: $artifactDir
- Workflow count: $workflowCount
- Backend hook counted as UI: $backendHookCountedAsUi
- Screenshot count: $($screenshots.Count)
- Trace count: $($traces.Count)
- Reason: $reason
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "UI automation artifacts validation: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
