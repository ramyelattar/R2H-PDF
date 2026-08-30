[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$ExpectedRoot = $Root.Replace("\", "/")
$ExpectedBranch = "feature/bentopdf-integration"

$RequiredIntegrationAncestor = "74c9ee6"
$ExpectedBentoCommit = "21c924a3e6a7ce28740535a5bc6b74f872fcdcb5"
$ExpectedBentoVersion = "2.8.6"

$PyMuPdfVersion = "0.11.16"
$GhostscriptVersion = "0.1.1"
$CoherentPdfVersion = "2.5.5"
$TessdataVersion = "4.0.0_best_int"
$OcrLanguages = @("ara", "eng")

$BentoDirectory = Join-Path $Root "thirdparty\bentopdf"
$BentoDistDirectory = Join-Path $BentoDirectory "dist"
$ProjectScriptDirectory = Join-Path $Root "scripts\bentopdf"
$InstalledScriptPath = Join-Path $ProjectScriptDirectory "prepare-offline-runtime.ps1"
$IntegrationManifestPath = Join-Path $ProjectScriptDirectory "integration-manifest.json"
$FinalOutputBasePathRepairScript = Join-Path $ProjectScriptDirectory "repair-final-html-base-paths.mjs"

$RuntimeVersionName = "$ExpectedBentoVersion-$($ExpectedBentoCommit.Substring(0, 8))"
$RuntimePackRoot = Join-Path $Root "local-packages\bentopdf-offline-runtime\$RuntimeVersionName"
$RuntimeCacheDirectory = Join-Path $RuntimePackRoot "cache"
$RuntimeWasmDirectory = Join-Path $RuntimePackRoot "wasm"
$RuntimeManifestPath = Join-Path $RuntimePackRoot "runtime-manifest.json"

$OfflineBundleVersionRoot = Join-Path $Root "generated\bentopdf-offline\$RuntimeVersionName"
$OfflineBundleAppDirectory = Join-Path $OfflineBundleVersionRoot "bentopdf"

$EvidenceDirectory = Join-Path $Root "audit-output\bentopdf-offline-runtime-after-tauri"
$TempDirectory = Join-Path $env:TEMP "r2h-pdf-bentopdf-offline-runtime"

$LocalBasePath = "/bentopdf/"
$LocalWasmBasePath = "/bentopdf/wasm"

$LocalPyMuPdfUrl = "$LocalWasmBasePath/pymupdf/"
$LocalGhostscriptUrl = "$LocalWasmBasePath/gs/"
$LocalCoherentPdfUrl = "$LocalWasmBasePath/cpdf/"
$LocalTesseractWorkerUrl = "$LocalWasmBasePath/ocr/worker.min.js"
$LocalTesseractCoreUrl = "$LocalWasmBasePath/ocr/core"
$LocalTesseractLanguageUrl = "$LocalWasmBasePath/ocr/lang-data"
$LocalOcrFontUrl = "$LocalWasmBasePath/ocr/fonts"

$NotoSansUrl = "https://rawcdn.githack.com/googlefonts/noto-fonts/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSans/NotoSans-Regular.ttf"
$NotoNaskhArabicUrl = "https://rawcdn.githack.com/googlefonts/noto-fonts/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoNaskhArabic/NotoNaskhArabic-Regular.ttf"

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

    if ($Lines.Count -le 50) {
        foreach ($Line in $Lines) {
            Write-Host $Line
        }
    }
    else {
        Write-Host "Output lines: $($Lines.Count)"
        $Lines | Select-Object -First 5 | ForEach-Object { Write-Host $_ }
        Write-Host "... full output saved to $LogPath ..." -ForegroundColor DarkGray
        $Lines | Select-Object -Last 30 | ForEach-Object { Write-Host $_ }
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

function Remove-SafeTree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$AllowedRoot
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $ResolvedPath = [System.IO.Path]::GetFullPath($Path).TrimEnd("\")
    $ResolvedAllowedRoot = [System.IO.Path]::GetFullPath($AllowedRoot).TrimEnd("\")

    if (
        $ResolvedPath -eq $ResolvedAllowedRoot -or
        -not $ResolvedPath.StartsWith(
            $ResolvedAllowedRoot + "\",
            [System.StringComparison]::OrdinalIgnoreCase
        )
    ) {
        throw "Refusing to remove path outside its approved child scope: $ResolvedPath"
    }

    Remove-Item -LiteralPath $ResolvedPath -Recurse -Force
}

function Download-File {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string]$Url,

        [Parameter(Mandatory = $true)]
        [string]$Destination
    )

    $Parent = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $Parent | Out-Null

    $LastError = $null

    for ($Attempt = 1; $Attempt -le 4; $Attempt++) {
        try {
            Write-Host "Downloading $Name (attempt $Attempt/4)..." -ForegroundColor Cyan

            Invoke-WebRequest `
                -Uri $Url `
                -OutFile $Destination `
                -UseBasicParsing `
                -Headers @{
                    "User-Agent" = "R2H-PDF-BentoPDF-Offline-Packager/1.0"
                }

            if (-not (Test-Path -LiteralPath $Destination -PathType Leaf)) {
                throw "Download did not create the destination file."
            }

            $Length = (Get-Item -LiteralPath $Destination).Length

            if ($Length -le 0) {
                throw "Downloaded file is empty."
            }

            Write-Host "Downloaded ${Name}: $Length bytes" -ForegroundColor Green
            return
        }
        catch {
            $LastError = $_

            if (Test-Path -LiteralPath $Destination) {
                Remove-Item -LiteralPath $Destination -Force
            }

            if ($Attempt -lt 4) {
                Start-Sleep -Seconds ([math]::Pow(2, $Attempt))
            }
        }
    }

    throw "Failed to download $Name from $Url. Last error: $LastError"
}

function Get-NpmPackageArchive {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PackageSpec,

        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    New-Item -ItemType Directory -Force -Path $RuntimeCacheDirectory | Out-Null

    $Before = @(
        Get-ChildItem `
            -LiteralPath $RuntimeCacheDirectory `
            -Filter "*.tgz" `
            -File |
        Select-Object -ExpandProperty FullName
    )

    $PackResult = Invoke-Native `
        -Name "npm-pack-$Name" `
        -FilePath "npm.cmd" `
        -WorkingDirectory $RuntimeCacheDirectory `
        -Arguments @(
            "pack",
            $PackageSpec,
            "--silent"
        )

    $ArchiveName = Get-LastNonEmptyLine `
        -Lines $PackResult.Lines `
        -Operation "npm pack $PackageSpec"

    $ArchivePath = Join-Path $RuntimeCacheDirectory $ArchiveName

    if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
        $After = @(
            Get-ChildItem `
                -LiteralPath $RuntimeCacheDirectory `
                -Filter "*.tgz" `
                -File |
            Where-Object {
                $_.FullName -notin $Before
            } |
            Sort-Object LastWriteTimeUtc -Descending
        )

        if ($After.Count -ne 1) {
            throw "Unable to identify the npm archive created for $PackageSpec."
        }

        $ArchivePath = $After[0].FullName
    }

    if ((Get-Item -LiteralPath $ArchivePath).Length -le 0) {
        throw "npm archive is empty: $ArchivePath"
    }

    Write-Host "Packed $PackageSpec -> $ArchivePath" -ForegroundColor Green
    return $ArchivePath
}

function Expand-NpmArchive {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Archive,

        [Parameter(Mandatory = $true)]
        [string]$Destination,

        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if (Test-Path -LiteralPath $Destination) {
        Remove-SafeTree -Path $Destination -AllowedRoot $TempDirectory
    }

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null

    Invoke-Native `
        -Name "extract-$Name" `
        -FilePath "tar.exe" `
        -WorkingDirectory $Root `
        -Arguments @(
            "-xf",
            $Archive,
            "-C",
            $Destination
        ) | Out-Null

    $PackageDirectory = Join-Path $Destination "package"

    if (-not (Test-Path -LiteralPath $PackageDirectory -PathType Container)) {
        throw "Extracted npm package root is missing: $PackageDirectory"
    }

    return $PackageDirectory
}

function Copy-DirectoryContents {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Source,

        [Parameter(Mandatory = $true)]
        [string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Source directory is missing: $Source"
    }

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null

    Get-ChildItem -LiteralPath $Source -Force |
        ForEach-Object {
            Copy-Item `
                -LiteralPath $_.FullName `
                -Destination $Destination `
                -Recurse `
                -Force
        }
}

function Add-GitIgnoreRule {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Rule
    )

    $GitIgnorePath = Join-Path $Root ".gitignore"

    if (-not (Test-Path -LiteralPath $GitIgnorePath -PathType Leaf)) {
        throw ".gitignore is missing."
    }

    $Lines = @(
        Get-Content -LiteralPath $GitIgnorePath
    )

    if ($Lines -contains $Rule) {
        return
    }

    Add-Content `
        -LiteralPath $GitIgnorePath `
        -Value $Rule `
        -Encoding UTF8
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
Write-Host "R2H-PDF BENTOPDF OFFLINE RUNTIME PACKAGING" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

# Ensure TLS 1.2 is available for Windows PowerShell 5.1 downloads.
[Net.ServicePointManager]::SecurityProtocol = `
    [Net.ServicePointManager]::SecurityProtocol -bor `
    [Net.SecurityProtocolType]::Tls12

# ============================================================
# 1. Verify required tools
# ============================================================

$RequiredCommands = @(
    "git.exe",
    "node.exe",
    "npm.cmd",
    "tar.exe"
)

foreach ($RequiredCommand in $RequiredCommands) {
    $Resolved = Get-Command $RequiredCommand -ErrorAction SilentlyContinue

    if ($null -eq $Resolved) {
        throw "Required executable was not found: $RequiredCommand"
    }

    Write-Host "$RequiredCommand -> $($Resolved.Source)" -ForegroundColor Green
}

# ============================================================
# 2. Verify parent repository and approved integration ancestry
# ============================================================

$RootResult = Invoke-Git `
    -Name "01-parent-root" `
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
    -Name "02-parent-branch" `
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
    -Name "03-approved-integration-ancestor" `
    -Arguments @(
        "merge-base",
        "--is-ancestor",
        $RequiredIntegrationAncestor,
        "HEAD"
    ) `
    -AllowFailure

if ($AncestorResult.ExitCode -ne 0) {
    throw "The approved BentoPDF integration commit is not an ancestor of HEAD."
}

$ParentStatusBefore = Invoke-Git `
    -Name "04-parent-status-before" `
    -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=all"
    )

$ParentDirtyBefore = @(
    $ParentStatusBefore.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($ParentDirtyBefore.Count -gt 0) {
    Write-Host "Unexpected parent working-tree changes:" -ForegroundColor Red
    $ParentDirtyBefore | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Parent repository must be clean before offline runtime packaging."
}

# ============================================================
# 3. Verify BentoPDF source state
# ============================================================

$BentoHeadResult = Invoke-Git `
    -Name "05-bentopdf-head" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "rev-parse",
        "--verify",
        "HEAD^{commit}"
    )

$BentoHead = Get-LastNonEmptyLine `
    -Lines $BentoHeadResult.Lines `
    -Operation "BentoPDF HEAD"

if ($BentoHead -ne $ExpectedBentoCommit) {
    throw "Unexpected BentoPDF commit. Expected $ExpectedBentoCommit, found $BentoHead"
}

$BentoStatusBefore = Invoke-Git `
    -Name "06-bentopdf-status-before" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=all"
    )

$BentoDirtyBefore = @(
    $BentoStatusBefore.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($BentoDirtyBefore.Count -gt 0) {
    Write-Host "Unexpected BentoPDF source changes:" -ForegroundColor Red
    $BentoDirtyBefore | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "BentoPDF submodule must be clean before offline runtime packaging."
}

$BentoPackage = Get-Content `
    -LiteralPath (Join-Path $BentoDirectory "package.json") `
    -Raw |
    ConvertFrom-Json

if ($BentoPackage.version -ne $ExpectedBentoVersion) {
    throw "Unexpected BentoPDF version: $($BentoPackage.version)"
}

# ============================================================
# 4. Resolve Tesseract versions from the pinned lock file
# ============================================================

$TesseractVersionResult = Invoke-Native `
    -Name "07-tesseract-version" `
    -FilePath "node.exe" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "-p",
        "require('./package-lock.json').packages['node_modules/tesseract.js'].version"
    )

$TesseractCoreVersionResult = Invoke-Native `
    -Name "08-tesseract-core-version" `
    -FilePath "node.exe" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "-p",
        "require('./package-lock.json').packages['node_modules/tesseract.js-core'].version"
    )

$TesseractVersion = Get-LastNonEmptyLine `
    -Lines $TesseractVersionResult.Lines `
    -Operation "Tesseract.js version"

$TesseractCoreVersion = Get-LastNonEmptyLine `
    -Lines $TesseractCoreVersionResult.Lines `
    -Operation "Tesseract.js core version"

Write-Host "PyMuPDF WASM: $PyMuPdfVersion" -ForegroundColor Green
Write-Host "Ghostscript WASM: $GhostscriptVersion" -ForegroundColor Green
Write-Host "CoherentPDF: $CoherentPdfVersion" -ForegroundColor Green
Write-Host "Tesseract.js: $TesseractVersion" -ForegroundColor Green
Write-Host "Tesseract.js core: $TesseractCoreVersion" -ForegroundColor Green
Write-Host "OCR languages: $($OcrLanguages -join ',')" -ForegroundColor Green

# ============================================================
# 5. Prepare safe local runtime directories
# ============================================================

if (Test-Path -LiteralPath $TempDirectory) {
    $TempParent = Split-Path -Parent $TempDirectory
    Remove-SafeTree -Path $TempDirectory -AllowedRoot $TempParent
}

New-Item -ItemType Directory -Force -Path $TempDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $RuntimeCacheDirectory | Out-Null

if (Test-Path -LiteralPath $RuntimeWasmDirectory) {
    Remove-SafeTree -Path $RuntimeWasmDirectory -AllowedRoot $RuntimePackRoot
}

New-Item -ItemType Directory -Force -Path $RuntimeWasmDirectory | Out-Null

# ============================================================
# 6. Download exact npm package archives
# ============================================================

$PyMuPdfArchive = Get-NpmPackageArchive `
    -PackageSpec "@bentopdf/pymupdf-wasm@$PyMuPdfVersion" `
    -Name "pymupdf"

$GhostscriptArchive = Get-NpmPackageArchive `
    -PackageSpec "@bentopdf/gs-wasm@$GhostscriptVersion" `
    -Name "ghostscript"

$CoherentPdfArchive = Get-NpmPackageArchive `
    -PackageSpec "coherentpdf@$CoherentPdfVersion" `
    -Name "coherentpdf"

$TesseractArchive = Get-NpmPackageArchive `
    -PackageSpec "tesseract.js@$TesseractVersion" `
    -Name "tesseract"

$TesseractCoreArchive = Get-NpmPackageArchive `
    -PackageSpec "tesseract.js-core@$TesseractCoreVersion" `
    -Name "tesseract-core"

# ============================================================
# 7. Extract packages into the separate runtime pack
# ============================================================

$PyMuPdfPackage = Expand-NpmArchive `
    -Archive $PyMuPdfArchive `
    -Destination (Join-Path $TempDirectory "pymupdf") `
    -Name "pymupdf"

Copy-DirectoryContents `
    -Source $PyMuPdfPackage `
    -Destination (Join-Path $RuntimeWasmDirectory "pymupdf")

$GhostscriptPackage = Expand-NpmArchive `
    -Archive $GhostscriptArchive `
    -Destination (Join-Path $TempDirectory "ghostscript") `
    -Name "ghostscript"

$GhostscriptAssets = Join-Path $GhostscriptPackage "assets"

if (Test-Path -LiteralPath $GhostscriptAssets -PathType Container) {
    Copy-DirectoryContents `
        -Source $GhostscriptAssets `
        -Destination (Join-Path $RuntimeWasmDirectory "gs")
}
else {
    Copy-DirectoryContents `
        -Source $GhostscriptPackage `
        -Destination (Join-Path $RuntimeWasmDirectory "gs")
}

$CoherentPdfPackage = Expand-NpmArchive `
    -Archive $CoherentPdfArchive `
    -Destination (Join-Path $TempDirectory "coherentpdf") `
    -Name "coherentpdf"

$CoherentPdfDist = Join-Path $CoherentPdfPackage "dist"

if (Test-Path -LiteralPath $CoherentPdfDist -PathType Container) {
    Copy-DirectoryContents `
        -Source $CoherentPdfDist `
        -Destination (Join-Path $RuntimeWasmDirectory "cpdf")
}
else {
    Copy-DirectoryContents `
        -Source $CoherentPdfPackage `
        -Destination (Join-Path $RuntimeWasmDirectory "cpdf")
}

$TesseractPackage = Expand-NpmArchive `
    -Archive $TesseractArchive `
    -Destination (Join-Path $TempDirectory "tesseract") `
    -Name "tesseract"

$TesseractWorkerSource = Join-Path $TesseractPackage "dist\worker.min.js"
$TesseractWorkerDestination = Join-Path $RuntimeWasmDirectory "ocr\worker.min.js"

if (-not (Test-Path -LiteralPath $TesseractWorkerSource -PathType Leaf)) {
    throw "Tesseract worker file is missing from its npm package."
}

New-Item `
    -ItemType Directory `
    -Force `
    -Path (Split-Path -Parent $TesseractWorkerDestination) | Out-Null

Copy-Item `
    -LiteralPath $TesseractWorkerSource `
    -Destination $TesseractWorkerDestination `
    -Force

$TesseractCorePackage = Expand-NpmArchive `
    -Archive $TesseractCoreArchive `
    -Destination (Join-Path $TempDirectory "tesseract-core") `
    -Name "tesseract-core"

Copy-DirectoryContents `
    -Source $TesseractCorePackage `
    -Destination (Join-Path $RuntimeWasmDirectory "ocr\core")

# ============================================================
# 8. Download Arabic and English OCR data and fonts
# ============================================================

$LanguageDirectory = Join-Path $RuntimeWasmDirectory "ocr\lang-data"
$FontDirectory = Join-Path $RuntimeWasmDirectory "ocr\fonts"

New-Item -ItemType Directory -Force -Path $LanguageDirectory | Out-Null
New-Item -ItemType Directory -Force -Path $FontDirectory | Out-Null

foreach ($Language in $OcrLanguages) {
    $LanguageUrl = "https://cdn.jsdelivr.net/npm/@tesseract.js-data/$Language/$TessdataVersion/$Language.traineddata.gz"
    $LanguageDestination = Join-Path $LanguageDirectory "$Language.traineddata.gz"

    Download-File `
        -Name "OCR language $Language" `
        -Url $LanguageUrl `
        -Destination $LanguageDestination
}

Download-File `
    -Name "Noto Sans" `
    -Url $NotoSansUrl `
    -Destination (Join-Path $FontDirectory "NotoSans-Regular.ttf")

Download-File `
    -Name "Noto Naskh Arabic" `
    -Url $NotoNaskhArabicUrl `
    -Destination (Join-Path $FontDirectory "NotoNaskhArabic-Regular.ttf")

# ============================================================
# 9. Validate runtime package layout
# ============================================================

$RequiredRuntimeFiles = @(
    "pymupdf\dist\index.js",
    "gs\gs.js",
    "gs\gs.wasm",
    "cpdf\coherentpdf.browser.min.js",
    "ocr\worker.min.js",
    "ocr\lang-data\ara.traineddata.gz",
    "ocr\lang-data\eng.traineddata.gz",
    "ocr\fonts\NotoSans-Regular.ttf",
    "ocr\fonts\NotoNaskhArabic-Regular.ttf"
)

foreach ($RelativePath in $RequiredRuntimeFiles) {
    $FullPath = Join-Path $RuntimeWasmDirectory $RelativePath

    if (-not (Test-Path -LiteralPath $FullPath -PathType Leaf)) {
        throw "Required offline runtime file is missing: $RelativePath"
    }

    if ((Get-Item -LiteralPath $FullPath).Length -le 0) {
        throw "Required offline runtime file is empty: $RelativePath"
    }

    Write-Host "Verified runtime file: $RelativePath" -ForegroundColor Green
}

$TesseractCoreWasmFiles = @(
    Get-ChildItem `
        -LiteralPath (Join-Path $RuntimeWasmDirectory "ocr\core") `
        -Recurse `
        -File |
    Where-Object {
        $_.Name -match '^tesseract-core.*\.wasm'
    }
)

if ($TesseractCoreWasmFiles.Count -eq 0) {
    throw "No Tesseract core WASM files were found."
}

$RuntimeFiles = @(
    Get-ChildItem `
        -LiteralPath $RuntimeWasmDirectory `
        -Recurse `
        -File
)

$RuntimeTotalBytes = (
    $RuntimeFiles |
        Measure-Object -Property Length -Sum
).Sum

Write-Host "Offline runtime files: $($RuntimeFiles.Count)" -ForegroundColor Green
Write-Host "Offline runtime bytes: $RuntimeTotalBytes" -ForegroundColor Green

# ============================================================
# 10. Ensure BentoPDF dependencies are installed
# ============================================================

$TscExecutable = Join-Path $BentoDirectory "node_modules\.bin\tsc.cmd"
$ViteExecutable = Join-Path $BentoDirectory "node_modules\.bin\vite.cmd"

if (
    -not (Test-Path -LiteralPath $TscExecutable -PathType Leaf) -or
    -not (Test-Path -LiteralPath $ViteExecutable -PathType Leaf)
) {
    $env:HUSKY = "0"

    Invoke-Native `
        -Name "09-npm-ci" `
        -FilePath "npm.cmd" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "ci",
            "--no-audit",
            "--no-fund"
        ) | Out-Null
}

if (-not (Test-Path -LiteralPath $TscExecutable -PathType Leaf)) {
    throw "TypeScript compiler is unavailable after npm ci."
}

if (-not (Test-Path -LiteralPath $ViteExecutable -PathType Leaf)) {
    throw "Vite executable is unavailable after npm ci."
}

# ============================================================
# 11. Build BentoPDF with local runtime URLs
# ============================================================

$PreviousEnvironment = @{}

$EnvironmentNames = @(
    "HUSKY",
    "NODE_OPTIONS",
    "SIMPLE_MODE",
    "BASE_URL",
    "VITE_USE_CDN",
    "VITE_DEFAULT_LANGUAGE",
    "VITE_BRAND_NAME",
    "VITE_BRAND_LOGO",
    "VITE_FOOTER_TEXT",
    "VITE_WASM_PYMUPDF_URL",
    "VITE_WASM_GS_URL",
    "VITE_WASM_CPDF_URL",
    "VITE_TESSERACT_WORKER_URL",
    "VITE_TESSERACT_CORE_URL",
    "VITE_TESSERACT_LANG_URL",
    "VITE_TESSERACT_AVAILABLE_LANGUAGES",
    "VITE_OCR_FONT_BASE_URL",
    "SITE_URL"
)

foreach ($EnvironmentName in $EnvironmentNames) {
    $PreviousEnvironment[$EnvironmentName] = `
        [Environment]::GetEnvironmentVariable($EnvironmentName, "Process")
}

try {
    $env:HUSKY = "0"
    $env:NODE_OPTIONS = "--max-old-space-size=4096"
    $env:SIMPLE_MODE = "true"
    $env:BASE_URL = $LocalBasePath
    $env:VITE_USE_CDN = "false"
    $env:VITE_DEFAULT_LANGUAGE = "ar"
    $env:VITE_BRAND_NAME = "R2H PDF"
    $env:VITE_BRAND_LOGO = ""
    $env:VITE_FOOTER_TEXT = "R2H PDF"

    $env:VITE_WASM_PYMUPDF_URL = $LocalPyMuPdfUrl
    $env:VITE_WASM_GS_URL = $LocalGhostscriptUrl
    $env:VITE_WASM_CPDF_URL = $LocalCoherentPdfUrl
    $env:VITE_TESSERACT_WORKER_URL = $LocalTesseractWorkerUrl
    $env:VITE_TESSERACT_CORE_URL = $LocalTesseractCoreUrl
    $env:VITE_TESSERACT_LANG_URL = $LocalTesseractLanguageUrl
    $env:VITE_TESSERACT_AVAILABLE_LANGUAGES = $OcrLanguages -join ","
    $env:VITE_OCR_FONT_BASE_URL = $LocalOcrFontUrl

    Invoke-Native `
        -Name "10-typescript-build" `
        -FilePath $TscExecutable `
        -WorkingDirectory $BentoDirectory | Out-Null

    Invoke-Native `
        -Name "11-vite-offline-build" `
        -FilePath $ViteExecutable `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "build"
        ) | Out-Null

    # i18n generator appends BASE_URL itself.
    $env:SITE_URL = "https://www.bentopdf.com"

    Invoke-Native `
        -Name "12-generate-i18n" `
        -FilePath "node.exe" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "scripts/generate-i18n-pages.mjs"
        ) | Out-Null

    # Sitemap and audit require the complete externally visible base path.
    $env:SITE_URL = "https://www.bentopdf.com/bentopdf"

    Invoke-Native `
        -Name "13-generate-sitemap" `
        -FilePath "node.exe" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "scripts/generate-sitemap.mjs"
        ) | Out-Null

    Invoke-Native `
        -Name "14-generate-security-headers" `
        -FilePath "node.exe" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "scripts/generate-security-headers.mjs"
        ) | Out-Null

    Invoke-Native `
        -Name "15-seo-audit" `
        -FilePath "node.exe" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "scripts/seo-audit.mjs"
        ) | Out-Null
}
finally {
    foreach ($EnvironmentName in $EnvironmentNames) {
        [Environment]::SetEnvironmentVariable(
            $EnvironmentName,
            $PreviousEnvironment[$EnvironmentName],
            "Process"
        )
    }
}

# Restore the one upstream tracked configuration file generated by the build.
$GeneratedTrackedFile = "security-headers-docs.conf"

$GeneratedTrackedStatus = Invoke-Git `
    -Name "16-generated-tracked-status" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--porcelain=v1",
        "--untracked-files=no"
    )

$TrackedChanges = @(
    foreach ($Line in $GeneratedTrackedStatus.Lines) {
        if ([string]::IsNullOrWhiteSpace($Line)) {
            continue
        }

        if ($Line.Length -ge 4) {
            $Line.Substring(3).Trim().Trim('"').Replace("\", "/")
        }
        else {
            $Line.Trim()
        }
    }
)

$UnexpectedTrackedChanges = @(
    $TrackedChanges |
        Where-Object {
            $_ -ne $GeneratedTrackedFile
        }
)

if ($UnexpectedTrackedChanges.Count -gt 0) {
    Write-Host "Unexpected tracked BentoPDF changes after build:" -ForegroundColor Red
    $UnexpectedTrackedChanges | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Offline build modified unexpected BentoPDF source files."
}

if ($TrackedChanges -contains $GeneratedTrackedFile) {
    Copy-Item `
        -LiteralPath (Join-Path $BentoDirectory $GeneratedTrackedFile) `
        -Destination (Join-Path $EvidenceDirectory "generated-security-headers-docs.conf") `
        -Force

    Invoke-Git `
        -Name "17-restore-generated-tracked-file" `
        -WorkingDirectory $BentoDirectory `
        -Arguments @(
            "restore",
            "--source=HEAD",
            "--worktree",
            "--",
            $GeneratedTrackedFile
        ) | Out-Null
}

$BentoStatusAfterBuild = Invoke-Git `
    -Name "18-bentopdf-status-after-build" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--short"
    )

$BentoDirtyAfterBuild = @(
    $BentoStatusAfterBuild.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($BentoDirtyAfterBuild.Count -gt 0) {
    throw "BentoPDF submodule is not clean after offline build."
}

# ============================================================
# 12. Add runtime assets to dist and stage a complete local bundle
# ============================================================

$DistWasmDirectory = Join-Path $BentoDistDirectory "wasm"

if (Test-Path -LiteralPath $DistWasmDirectory) {
    Remove-SafeTree -Path $DistWasmDirectory -AllowedRoot $BentoDistDirectory
}

Copy-Item `
    -LiteralPath $RuntimeWasmDirectory `
    -Destination $BentoDistDirectory `
    -Recurse `
    -Force

if (Test-Path -LiteralPath $OfflineBundleVersionRoot) {
    $GeneratedOfflineRoot = Join-Path $Root "generated\bentopdf-offline"
    Remove-SafeTree `
        -Path $OfflineBundleVersionRoot `
        -AllowedRoot $GeneratedOfflineRoot
}

New-Item `
    -ItemType Directory `
    -Force `
    -Path $OfflineBundleVersionRoot | Out-Null

Copy-Item `
    -LiteralPath $BentoDistDirectory `
    -Destination $OfflineBundleAppDirectory `
    -Recurse `
    -Force

if (-not (Test-Path -LiteralPath $FinalOutputBasePathRepairScript -PathType Leaf)) {
    throw "Final-output base-path repair script is missing: $FinalOutputBasePathRepairScript"
}

$FinalOutputRepairReportPath = Join-Path `
    $EvidenceDirectory `
    "18-final-output-base-path-repair.json"

Invoke-Native `
    -Name "18-final-output-base-path-repair" `
    -FilePath "node.exe" `
    -WorkingDirectory $Root `
    -Arguments @(
        $FinalOutputBasePathRepairScript,
        $OfflineBundleAppDirectory,
        $LocalBasePath,
        $FinalOutputRepairReportPath
    ) | Out-Null

$FinalOutputRepairReport = Get-Content `
    -LiteralPath $FinalOutputRepairReportPath `
    -Raw |
    ConvertFrom-Json

if (-not $FinalOutputRepairReport.webManifestFound) {
    throw "Final-output repair did not find site.webmanifest."
}

if ($FinalOutputRepairReport.remainingHtmlRootAbsoluteReferenceCount -ne 0) {
    throw "Final-output repair left root-absolute HTML references."
}

if ($FinalOutputRepairReport.remainingWebManifestRootAbsoluteReferenceCount -ne 0) {
    throw "Final-output repair left root-absolute web manifest references."
}

$FinalOutputVerificationReportPath = Join-Path `
    $EvidenceDirectory `
    "18-final-output-base-path-verification.json"

Invoke-Native `
    -Name "18-final-output-base-path-verification" `
    -FilePath "node.exe" `
    -WorkingDirectory $Root `
    -Arguments @(
        $FinalOutputBasePathRepairScript,
        $OfflineBundleAppDirectory,
        $LocalBasePath,
        $FinalOutputVerificationReportPath
    ) | Out-Null

$FinalOutputVerificationReport = Get-Content `
    -LiteralPath $FinalOutputVerificationReportPath `
    -Raw |
    ConvertFrom-Json

if ($FinalOutputVerificationReport.changedHtmlFileCount -ne 0) {
    throw "Final-output HTML repair is not idempotent."
}

if ($FinalOutputVerificationReport.webManifestChanged) {
    throw "Final-output web manifest repair is not idempotent."
}

if ($FinalOutputVerificationReport.remainingHtmlRootAbsoluteReferenceCount -ne 0) {
    throw "Verified bundle contains root-absolute HTML references."
}

if ($FinalOutputVerificationReport.remainingWebManifestRootAbsoluteReferenceCount -ne 0) {
    throw "Verified bundle contains root-absolute web manifest references."
}

Write-Host "Final-output base paths repaired and verified." -ForegroundColor Green

$OfflineIndexPath = Join-Path $OfflineBundleAppDirectory "index.html"

if (-not (Test-Path -LiteralPath $OfflineIndexPath -PathType Leaf)) {
    throw "Offline BentoPDF bundle index.html was not created."
}

$OfflineBundleFiles = @(
    Get-ChildItem `
        -LiteralPath $OfflineBundleAppDirectory `
        -Recurse `
        -File
)

$OfflineBundleBytes = (
    $OfflineBundleFiles |
        Measure-Object -Property Length -Sum
).Sum

Write-Host "Offline bundle files: $($OfflineBundleFiles.Count)" -ForegroundColor Green
Write-Host "Offline bundle bytes: $OfflineBundleBytes" -ForegroundColor Green

# ============================================================
# 13. Verify compiled local runtime URLs
# ============================================================

$CompiledSearchFiles = @(
    Get-ChildItem `
        -LiteralPath $OfflineBundleAppDirectory `
        -Recurse `
        -File |
    Where-Object {
        $_.Extension -in @(".js", ".html", ".mjs")
    }
)

$RequiredCompiledMarkers = @(
    $LocalPyMuPdfUrl,
    $LocalGhostscriptUrl,
    $LocalCoherentPdfUrl,
    $LocalTesseractWorkerUrl,
    $LocalTesseractCoreUrl,
    $LocalTesseractLanguageUrl,
    $LocalOcrFontUrl
)

foreach ($Marker in $RequiredCompiledMarkers) {
    $Matches = @(
        $CompiledSearchFiles |
            Select-String `
                -SimpleMatch `
                -Pattern $Marker
    )

    if ($Matches.Count -eq 0) {
        throw "Compiled BentoPDF bundle does not contain local runtime marker: $Marker"
    }

    Write-Host "Compiled marker verified: $Marker" -ForegroundColor Green
}

$DoubleBaseMatches = @(
    $CompiledSearchFiles |
        Select-String `
            -SimpleMatch `
            -Pattern "https://www.bentopdf.com/bentopdf/bentopdf"
)

if ($DoubleBaseMatches.Count -gt 0) {
    throw "Offline bundle contains duplicated /bentopdf/bentopdf canonical paths."
}

# ============================================================
# 14. Serve the complete bundle locally and verify endpoints
# ============================================================

$ServerScriptPath = Join-Path $TempDirectory "offline-static-server.mjs"
$ServerPort = 41791

@'
import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(process.argv[2]);
const port = Number(process.argv[3]);

const mime = {
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
  ".pdf": "application/pdf",
};

const server = http.createServer((req, res) => {
  try {
    const url = new URL(req.url || "/", "http://127.0.0.1");
    const relative = decodeURIComponent(url.pathname).replace(/^[/\\]+/, "");
    let target = path.resolve(root, relative);

    if (target !== root && !target.startsWith(root + path.sep)) {
      res.writeHead(403);
      res.end("Forbidden");
      return;
    }

    if (fs.existsSync(target) && fs.statSync(target).isDirectory()) {
      target = path.join(target, "index.html");
    }

    if (!fs.existsSync(target) || !fs.statSync(target).isFile()) {
      res.writeHead(404);
      res.end("Not found");
      return;
    }

    const stat = fs.statSync(target);
    const extension = path.extname(target).toLowerCase();

    res.setHeader("Content-Type", mime[extension] || "application/octet-stream");
    res.setHeader("Content-Length", String(stat.size));
    res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    res.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    res.setHeader("Cache-Control", "no-store");

    if ((req.method || "GET").toUpperCase() === "HEAD") {
      res.writeHead(200);
      res.end();
      return;
    }

    res.writeHead(200);
    fs.createReadStream(target).pipe(res);
  } catch (error) {
    res.writeHead(500);
    res.end(String(error));
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`READY http://127.0.0.1:${port}`);
});
'@ |
    Set-Content `
        -LiteralPath $ServerScriptPath `
        -Encoding UTF8

$ServerStdout = Join-Path $EvidenceDirectory "offline-server.stdout.log"
$ServerStderr = Join-Path $EvidenceDirectory "offline-server.stderr.log"

$ServerProcess = Start-Process `
    -FilePath "node.exe" `
    -ArgumentList @(
        $ServerScriptPath,
        $OfflineBundleVersionRoot,
        "$ServerPort"
    ) `
    -WorkingDirectory $Root `
    -RedirectStandardOutput $ServerStdout `
    -RedirectStandardError $ServerStderr `
    -PassThru `
    -NoNewWindow

try {
    $Ready = $false

    for ($Attempt = 1; $Attempt -le 30; $Attempt++) {
        Start-Sleep -Milliseconds 500

        if ($ServerProcess.HasExited) {
            break
        }

        try {
            $Probe = Invoke-WebRequest `
                -Uri "http://127.0.0.1:$ServerPort/bentopdf/index.html" `
                -Method Head `
                -UseBasicParsing `
                -TimeoutSec 2

            if ($Probe.StatusCode -eq 200) {
                $Ready = $true
                break
            }
        }
        catch {
            # Continue waiting until the bounded timeout expires.
        }
    }

    if (-not $Ready) {
        $ServerErrorText = ""

        if (Test-Path -LiteralPath $ServerStderr) {
            $ServerErrorText = Get-Content -LiteralPath $ServerStderr -Raw
        }

        throw "Offline local server did not become ready. $ServerErrorText"
    }

    $EndpointPaths = @(
        "/bentopdf/index.html",
        "/bentopdf/merge-pdf.html",
        "/bentopdf/ar/index.html",
        "/bentopdf/wasm/pymupdf/dist/index.js",
        "/bentopdf/wasm/gs/gs.js",
        "/bentopdf/wasm/gs/gs.wasm",
        "/bentopdf/wasm/cpdf/coherentpdf.browser.min.js",
        "/bentopdf/wasm/ocr/worker.min.js",
        "/bentopdf/wasm/ocr/lang-data/ara.traineddata.gz",
        "/bentopdf/wasm/ocr/lang-data/eng.traineddata.gz",
        "/bentopdf/wasm/ocr/fonts/NotoSans-Regular.ttf",
        "/bentopdf/wasm/ocr/fonts/NotoNaskhArabic-Regular.ttf"
    )

    foreach ($EndpointPath in $EndpointPaths) {
        $Response = Invoke-WebRequest `
            -Uri "http://127.0.0.1:$ServerPort$EndpointPath" `
            -Method Head `
            -UseBasicParsing `
            -TimeoutSec 15

        if ($Response.StatusCode -ne 200) {
            throw "Offline endpoint returned HTTP $($Response.StatusCode): $EndpointPath"
        }

        Write-Host "HTTP 200: $EndpointPath" -ForegroundColor Green
    }
}
finally {
    if ($null -ne $ServerProcess -and -not $ServerProcess.HasExited) {
        Stop-Process -Id $ServerProcess.Id -Force
        $ServerProcess.WaitForExit()
    }
}

# ============================================================
# 15. Create local runtime manifest with SHA-256 inventory
# ============================================================

$RuntimeInventory = @(
    foreach ($RuntimeFile in $RuntimeFiles) {
        $RelativePath = $RuntimeFile.FullName.Substring(
            $RuntimeWasmDirectory.Length
        ).TrimStart("\").Replace("\", "/")

        [ordered]@{
            path = $RelativePath
            sizeBytes = $RuntimeFile.Length
            sha256 = (
                Get-FileHash `
                    -LiteralPath $RuntimeFile.FullName `
                    -Algorithm SHA256
            ).Hash
        }
    }
)

$RuntimeManifest = [ordered]@{
    schemaVersion = 1
    component = "BentoPDF offline runtime"
    bentoVersion = $ExpectedBentoVersion
    bentoCommit = $ExpectedBentoCommit

    pymupdfVersion = $PyMuPdfVersion
    ghostscriptVersion = $GhostscriptVersion
    coherentPdfVersion = $CoherentPdfVersion
    tesseractVersion = $TesseractVersion
    tesseractCoreVersion = $TesseractCoreVersion
    tessdataVersion = $TessdataVersion

    ocrLanguages = $OcrLanguages
    localBasePath = $LocalBasePath
    wasmBasePath = "$LocalBasePath" + "wasm/"

    runtimeFileCount = $RuntimeFiles.Count
    runtimeTotalBytes = $RuntimeTotalBytes
    bundleFileCount = $OfflineBundleFiles.Count
    bundleTotalBytes = $OfflineBundleBytes

    staticHttpVerificationPassed = $true
    browserNetworkIsolationAuditPassed = $false
    tauriUiWired = $true
    fullyOfflineClaimed = $false

    createdAtUtc = [DateTime]::UtcNow.ToString("o")
    files = $RuntimeInventory
}

$RuntimeManifest |
    ConvertTo-Json `
        -Depth 10 |
    Set-Content `
        -LiteralPath $RuntimeManifestPath `
        -Encoding UTF8

# ============================================================
# 16. Install the reusable script and update tracked metadata
# ============================================================

if ($PSCommandPath -ne $InstalledScriptPath) {
    Copy-Item `
        -LiteralPath $PSCommandPath `
        -Destination $InstalledScriptPath `
        -Force
}

Add-GitIgnoreRule -Rule "/local-packages/bentopdf-offline-runtime/"
Add-GitIgnoreRule -Rule "/generated/bentopdf-offline/"

if (-not (Test-Path -LiteralPath $IntegrationManifestPath -PathType Leaf)) {
    throw "Tracked BentoPDF integration manifest is missing."
}

$IntegrationManifest = Get-Content `
    -LiteralPath $IntegrationManifestPath `
    -Raw |
    ConvertFrom-Json

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineRuntimePipelinePrepared" `
    -Value $true

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineRuntimePackPath" `
    -Value "local-packages/bentopdf-offline-runtime/$RuntimeVersionName"

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBundlePath" `
    -Value "generated/bentopdf-offline/$RuntimeVersionName/bentopdf"

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineRuntimeBasePath" `
    -Value $LocalBasePath

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineOcrLanguages" `
    -Value $OcrLanguages

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "advancedWasmStatus" `
    -Value "LOCAL_PACKAGING_PIPELINE_READY"

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineStaticHttpVerificationPassed" `
    -Value $true

Set-ManifestProperty `
    -Object $IntegrationManifest `
    -Name "offlineBrowserNetworkAuditPassed" `
    -Value $false

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

# ============================================================
# 17. Stage and commit only reproducible pipeline files
# ============================================================

Invoke-Git `
    -Name "19-stage-offline-pipeline" `
    -Arguments @(
        "add",
        "--",
        ".gitignore",
        "scripts/bentopdf/prepare-offline-runtime.ps1",
        "scripts/bentopdf/integration-manifest.json"
    ) | Out-Null

$StagedResult = Invoke-Git `
    -Name "20-staged-offline-pipeline" `
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
    ".gitignore",
    "scripts/bentopdf/prepare-offline-runtime.ps1",
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
        -Name "21-commit-offline-pipeline" `
        -Arguments @(
            "commit",
            "-m",
            "feat(pdf): prepare local BentoPDF offline runtime pack"
        ) | Out-Null
}
else {
    Write-Host "Tracked offline pipeline files are unchanged; no new commit required." -ForegroundColor Yellow
}

# ============================================================
# 18. Final verification
# ============================================================

$FinalHeadResult = Invoke-Git `
    -Name "22-final-head" `
    -Arguments @(
        "rev-parse",
        "--verify",
        "HEAD^{commit}"
    )

$FinalCommit = Get-LastNonEmptyLine `
    -Lines $FinalHeadResult.Lines `
    -Operation "Final parent commit"

$FinalStatusResult = Invoke-Git `
    -Name "23-final-parent-status" `
    -Arguments @(
        "status",
        "--short"
    )

$FinalParentChanges = @(
    $FinalStatusResult.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($FinalParentChanges.Count -gt 0) {
    Write-Host "Remaining parent changes:" -ForegroundColor Red
    $FinalParentChanges | ForEach-Object { Write-Host $_ -ForegroundColor Yellow }
    throw "Parent repository is not clean after offline runtime packaging."
}

$FinalBentoStatusResult = Invoke-Git `
    -Name "24-final-bentopdf-status" `
    -WorkingDirectory $BentoDirectory `
    -Arguments @(
        "status",
        "--short"
    )

$FinalBentoChanges = @(
    $FinalBentoStatusResult.Lines |
        Where-Object {
            -not [string]::IsNullOrWhiteSpace($_)
        }
)

if ($FinalBentoChanges.Count -gt 0) {
    throw "BentoPDF submodule is not clean after offline runtime packaging."
}

$Summary = [ordered]@{
    status = "PASS"
    finalCommit = $FinalCommit
    branch = $CurrentBranch

    bentoVersion = $ExpectedBentoVersion
    bentoCommit = $ExpectedBentoCommit

    runtimePackPath = $RuntimePackRoot
    offlineBundlePath = $OfflineBundleAppDirectory
    runtimeManifestPath = $RuntimeManifestPath

    runtimeFileCount = $RuntimeFiles.Count
    runtimeTotalBytes = $RuntimeTotalBytes
    bundleFileCount = $OfflineBundleFiles.Count
    bundleTotalBytes = $OfflineBundleBytes

    ocrLanguages = $OcrLanguages

    localRuntimeUrlsCompiled = $true
    staticHttpVerificationPassed = $true
    seoAuditPassed = $true
    duplicateBasePathCount = 0

    browserNetworkIsolationAuditPassed = $false
    tauriUiWired = $true
    fullyOffline = $false

    parentWorkingTreeClean = $true
    bentoSubmoduleClean = $true
}

$Summary |
    ConvertTo-Json `
        -Depth 8 |
    Set-Content `
        -LiteralPath (Join-Path $EvidenceDirectory "summary.json") `
        -Encoding UTF8

Write-Host "`n================================================" -ForegroundColor Green
Write-Host "BENTOPDF OFFLINE RUNTIME PACKAGING PASSED" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Commit: $FinalCommit"
Write-Host "Branch: $CurrentBranch"
Write-Host "BentoPDF: $ExpectedBentoVersion @ $ExpectedBentoCommit"
Write-Host "Runtime pack: $RuntimePackRoot"
Write-Host "Offline bundle: $OfflineBundleAppDirectory"
Write-Host "Runtime files: $($RuntimeFiles.Count)"
Write-Host "Bundle files: $($OfflineBundleFiles.Count)"
Write-Host "OCR languages: $($OcrLanguages -join ',')"
Write-Host "Local runtime URLs compiled: yes"
Write-Host "Static HTTP verification: passed"
Write-Host "SEO audit: passed"
Write-Host "Parent working tree: clean"
Write-Host "BentoPDF submodule: clean"
Write-Host ""
Write-Host "Browser network-isolation audit: not run yet"
Write-Host "Tauri UI wiring: integrated and verified before packaging"
Write-Host "Fully offline release status: not claimed yet"
