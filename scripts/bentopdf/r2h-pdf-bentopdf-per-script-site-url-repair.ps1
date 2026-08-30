[CmdletBinding()]
param(
    [string]$ProjectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path,
    [string]$ExpectedBranch = "feature/bentopdf-integration",
    [string]$ExpectedBentoCommit = "21c924a3e6a7ce28740535a5bc6b74f872fcdcb5",
    [string]$BentoVersion = "2.8.6"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "===== $Message =====" -ForegroundColor Cyan
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $false)][string[]]$Arguments = @(),
        [Parameter(Mandatory = $false)][string]$WorkingDirectory = $ProjectRoot,
        [Parameter(Mandatory = $false)][string]$LogPath
    )

    if (-not (Test-Path -LiteralPath $FilePath)) {
        throw "Executable not found: $FilePath"
    }

    Push-Location $WorkingDirectory

    $previousErrorActionPreference = $ErrorActionPreference

    try {
        if ($LogPath) {
            $logDirectory = Split-Path -Parent $LogPath

            if ($logDirectory) {
                New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
            }
        }

        $ErrorActionPreference = "Continue"

        if ($LogPath) {
            & $FilePath @Arguments 2>&1 |
                ForEach-Object {
                    $text = $_.ToString()
                    Write-Host $text
                    Add-Content -LiteralPath $LogPath -Value $text -Encoding UTF8
                }
        }
        else {
            & $FilePath @Arguments 2>&1 |
                ForEach-Object {
                    Write-Host $_.ToString()
                }
        }

        $nativeExitCode = $LASTEXITCODE
        $ErrorActionPreference = $previousErrorActionPreference

        if ($nativeExitCode -ne 0) {
            throw "Command failed with exit code $nativeExitCode`: $FilePath $($Arguments -join ' ')"
        }
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }
}

function Get-RelativeGitChanges {
    param([string]$Repository)

    $lines = @(& git -C $Repository status --porcelain=v1 --untracked-files=all)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read Git status: $Repository"
    }

    $paths = New-Object System.Collections.Generic.List[string]

    foreach ($line in $lines) {
        if ([string]::IsNullOrWhiteSpace($line) -or $line.Length -lt 4) {
            continue
        }

        $path = $line.Substring(3).Trim()

        if ($path -match "\s+->\s+") {
            $path = ($path -split "\s+->\s+")[-1]
        }

        $path = $path.Trim('"').Replace('\', '/')
        $paths.Add($path)
    }

    return @($paths)
}

function Assert-AllowedParentChanges {
    param([string[]]$Paths)

    $unexpected = @(
        $Paths | Where-Object {
            $_ -ne ".gitmodules" -and
            $_ -ne "thirdparty/bentopdf" -and
            $_ -notlike "scripts/bentopdf/*" -and
            $_ -notlike "audit-output/bentopdf-*"
        }
    )

    if ($unexpected.Count -gt 0) {
        throw "Unexpected parent repository changes detected:`n$($unexpected -join "`n")"
    }
}

function Restore-AllowedSubmoduleGeneratedFile {
    param([string]$SubmoduleDirectory)

    $changes = @(Get-RelativeGitChanges -Repository $SubmoduleDirectory)
    if ($changes.Count -eq 0) {
        return
    }

    $allowed = @("security-headers-docs.conf")
    $unexpected = @($changes | Where-Object { $_ -notin $allowed })

    if ($unexpected.Count -gt 0) {
        throw "Unexpected BentoPDF source changes detected:`n$($unexpected -join "`n")"
    }

    foreach ($path in $changes) {
        & git -C $SubmoduleDirectory restore --source=HEAD --staged --worktree -- $path
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to restore generated tracked file: $path"
        }
    }

    $remaining = @(Get-RelativeGitChanges -Repository $SubmoduleDirectory)
    if ($remaining.Count -gt 0) {
        throw "BentoPDF submodule is still dirty after restoring the permitted generated file."
    }
}

function Set-ProcessEnvironment {
    param([hashtable]$Values)

    foreach ($entry in $Values.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable(
            [string]$entry.Key,
            [string]$entry.Value,
            [EnvironmentVariableTarget]::Process
        )
    }
}

function Remove-ProcessEnvironment {
    param([string[]]$Names)

    foreach ($name in $Names) {
        [Environment]::SetEnvironmentVariable(
            $name,
            $null,
            [EnvironmentVariableTarget]::Process
        )
    }
}

Write-Step "Preflight"

if (-not (Test-Path -LiteralPath $ProjectRoot -PathType Container)) {
    throw "Project root does not exist: $ProjectRoot"
}

$ProjectRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path
$SubmoduleDirectory = Join-Path $ProjectRoot "thirdparty\bentopdf"
$ScriptsDirectory = Join-Path $ProjectRoot "scripts\bentopdf"
$AuditDirectory = Join-Path $ProjectRoot "audit-output\bentopdf-phase1-finalize"
$GeneratedDirectory = Join-Path $ProjectRoot "generated\bentopdf\$BentoVersion-$($ExpectedBentoCommit.Substring(0,8))"
$ManifestPath = Join-Path $ScriptsDirectory "integration-manifest.json"
$BuildScriptPath = Join-Path $ScriptsDirectory "build-bentopdf-windows.ps1"
$BootstrapPath = Join-Path $ScriptsDirectory "bootstrap-bentopdf.ps1"

New-Item -ItemType Directory -Force -Path $ScriptsDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $AuditDirectory | Out-Null

foreach ($command in @("git.exe", "node.exe", "npm.cmd")) {
    $resolved = Get-Command $command -ErrorAction SilentlyContinue
    if (-not $resolved) {
        throw "Required command is not available: $command"
    }
}

$gitTopLevelRaw = (& git -C $ProjectRoot rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "Unable to determine Git repository root: $ProjectRoot"
}

$gitTopLevel = [System.IO.Path]::GetFullPath($gitTopLevelRaw)
$expectedGitRoot = [System.IO.Path]::GetFullPath($ProjectRoot)

$gitTopLevel = $gitTopLevel.TrimEnd([char[]]"\/")
$expectedGitRoot = $expectedGitRoot.TrimEnd([char[]]"\/")

if (-not [string]::Equals(
    $gitTopLevel,
    $expectedGitRoot,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "Not the expected Git repository root. Expected: $expectedGitRoot | Actual: $gitTopLevel"
}

$currentBranch = (& git -C $ProjectRoot branch --show-current).Trim()
if ($currentBranch -ne $ExpectedBranch) {
    throw "Wrong branch. Expected '$ExpectedBranch', found '$currentBranch'."
}

$parentChanges = @(Get-RelativeGitChanges -Repository $ProjectRoot)
Assert-AllowedParentChanges -Paths $parentChanges

if (-not (Test-Path -LiteralPath $SubmoduleDirectory -PathType Container)) {
    throw "BentoPDF submodule directory is missing: $SubmoduleDirectory"
}

$currentBentoCommit = (& git -C $SubmoduleDirectory rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $currentBentoCommit -ne $ExpectedBentoCommit) {
    throw "Wrong BentoPDF commit. Expected $ExpectedBentoCommit, found $currentBentoCommit."
}

Restore-AllowedSubmoduleGeneratedFile -SubmoduleDirectory $SubmoduleDirectory

$nodeVersionText = (& node.exe --version).Trim().TrimStart("v")
$nodeMajor = [int]($nodeVersionText.Split(".")[0])
if ($nodeMajor -lt 18) {
    throw "Node.js 18 or later is required. Found: $nodeVersionText"
}

Write-Host "Project root: $ProjectRoot"
Write-Host "Branch: $currentBranch"
Write-Host "BentoPDF commit: $currentBentoCommit"
Write-Host "Node.js: $nodeVersionText"

Write-Step "Install BentoPDF dependencies"

Invoke-Native `
    -FilePath (Get-Command npm.cmd).Source `
    -Arguments @("ci") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "01-npm-ci.log")

$TscPath = Join-Path $SubmoduleDirectory "node_modules\.bin\tsc.cmd"
$VitePath = Join-Path $SubmoduleDirectory "node_modules\.bin\vite.cmd"
$NodePath = (Get-Command node.exe).Source

Write-Step "Build BentoPDF"

$commonEnvironment = @{
    "SIMPLE_MODE" = "true"
    "BASE_URL" = "/bentopdf/"
    "VITE_USE_CDN" = "false"
    "VITE_DEFAULT_LANGUAGE" = "ar"
    "VITE_APP_NAME" = "R2H PDF"
    "VITE_BRAND_NAME" = "R2H PDF"
    "NODE_OPTIONS" = "--max-old-space-size=3072"
}

Set-ProcessEnvironment -Values $commonEnvironment
Set-ProcessEnvironment -Values @{ "SITE_URL" = "https://www.bentopdf.com" }

Invoke-Native `
    -FilePath $TscPath `
    -Arguments @() `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "02-typescript.log")

Invoke-Native `
    -FilePath $VitePath `
    -Arguments @("build") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "03-vite-build.log")

Write-Step "Generate localized pages with root SITE_URL"

Set-ProcessEnvironment -Values @{ "SITE_URL" = "https://www.bentopdf.com" }

Invoke-Native `
    -FilePath $NodePath `
    -Arguments @("scripts/generate-i18n-pages.mjs") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "04-generate-i18n-pages.log")

Write-Step "Generate sitemap with deployed BentoPDF base path"

Set-ProcessEnvironment -Values @{ "SITE_URL" = "https://www.bentopdf.com/bentopdf" }

Invoke-Native `
    -FilePath $NodePath `
    -Arguments @("scripts/generate-sitemap.mjs") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "05-generate-sitemap.log")

Write-Step "Generate security headers"

Invoke-Native `
    -FilePath $NodePath `
    -Arguments @("scripts/generate-security-headers.mjs") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "06-generate-security-headers.log")

Restore-AllowedSubmoduleGeneratedFile -SubmoduleDirectory $SubmoduleDirectory

Write-Step "Run SEO audit with deployed BentoPDF base path"

Set-ProcessEnvironment -Values @{ "SITE_URL" = "https://www.bentopdf.com/bentopdf" }

Invoke-Native `
    -FilePath $NodePath `
    -Arguments @("scripts/seo-audit.mjs") `
    -WorkingDirectory $SubmoduleDirectory `
    -LogPath (Join-Path $AuditDirectory "07-seo-audit.log")

Write-Step "Validate generated URLs"

$DistDirectory = Join-Path $SubmoduleDirectory "dist"
if (-not (Test-Path -LiteralPath (Join-Path $DistDirectory "index.html") -PathType Leaf)) {
    throw "BentoPDF dist output is missing: $DistDirectory"
}

$doubleBaseMatches = @(
    Get-ChildItem -LiteralPath $DistDirectory -Recurse -File -Filter "*.html" |
        Select-String -SimpleMatch "/bentopdf/bentopdf/" -ErrorAction Stop
)

$doubleBaseReport = Join-Path $AuditDirectory "08-double-base-path-scan.txt"
if ($doubleBaseMatches.Count -gt 0) {
    $doubleBaseMatches |
        ForEach-Object { "$($_.Path):$($_.LineNumber):$($_.Line.Trim())" } |
        Set-Content -LiteralPath $doubleBaseReport -Encoding UTF8

    throw "Double BentoPDF base path references found: $($doubleBaseMatches.Count). See $doubleBaseReport"
}

"Double base-path references: 0" |
    Set-Content -LiteralPath $doubleBaseReport -Encoding UTF8

Write-Step "Publish generated BentoPDF build"

if (Test-Path -LiteralPath $GeneratedDirectory) {
    Remove-Item -LiteralPath $GeneratedDirectory -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $GeneratedDirectory | Out-Null
Copy-Item -Path (Join-Path $DistDirectory "*") -Destination $GeneratedDirectory -Recurse -Force

if (-not (Test-Path -LiteralPath (Join-Path $GeneratedDirectory "index.html") -PathType Leaf)) {
    throw "Published BentoPDF index.html is missing."
}

Write-Step "Create canonical Windows build script"

$canonicalBuildScript = @'
[CmdletBinding()]
param(
    [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$submodule = Join-Path $ProjectRoot "thirdparty\bentopdf"
$node = (Get-Command node.exe -ErrorAction Stop).Source
$npm = (Get-Command npm.cmd -ErrorAction Stop).Source
$tsc = Join-Path $submodule "node_modules\.bin\tsc.cmd"
$vite = Join-Path $submodule "node_modules\.bin\vite.cmd"

function Run {
    param([string]$File, [string[]]$Args = @())
    & $File @Args
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE`: $File $($Args -join ' ')"
    }
}

$env:SIMPLE_MODE = "true"
$env:BASE_URL = "/bentopdf/"
$env:VITE_USE_CDN = "false"
$env:VITE_DEFAULT_LANGUAGE = "ar"
$env:VITE_APP_NAME = "R2H PDF"
$env:VITE_BRAND_NAME = "R2H PDF"
$env:NODE_OPTIONS = "--max-old-space-size=3072"

Push-Location $submodule
try {
    Run $npm @("ci")

    $env:SITE_URL = "https://www.bentopdf.com"
    Run $tsc
    Run $vite @("build")
    Run $node @("scripts/generate-i18n-pages.mjs")

    $env:SITE_URL = "https://www.bentopdf.com/bentopdf"
    Run $node @("scripts/generate-sitemap.mjs")
    Run $node @("scripts/generate-security-headers.mjs")

    git restore --source=HEAD --staged --worktree -- security-headers-docs.conf
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to restore generated security-headers-docs.conf."
    }

    Run $node @("scripts/seo-audit.mjs")
}
finally {
    Pop-Location
}

Write-Host "BENTOPDF WINDOWS BUILD PASSED" -ForegroundColor Green
'@

Set-Content -LiteralPath $BuildScriptPath -Value $canonicalBuildScript -Encoding UTF8

$bootstrapWrapper = @'
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$buildScript = Join-Path $PSScriptRoot "build-bentopdf-windows.ps1"
if (-not (Test-Path -LiteralPath $buildScript -PathType Leaf)) {
    throw "Missing BentoPDF build script: $buildScript"
}

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $buildScript
if ($LASTEXITCODE -ne 0) {
    throw "BentoPDF bootstrap/build failed with exit code $LASTEXITCODE."
}

Write-Host "BENTOPDF SOURCE AND BUILD PHASE PASSED" -ForegroundColor Green
'@

Set-Content -LiteralPath $BootstrapPath -Value $bootstrapWrapper -Encoding UTF8

Write-Step "Write integration manifest"

$distFiles = @(Get-ChildItem -LiteralPath $DistDirectory -Recurse -File)
$distBytes = [int64](($distFiles | Measure-Object -Property Length -Sum).Sum)
$indexHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $DistDirectory "index.html")).Hash

$manifest = [ordered]@{
    schemaVersion = 1
    component = "BentoPDF"
    upstreamRepository = "https://github.com/alam00000/bentopdf.git"
    upstreamCommit = $ExpectedBentoCommit
    upstreamVersion = $BentoVersion
    license = "AGPL-3.0-only"
    baseUrl = "/bentopdf/"
    defaultLanguage = "ar"
    simpleMode = $true
    phase = "source-and-build"
    runtimeOfflineComplete = $false
    runtimeOfflineNote = "Advanced WASM and OCR assets are handled in Phase 2."
    generatedDirectory = "generated/bentopdf/$BentoVersion-$($ExpectedBentoCommit.Substring(0,8))"
    generatedFileCount = $distFiles.Count
    generatedBytes = $distBytes
    generatedIndexSha256 = $indexHash
    seoAuditPassed = $true
    doubleBasePathReferences = 0
    generatedAtUtc = [DateTime]::UtcNow.ToString("o")
}

$manifest |
    ConvertTo-Json -Depth 8 |
    Set-Content -LiteralPath $ManifestPath -Encoding UTF8

Write-Step "Final Git validation and commit"

Restore-AllowedSubmoduleGeneratedFile -SubmoduleDirectory $SubmoduleDirectory

$finalParentChanges = @(Get-RelativeGitChanges -Repository $ProjectRoot)
Assert-AllowedParentChanges -Paths $finalParentChanges

& git -C $ProjectRoot add -- .gitmodules thirdparty/bentopdf scripts/bentopdf
if ($LASTEXITCODE -ne 0) {
    throw "Unable to stage BentoPDF integration files."
}

$stagedNames = @(& git -C $ProjectRoot diff --cached --name-only)
if ($LASTEXITCODE -ne 0) {
    throw "Unable to inspect staged changes."
}

if ($stagedNames.Count -eq 0) {
    Write-Host "No new source changes require a commit."
}
else {
    $unexpectedStaged = @(
        $stagedNames | Where-Object {
            $_ -ne ".gitmodules" -and
            $_ -ne "thirdparty/bentopdf" -and
            $_ -notlike "scripts/bentopdf/*"
        }
    )

    if ($unexpectedStaged.Count -gt 0) {
        throw "Unexpected staged files detected:`n$($unexpectedStaged -join "`n")"
    }

    & git -C $ProjectRoot commit -m "feat(pdf): integrate pinned BentoPDF source and Windows build pipeline"
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to create BentoPDF integration commit."
    }
}

$finalCommit = (& git -C $ProjectRoot rev-parse HEAD).Trim()
$finalBranch = (& git -C $ProjectRoot branch --show-current).Trim()
$finalBentoCommit = (& git -C $SubmoduleDirectory rev-parse HEAD).Trim()
$remainingParentChanges = @(Get-RelativeGitChanges -Repository $ProjectRoot)
$remainingSubmoduleChanges = @(Get-RelativeGitChanges -Repository $SubmoduleDirectory)

$summary = [ordered]@{
    result = "PASS"
    projectRoot = $ProjectRoot
    branch = $finalBranch
    parentCommit = $finalCommit
    bentoCommit = $finalBentoCommit
    parentWorkingTreeClean = ($remainingParentChanges.Count -eq 0)
    submoduleWorkingTreeClean = ($remainingSubmoduleChanges.Count -eq 0)
    seoAuditPassed = $true
    doubleBasePathReferences = 0
    manifest = $ManifestPath
    generatedDirectory = $GeneratedDirectory
}

$summaryPath = Join-Path $AuditDirectory "summary.json"
$summary |
    ConvertTo-Json -Depth 6 |
    Set-Content -LiteralPath $summaryPath -Encoding UTF8

if ($finalBranch -ne $ExpectedBranch) {
    throw "Final branch verification failed."
}

if ($finalBentoCommit -ne $ExpectedBentoCommit) {
    throw "Final BentoPDF commit verification failed."
}

if ($remainingParentChanges.Count -gt 0) {
    throw "Parent working tree is not clean:`n$($remainingParentChanges -join "`n")"
}

if ($remainingSubmoduleChanges.Count -gt 0) {
    throw "BentoPDF submodule is not clean:`n$($remainingSubmoduleChanges -join "`n")"
}

Remove-ProcessEnvironment -Names @(
    "SIMPLE_MODE",
    "BASE_URL",
    "VITE_USE_CDN",
    "VITE_DEFAULT_LANGUAGE",
    "VITE_APP_NAME",
    "VITE_BRAND_NAME",
    "NODE_OPTIONS",
    "SITE_URL"
)

Write-Host ""
Write-Host "Per-script SITE_URL model passed." -ForegroundColor Green
Write-Host "Double base-path references: 0" -ForegroundColor Green
Write-Host "SEO audit: passed" -ForegroundColor Green
Write-Host "BentoPDF commit: $finalBentoCommit"
Write-Host "Parent commit: $finalCommit"
Write-Host "Parent working tree: clean"
Write-Host "BentoPDF submodule: clean"
Write-Host "Summary: $summaryPath"
Write-Host ""
Write-Host "BENTOPDF PER-SCRIPT SITE_URL REPAIR PASSED" -ForegroundColor Green



