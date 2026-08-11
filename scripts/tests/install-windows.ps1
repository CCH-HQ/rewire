param(
    [Parameter(Mandatory = $true)][string]$BinaryPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Installer = Join-Path $Root "scripts\install.ps1"
$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("rewire-installer-test-" + [guid]::NewGuid())
$Assets = Join-Path $TemporaryDirectory "assets"
$Package = Join-Path $TemporaryDirectory "package"
$InstallDir = Join-Path $TemporaryDirectory "install"
$FixtureHome = Join-Path $TemporaryDirectory "fixture home"

New-Item -ItemType Directory -Path $Assets, $Package, $FixtureHome | Out-Null
try {
    $Asset = "rewire-x86_64-pc-windows-msvc.zip"
    Copy-Item -LiteralPath $BinaryPath -Destination (Join-Path $Package "rewire.exe")
    Compress-Archive -Path (Join-Path $Package "*") -DestinationPath (Join-Path $Assets $Asset)
    $Digest = (Get-FileHash -LiteralPath (Join-Path $Assets $Asset) -Algorithm SHA256).Hash
    Set-Content -LiteralPath (Join-Path $Assets "SHA256SUMS") -Value "$Digest  $Asset" -Encoding ascii

    $env:REWIRE_ASSET_BASE_URL = Join-Path $TemporaryDirectory "missing-assets"
    $env:REWIRE_CHECKSUM_URL = Join-Path $TemporaryDirectory "missing-checksums"
    $env:REWIRE_RELEASE = "../ignored-for-direct-download"
    try {
        $Output = @(& $Installer `
            --download-url (Join-Path $Assets $Asset) `
            --sha256 $Digest `
            --install-dir $InstallDir `
            -- `
            --home $FixtureHome `
            doctor `
            --json)
    } finally {
        Remove-Item Env:REWIRE_ASSET_BASE_URL, Env:REWIRE_CHECKSUM_URL, Env:REWIRE_RELEASE
    }
    if ($LASTEXITCODE -ne 0) {
        throw "installed Rewire exited with $LASTEXITCODE"
    }
    $JsonStart = [Array]::IndexOf($Output, "{")
    if ($JsonStart -lt 0) {
        throw "doctor JSON was not present in installer output"
    }
    $DoctorJson = $Output[$JsonStart..($Output.Count - 1)] -join [Environment]::NewLine
    $Doctor = $DoctorJson | ConvertFrom-Json
    if ([IO.Path]::GetFullPath($Doctor.home) -ne [IO.Path]::GetFullPath($FixtureHome)) {
        throw "Rewire arguments were not preserved"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $InstallDir "rewire.exe") -PathType Leaf)) {
        throw "rewire.exe was not installed"
    }

    $NoRunOutput = @(& $Installer `
        --asset-base-url $Assets `
        --install-dir $InstallDir `
        --no-run)
    if (($NoRunOutput -join "`n") -match '"detected"') {
        throw "--no-run unexpectedly started Rewire"
    }

    $Installed = Join-Path $InstallDir "rewire.exe"
    $Before = (Get-FileHash -LiteralPath $Installed -Algorithm SHA256).Hash
    Set-Content -LiteralPath (Join-Path $Assets "SHA256SUMS") -Value "$('0' * 64)  $Asset" -Encoding ascii
    $ChecksumFailed = $false
    try {
        & $Installer --asset-base-url $Assets --install-dir $InstallDir --no-run
    } catch {
        $ChecksumFailed = $_.Exception.Message -match 'checksum mismatch'
    }
    if (-not $ChecksumFailed) {
        throw "checksum mismatch unexpectedly succeeded"
    }
    $After = (Get-FileHash -LiteralPath $Installed -Algorithm SHA256).Hash
    if ($Before -ne $After) {
        throw "checksum failure replaced the existing installation"
    }

    Write-Output "Windows installer tests passed."
} finally {
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}
