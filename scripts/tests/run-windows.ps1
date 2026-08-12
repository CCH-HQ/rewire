Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Runner = Join-Path $Root "scripts\run.ps1"
$Installer = Join-Path $Root "scripts\install.ps1"
$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("rewire-runner-test-" + [guid]::NewGuid())
$Assets = Join-Path $TemporaryDirectory "assets"
$Package = Join-Path $TemporaryDirectory "package"
$RunnerTemp = Join-Path $TemporaryDirectory "runner-temp"
$PersistentInstall = Join-Path $TemporaryDirectory "must-not-be-used"

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

New-Item -ItemType Directory -Path $Assets, $Package, $RunnerTemp | Out-Null
try {
    # Compile the isolated fixture with the CI toolchain; no test hook enters the Rewire binary.
    $FixtureSource = Join-Path $Root "scripts\tests\fixtures\runner.rs"
    $FixtureBinary = Join-Path $Package "rewire.exe"
    & rustc $FixtureSource --edition 2024 -o $FixtureBinary
    if ($LASTEXITCODE -ne 0) {
        throw "could not compile the run-only fixture"
    }

    $Asset = "rewire-x86_64-pc-windows-msvc.zip"
    Compress-Archive -Path (Join-Path $Package "*") -DestinationPath (Join-Path $Assets $Asset)
    $Digest = (Get-FileHash -LiteralPath (Join-Path $Assets $Asset) -Algorithm SHA256).Hash
    Set-Content -LiteralPath (Join-Path $Assets "SHA256SUMS") -Value "$Digest  $Asset" -Encoding ascii

    $Output = Join-Path $TemporaryDirectory "arguments"
    $PreviousInstallDir = $env:REWIRE_INSTALL_DIR
    $PreviousTestOutput = $env:REWIRE_TEST_OUTPUT
    $PreviousTemp = $env:TEMP
    try {
        $env:REWIRE_INSTALL_DIR = $PersistentInstall
        $env:REWIRE_TEST_OUTPUT = $Output
        $env:TEMP = $RunnerTemp
        & $Runner `
            --download-url (Join-Path $Assets $Asset) `
            --sha256 $Digest -- `
            --baseurl "https://gateway.example/api path" `
            --client "claude,codex" `
            --dry-run
        if ($LASTEXITCODE -ne 0) {
            throw "run.ps1 returned $LASTEXITCODE"
        }
    } finally {
        $env:REWIRE_INSTALL_DIR = $PreviousInstallDir
        $env:REWIRE_TEST_OUTPUT = $PreviousTestOutput
        $env:TEMP = $PreviousTemp
    }

    $Lines = @(Get-Content -LiteralPath $Output)
    $Expected = @(
        "argument=--baseurl",
        "argument=https://gateway.example/api path",
        "argument=--client",
        "argument=claude,codex",
        "argument=--dry-run"
    )
    if (($Lines[1..($Lines.Count - 1)] -join "`n") -ne ($Expected -join "`n")) {
        throw "Rewire arguments were not preserved"
    }
    $Executable = $Lines[0].Substring("executable=".Length)
    if (Test-Path -LiteralPath $Executable) {
        throw "temporary Rewire executable survived the run"
    }
    if (Test-Path -LiteralPath $PersistentInstall) {
        throw "REWIRE_INSTALL_DIR escaped the run-only boundary"
    }
    if (@(Get-ChildItem -LiteralPath $RunnerTemp -Filter "rewire-run-*" -ErrorAction SilentlyContinue).Count -ne 0) {
        throw "temporary runner directory survived a successful run"
    }

    $env:REWIRE_TEST_OUTPUT = Join-Path $TemporaryDirectory "default-arguments"
    try {
        & $Runner --asset-base-url $Assets
        if ($LASTEXITCODE -ne 0) { throw "default run returned $LASTEXITCODE" }
    } finally {
        Remove-Item Env:REWIRE_TEST_OUTPUT -ErrorAction SilentlyContinue
    }
    $DefaultLines = @(Get-Content -LiteralPath (Join-Path $TemporaryDirectory "default-arguments"))
    if ($DefaultLines[1] -ne "argument=configure") {
        throw "run.ps1 did not default to configure; fixture output: $($DefaultLines -join ' | ')"
    }

    $TerminalOutput = Join-Path $TemporaryDirectory "terminal-arguments"
    $TerminalStatus = Invoke-WithRedirectedInput `
        -Script $Runner `
        -Arguments @("--asset-base-url", $Assets) `
        -Input "bootstrap-source" `
        -Environment @{ REWIRE_TEST_OUTPUT = $TerminalOutput; REWIRE_TEST_REQUIRE_TERMINAL = "1"; TEMP = $RunnerTemp }
    if ($TerminalStatus -ne 0) {
        throw "run.ps1 did not attach console input; child exit code $TerminalStatus"
    }
    if ((Get-Content -LiteralPath $TerminalOutput) -notcontains "argument=configure") {
        throw "run.ps1 did not preserve the default configure invocation"
    }

    foreach ($Mode in "--token-stdin", "--non-interactive") {
        $ModeName = $Mode.TrimStart("-")
        $InputOutput = Join-Path $TemporaryDirectory "$ModeName-arguments"
        $InputStatus = Invoke-WithRedirectedInput `
            -Script $Runner `
            -Arguments @("--asset-base-url", $Assets, "--", "--fixture-read-stdin", $Mode) `
            -Input "$ModeName-input" `
            -Environment @{ REWIRE_TEST_OUTPUT = $InputOutput; TEMP = $RunnerTemp }
        if ($InputStatus -ne 0 -or (Get-Content -LiteralPath $InputOutput) -notcontains "stdin=$ModeName-input") {
            throw "$Mode did not preserve redirected standard input"
        }
    }

    $Isolated = Join-Path $TemporaryDirectory "isolated"
    New-Item -ItemType Directory -Path $Isolated | Out-Null
    $IsolatedRunner = Join-Path $Isolated "run.ps1"
    Copy-Item -LiteralPath $Runner -Destination $IsolatedRunner
    $env:REWIRE_TEST_OUTPUT = Join-Path $TemporaryDirectory "downloaded-installer-arguments"
    try {
        & $IsolatedRunner `
            --installer-url $Installer `
            --asset-base-url $Assets -- --from-standalone
        if ($LASTEXITCODE -ne 0) { throw "standalone run returned $LASTEXITCODE" }
    } finally {
        Remove-Item Env:REWIRE_TEST_OUTPUT -ErrorAction SilentlyContinue
    }
    if ((Get-Content -LiteralPath (Join-Path $TemporaryDirectory "downloaded-installer-arguments")) -notcontains "argument=--from-standalone") {
        throw "explicit installer source was not used"
    }

    $InstallDirRejected = $false
    try {
        & $Runner --install-dir $PersistentInstall
    } catch {
        $InstallDirRejected = $_.Exception.Message -match 'reserved by the run-only entrypoint'
    }
    if (-not $InstallDirRejected) {
        throw "run-only entrypoint accepted --install-dir"
    }

    Set-Content `
        -LiteralPath (Join-Path $Assets "SHA256SUMS") `
        -Value "$('0' * 64)  $Asset" `
        -Encoding ascii
    $ChecksumOutput = Join-Path $TemporaryDirectory "checksum-arguments"
    $env:REWIRE_TEST_OUTPUT = $ChecksumOutput
    $ChecksumFailed = $false
    try {
        & $Runner --asset-base-url $Assets
    } catch {
        $ChecksumFailed = $_.Exception.Message -match 'checksum mismatch'
    } finally {
        Remove-Item Env:REWIRE_TEST_OUTPUT -ErrorAction SilentlyContinue
    }
    if (-not $ChecksumFailed -or (Test-Path -LiteralPath $ChecksumOutput)) {
        throw "checksum failure started the fixture or unexpectedly succeeded"
    }

    Set-Content -LiteralPath (Join-Path $Assets "SHA256SUMS") -Value "$Digest  $Asset" -Encoding ascii
    $env:REWIRE_TEST_OUTPUT = Join-Path $TemporaryDirectory "exit-arguments"
    try {
        & $Runner --asset-base-url $Assets -- --fixture-exit 23
        $ExitCode = $LASTEXITCODE
    } finally {
        Remove-Item Env:REWIRE_TEST_OUTPUT -ErrorAction SilentlyContinue
    }
    if ($ExitCode -ne 23) {
        throw "runner returned $ExitCode instead of fixture exit code 23"
    }
    $ExitLine = (Get-Content -LiteralPath (Join-Path $TemporaryDirectory "exit-arguments"))[0]
    $ExitExecutable = $ExitLine.Substring("executable=".Length)
    if (Test-Path -LiteralPath $ExitExecutable) {
        throw "temporary executable survived a failed run"
    }

    Write-Output "Windows run-only tests passed."
} finally {
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}

# The non-zero propagation case intentionally leaves LASTEXITCODE set to 23.
exit 0
