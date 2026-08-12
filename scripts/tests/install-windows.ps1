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

function Invoke-WithRedirectedInput {
    param(
        [Parameter(Mandatory = $true)][string]$Script,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Input,
        [Parameter(Mandatory = $true)][hashtable]$Environment
    )

    $StartInfo = [Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = Join-Path $PSHOME "pwsh.exe"
    $StartInfo.UseShellExecute = $false
    $StartInfo.RedirectStandardInput = $true
    foreach ($Argument in @("-NoLogo", "-NoProfile", "-File", $Script) + $Arguments) {
        $StartInfo.ArgumentList.Add($Argument)
    }
    foreach ($Entry in $Environment.GetEnumerator()) {
        $StartInfo.Environment[$Entry.Key] = [string]$Entry.Value
    }
    $Process = [Diagnostics.Process]::Start($StartInfo)
    $Process.StandardInput.WriteLine($Input)
    $Process.StandardInput.Close()
    $Process.WaitForExit()
    return $Process.ExitCode
}

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

    # Replace the archive with the isolated stdin fixture for bootstrap terminal-boundary tests.
    $FixtureSource = Join-Path $Root "scripts\tests\fixtures\runner.rs"
    & rustc $FixtureSource --edition 2024 -o (Join-Path $Package "rewire.exe")
    if ($LASTEXITCODE -ne 0) { throw "could not compile the installer fixture" }
    Remove-Item -LiteralPath (Join-Path $Assets $Asset)
    Compress-Archive -Path (Join-Path $Package "*") -DestinationPath (Join-Path $Assets $Asset)
    $FixtureDigest = (Get-FileHash -LiteralPath (Join-Path $Assets $Asset) -Algorithm SHA256).Hash
    Set-Content -LiteralPath (Join-Path $Assets "SHA256SUMS") -Value "$FixtureDigest  $Asset" -Encoding ascii

    $TerminalOutput = Join-Path $TemporaryDirectory "terminal-arguments"
    $TerminalStatus = Invoke-WithRedirectedInput `
        -Script $Installer `
        -Arguments @("--asset-base-url", $Assets, "--install-dir", $InstallDir) `
        -Input "bootstrap-source" `
        -Environment @{ REWIRE_TEST_OUTPUT = $TerminalOutput; REWIRE_TEST_REQUIRE_TERMINAL = "1" }
    if ($TerminalStatus -ne 0) {
        throw "installer did not attach console input; child exit code $TerminalStatus"
    }
    if ((Get-Content -LiteralPath $TerminalOutput) -notcontains "argument=configure") {
        throw "installer did not preserve the default configure invocation"
    }

    $QuotedOutput = Join-Path $TemporaryDirectory "quoted-terminal-arguments"
    $QuotedStatus = Invoke-WithRedirectedInput `
        -Script $Installer `
        -Arguments @("--asset-base-url", $Assets, "--install-dir", $InstallDir, "--", "--fixture-terminal", "value with spaces") `
        -Input "bootstrap-source" `
        -Environment @{ REWIRE_TEST_OUTPUT = $QuotedOutput; REWIRE_TEST_REQUIRE_TERMINAL = "1" }
    $QuotedLines = @(Get-Content -LiteralPath $QuotedOutput)
    if ($QuotedStatus -ne 0 -or $QuotedLines -notcontains "argument=value with spaces") {
        throw "console-attached invocation did not preserve quoted arguments"
    }

    foreach ($Mode in "--token-stdin", "--non-interactive") {
        $ModeName = $Mode.TrimStart("-")
        $InputOutput = Join-Path $TemporaryDirectory "$ModeName-arguments"
        $InputStatus = Invoke-WithRedirectedInput `
            -Script $Installer `
            -Arguments @("--asset-base-url", $Assets, "--install-dir", $InstallDir, "--", "--fixture-read-stdin", $Mode) `
            -Input "$ModeName-input" `
            -Environment @{ REWIRE_TEST_OUTPUT = $InputOutput }
        $InputLines = @(Get-Content -LiteralPath $InputOutput)
        if ($InputStatus -ne 0 -or $InputLines -notcontains "stdin=$ModeName-input") {
            throw "$Mode did not preserve redirected standard input; fixture output: $($InputLines -join ' | ')"
        }
    }

    Write-Output "Windows installer tests passed."
} finally {
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}
