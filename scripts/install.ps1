Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
# Capture native exit codes ourselves so PowerShell 7 preference changes do not preempt cleanup.
$PSNativeCommandUseErrorActionPreference = $false

$Repository = "CCH-HQ/rewire"
$Release = if ($env:REWIRE_RELEASE) { $env:REWIRE_RELEASE } else { "latest" }
$InstallDir = $env:REWIRE_INSTALL_DIR
$AssetBaseUrl = $env:REWIRE_ASSET_BASE_URL
$DownloadUrl = $env:REWIRE_DOWNLOAD_URL
$ChecksumUrl = $env:REWIRE_CHECKSUM_URL
$ExpectedSha256 = $env:REWIRE_SHA256
$RunAfterInstall = $true
$Quiet = $false
$RewireArgs = [System.Collections.Generic.List[string]]::new()
$AssetBaseOptionSet = $false
$DownloadOptionSet = $false
$ChecksumOptionSet = $false
$Sha256OptionSet = $false

function Test-RewireUsesTerminalInput {
    param([string[]]$Arguments)
    if ($Arguments -contains "--token-stdin" -or $Arguments -contains "--non-interactive") {
        return $false
    }
    if ($Arguments -contains "--help" -or $Arguments -contains "-h" -or
        $Arguments -contains "--version" -or $Arguments -contains "-V") {
        return $false
    }
    $ValueOptions = @("--baseurl", "--token", "--client", "--model", "--model-name", "--sdk", "--home")
    $SkipValue = $false
    foreach ($Argument in $Arguments) {
        if ($SkipValue) {
            $SkipValue = $false
            continue
        }
        if ($Argument -in $ValueOptions) {
            $SkipValue = $true
            continue
        }
        if ($Argument -match '^--(?:baseurl|token|client|model|model-name|sdk|home)=') { continue }
        if ($Argument.StartsWith("-")) { continue }
        if ($Argument -in "doctor", "backup", "completions") { return $false }
        if ($Argument -in "configure", "tui", "plan") { return $true }
        if ($Argument -in "rollback", "remove") {
            return $Arguments -notcontains "--yes" -and $Arguments -notcontains "--json"
        }
    }
    return $true
}

function Test-RewireConsumesStandardInput {
    param([string[]]$Arguments)
    return $Arguments -contains "--token-stdin" -or $Arguments -contains "--non-interactive"
}

function Initialize-RewireNativeConsole {
    if ("Rewire.NativeConsole" -as [type]) { return }
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Rewire {
    public static class NativeConsole {
        private const int StandardInputHandle = -10;
        private const uint GenericRead = 0x80000000;
        private const uint ShareReadWrite = 3;
        private const uint OpenExisting = 3;
        private const uint HandleFlagInherit = 1;
        private static readonly IntPtr InvalidHandle = new IntPtr(-1);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateFile(
            string name, uint access, uint share, IntPtr security,
            uint creation, uint flags, IntPtr template);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetStdHandle(int handle, IntPtr value);

        [DllImport("kernel32.dll")]
        public static extern IntPtr GetStdHandle(int handle);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetHandleInformation(IntPtr handle, uint mask, uint flags);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool CloseHandle(IntPtr handle);

        public static bool TryAttachInput(out IntPtr original, out IntPtr console) {
            original = GetStdHandle(StandardInputHandle);
            console = CreateFile(
                "CONIN$", GenericRead, ShareReadWrite, IntPtr.Zero,
                OpenExisting, 0, IntPtr.Zero);
            if (console == InvalidHandle) {
                console = IntPtr.Zero;
                return false;
            }
            if (SetHandleInformation(console, HandleFlagInherit, HandleFlagInherit)
                && SetStdHandle(StandardInputHandle, console)) {
                return true;
            }
            CloseHandle(console);
            console = IntPtr.Zero;
            return false;
        }

        public static void RestoreInput(IntPtr original, IntPtr console) {
            SetStdHandle(StandardInputHandle, original);
            CloseHandle(console);
        }
    }
}
'@
}

function ConvertTo-RewireNativeArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Argument)

    if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') { return $Argument }
    $Builder = [Text.StringBuilder]::new()
    [void]$Builder.Append('"')
    $Backslashes = 0
    foreach ($Character in $Argument.ToCharArray()) {
        if ($Character -eq '\') {
            $Backslashes += 1
        } elseif ($Character -eq '"') {
            [void]$Builder.Append(('\' * (2 * $Backslashes + 1)))
            [void]$Builder.Append('"')
            $Backslashes = 0
        } else {
            if ($Backslashes -gt 0) { [void]$Builder.Append(('\' * $Backslashes)) }
            [void]$Builder.Append($Character)
            $Backslashes = 0
        }
    }
    if ($Backslashes -gt 0) { [void]$Builder.Append(('\' * (2 * $Backslashes))) }
    [void]$Builder.Append('"')
    return $Builder.ToString()
}

function Invoke-RewireNativeProcess {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $StartInfo = [Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = $Path
    $StartInfo.UseShellExecute = $false
    $StartInfo.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-RewireNativeArgument -Argument $_
    }) -join ' ')
    $Process = [Diagnostics.Process]::Start($StartInfo)
    $Process.WaitForExit()
    return $Process.ExitCode
}

function Invoke-RewireRedirectedInputProcess {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $StartInfo = [Diagnostics.ProcessStartInfo]::new()
    $StartInfo.FileName = $Path
    $StartInfo.UseShellExecute = $false
    $StartInfo.RedirectStandardInput = $true
    $StartInfo.Arguments = (($Arguments | ForEach-Object {
        ConvertTo-RewireNativeArgument -Argument $_
    }) -join ' ')
    $Process = [Diagnostics.Process]::Start($StartInfo)
    $SourceInput = [Console]::OpenStandardInput()
    $SourceInput.CopyTo($Process.StandardInput.BaseStream)
    $Process.StandardInput.Close()
    $Process.WaitForExit()
    return $Process.ExitCode
}

function Invoke-Rewire {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $script:RewireExitCode = 0
    [IntPtr]$OriginalInput = [IntPtr]::Zero
    [IntPtr]$ConsoleInput = [IntPtr]::Zero
    $AttachConsole = [Console]::IsInputRedirected -and
        (Test-RewireUsesTerminalInput -Arguments $Arguments)
    try {
        if ($AttachConsole) {
            Initialize-RewireNativeConsole
            [void][Rewire.NativeConsole]::TryAttachInput(
                [ref]$OriginalInput,
                [ref]$ConsoleInput
            )
        }

        if ($ConsoleInput -ne [IntPtr]::Zero) {
            $script:RewireExitCode = Invoke-RewireNativeProcess -Path $Path -Arguments $Arguments
            $global:LASTEXITCODE = $script:RewireExitCode
        } elseif ([Console]::IsInputRedirected -and
            (Test-RewireConsumesStandardInput -Arguments $Arguments)) {
            $script:RewireExitCode = Invoke-RewireRedirectedInputProcess `
                -Path $Path `
                -Arguments $Arguments
            $global:LASTEXITCODE = $script:RewireExitCode
        } else {
            & $Path @Arguments
            $script:RewireExitCode = $LASTEXITCODE
        }
    } finally {
        if ($ConsoleInput -ne [IntPtr]::Zero) {
            [Rewire.NativeConsole]::RestoreInput($OriginalInput, $ConsoleInput)
        }
    }
}

function Show-Usage {
    @'
Install Rewire from GitHub Releases and optionally run it.

Usage:
  install.ps1 [INSTALLER OPTIONS] [--] [REWIRE ARGUMENTS...]

Installer options:
  --release <VERSION>       Release to install, for example 0.0.1 or v0.0.1
                            (default: latest)
  --install-dir <DIR>       Destination directory
                            (default: `%LOCALAPPDATA%\Programs\rewire\bin)
  --asset-base-url <VALUE>  Release asset URL or local fixture/mirror directory
  --download-url <URL>      Exact platform archive URL or local file
  --checksum-url <URL>      Exact SHA256SUMS URL or local file
  --sha256 <DIGEST>         Expected archive SHA-256 instead of SHA256SUMS
  --no-run                  Install without starting Rewire
  --quiet                   Suppress installation status and PATH notices
  -h, --help                Print this help

With no Rewire arguments, the installer starts `rewire configure`. Otherwise,
all remaining arguments are passed to Rewire unchanged. Put `--` before Rewire
arguments when an argument could be confused with an installer option.
'@
}

:ParseArguments for ($Index = 0; $Index -lt $args.Count;) {
    $Argument = [string]$args[$Index]
    switch ($Argument) {
        "--release" {
            if ($Index + 1 -ge $args.Count) { throw "--release requires a value" }
            $Release = [string]$args[$Index + 1]
            $Index += 2
            continue ParseArguments
        }
        "--install-dir" {
            if ($Index + 1 -ge $args.Count) { throw "--install-dir requires a value" }
            $InstallDir = [string]$args[$Index + 1]
            $Index += 2
            continue ParseArguments
        }
        "--asset-base-url" {
            if ($Index + 1 -ge $args.Count) { throw "--asset-base-url requires a value" }
            if ($DownloadOptionSet) { throw "--asset-base-url conflicts with --download-url" }
            $AssetBaseUrl = [string]$args[$Index + 1]
            $DownloadUrl = $null
            $AssetBaseOptionSet = $true
            $Index += 2
            continue ParseArguments
        }
        "--download-url" {
            if ($Index + 1 -ge $args.Count) { throw "--download-url requires a value" }
            if ($AssetBaseOptionSet) { throw "--download-url conflicts with --asset-base-url" }
            $DownloadUrl = [string]$args[$Index + 1]
            $AssetBaseUrl = $null
            $DownloadOptionSet = $true
            $Index += 2
            continue ParseArguments
        }
        "--checksum-url" {
            if ($Index + 1 -ge $args.Count) { throw "--checksum-url requires a value" }
            if ($Sha256OptionSet) { throw "--checksum-url conflicts with --sha256" }
            $ChecksumUrl = [string]$args[$Index + 1]
            $ExpectedSha256 = $null
            $ChecksumOptionSet = $true
            $Index += 2
            continue ParseArguments
        }
        "--sha256" {
            if ($Index + 1 -ge $args.Count) { throw "--sha256 requires a value" }
            if ($ChecksumOptionSet) { throw "--sha256 conflicts with --checksum-url" }
            $ExpectedSha256 = [string]$args[$Index + 1]
            $ChecksumUrl = $null
            $Sha256OptionSet = $true
            $Index += 2
            continue ParseArguments
        }
        "--no-run" {
            $RunAfterInstall = $false
            $Index += 1
            continue ParseArguments
        }
        "--quiet" {
            $Quiet = $true
            $Index += 1
            continue ParseArguments
        }
        { $_ -in "-h", "--help" } {
            Show-Usage
            return
        }
        "--" {
            $Index += 1
            while ($Index -lt $args.Count) {
                $RewireArgs.Add([string]$args[$Index])
                $Index += 1
            }
            break ParseArguments
        }
        default {
            while ($Index -lt $args.Count) {
                $RewireArgs.Add([string]$args[$Index])
                $Index += 1
            }
            break ParseArguments
        }
    }
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($env:LOCALAPPDATA) {
        $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\rewire\bin"
    } elseif ($HOME) {
        $InstallDir = Join-Path $HOME ".local\bin"
    } else {
        throw "HOME and LOCALAPPDATA are unset; pass --install-dir"
    }
}

$IsWindowsHost = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
if (-not $IsWindowsHost) {
    throw "install.ps1 supports Windows; use install.sh on Unix"
}
$Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($Architecture -ne "X64") {
    throw "unsupported Windows architecture: $Architecture"
}

$Asset = "rewire-x86_64-pc-windows-msvc.zip"
if ($DownloadUrl -and $AssetBaseUrl) {
    throw "--download-url conflicts with --asset-base-url"
}
if ($ExpectedSha256 -and $ChecksumUrl) {
    throw "--sha256 conflicts with --checksum-url"
}
if (-not $DownloadUrl -and [string]::IsNullOrWhiteSpace($AssetBaseUrl)) {
    if ($Release -eq "latest") {
        $ReleasePath = "latest/download"
    } else {
        $Version = if ($Release.StartsWith("v")) { $Release.Substring(1) } else { $Release }
        if ($Version -notmatch '^[0-9A-Za-z][0-9A-Za-z._-]*$') {
            throw "invalid release: $Release"
        }
        $ReleasePath = "download/v$Version"
    }
    $AssetBaseUrl = "https://github.com/$Repository/releases/$ReleasePath"
}

function Join-AssetSource {
    param(
        [Parameter(Mandatory = $true)][string]$Base,
        [Parameter(Mandatory = $true)][string]$Name
    )
    if (Test-Path -LiteralPath $Base -PathType Container) {
        return Join-Path $Base $Name
    }
    return "$($Base.TrimEnd('/'))/$Name"
}

function Copy-Source {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (Test-Path -LiteralPath $Source -PathType Leaf) {
        Copy-Item -LiteralPath $Source -Destination $Destination
        return
    }

    $Request = @{ Uri = $Source; OutFile = $Destination }
    if ($PSVersionTable.PSVersion.Major -lt 6) {
        $Request.UseBasicParsing = $true
    }
    for ($Attempt = 1; $Attempt -le 3; $Attempt += 1) {
        try {
            Invoke-WebRequest @Request
            return
        } catch {
            if ($Attempt -eq 3) {
                throw "could not download $Source after 3 attempts: $($_.Exception.Message)"
            }
            Start-Sleep -Seconds $Attempt
        }
    }
}

# Windows PowerShell defaults to older TLS versions on some supported systems.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("rewire-install-" + [guid]::NewGuid())
$InstallTemporary = $null
New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null
try {
    $Archive = Join-Path $TemporaryDirectory $Asset
    $Checksums = Join-Path $TemporaryDirectory "SHA256SUMS"
    $ArchiveSource = if ($DownloadUrl) {
        $DownloadUrl
    } else {
        Join-AssetSource -Base $AssetBaseUrl -Name $Asset
    }
    Copy-Source -Source $ArchiveSource -Destination $Archive

    $Expected = $ExpectedSha256
    if (-not $Expected) {
        if (-not $ChecksumUrl) {
            if ($DownloadUrl) {
                if (Test-Path -LiteralPath $DownloadUrl -PathType Leaf) {
                    $ChecksumUrl = Join-Path (Split-Path -Parent $DownloadUrl) "SHA256SUMS"
                } else {
                    $ChecksumUrl = [Uri]::new([Uri]$DownloadUrl, "SHA256SUMS").AbsoluteUri
                }
            } else {
                $ChecksumUrl = Join-AssetSource -Base $AssetBaseUrl -Name "SHA256SUMS"
            }
        }
        Copy-Source -Source $ChecksumUrl -Destination $Checksums
        $Pattern = '^([0-9A-Fa-f]{64})\s+\*?' + [regex]::Escape($Asset) + '$'
        foreach ($Line in Get-Content -LiteralPath $Checksums) {
            $Match = [regex]::Match($Line.Trim(), $Pattern)
            if ($Match.Success) {
                $Expected = $Match.Groups[1].Value
                break
            }
        }
    }
    if (-not $Expected -or $Expected -notmatch '^[0-9A-Fa-f]{64}$') {
        throw "expected SHA-256 is missing or invalid for $Asset"
    }
    $Expected = $Expected.ToUpperInvariant()
    $Actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($Actual -ne $Expected) {
        throw "checksum mismatch for $Asset"
    }

    # Extract only the expected binary; custom archives cannot write arbitrary sibling paths.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $SourceBinary = Join-Path $TemporaryDirectory "rewire.exe"
    $Zip = [System.IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        $BinaryEntries = @($Zip.Entries | Where-Object { $_.FullName -in "rewire.exe", "./rewire.exe" })
        if ($BinaryEntries.Count -ne 1 -or $BinaryEntries[0].Length -eq 0) {
            throw "$Asset must contain exactly one rewire.exe binary"
        }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($BinaryEntries[0], $SourceBinary, $true)
    } finally {
        $Zip.Dispose()
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $Destination = Join-Path $InstallDir "rewire.exe"
    $InstallTemporary = Join-Path $InstallDir (".rewire-" + [guid]::NewGuid() + ".exe")
    Copy-Item -LiteralPath $SourceBinary -Destination $InstallTemporary
    Move-Item -LiteralPath $InstallTemporary -Destination $Destination -Force
    $InstallTemporary = $null
} finally {
    if ($InstallTemporary -and (Test-Path -LiteralPath $InstallTemporary)) {
        Remove-Item -Force -LiteralPath $InstallTemporary
    }
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}

if (-not $Quiet) {
    Write-Output "Installed rewire to $Destination"
    $PathEntries = @($env:PATH -split [IO.Path]::PathSeparator)
    if ($InstallDir -notin $PathEntries) {
        Write-Warning "Add $InstallDir to PATH to run rewire directly."
    }
}

if (-not $RunAfterInstall) {
    return
}

$RunArguments = if ($RewireArgs.Count -eq 0) { @("configure") } else { $RewireArgs.ToArray() }
Invoke-Rewire -Path $Destination -Arguments $RunArguments
if ($script:RewireExitCode -ne 0) {
    throw "rewire exited with code $script:RewireExitCode"
}
