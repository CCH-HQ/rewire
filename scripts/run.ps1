Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
# The runner must preserve Rewire's native exit code after cleaning its temporary directory.
$PSNativeCommandUseErrorActionPreference = $false

$Repository = "CCH-HQ/rewire"
$Release = if ($env:REWIRE_RELEASE) { $env:REWIRE_RELEASE } else { "latest" }
$AssetBaseUrl = $env:REWIRE_ASSET_BASE_URL
$DownloadUrl = $env:REWIRE_DOWNLOAD_URL
$ChecksumUrl = $env:REWIRE_CHECKSUM_URL
$ExpectedSha256 = $env:REWIRE_SHA256
$InstallerUrl = $env:REWIRE_INSTALLER_URL
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
        public static extern bool GetHandleInformation(IntPtr handle, out uint flags);

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

        public static bool TryEnableInputInheritance(out IntPtr input, out uint originalFlags) {
            input = GetStdHandle(StandardInputHandle);
            originalFlags = 0;
            if (input == IntPtr.Zero || input == InvalidHandle
                || !GetHandleInformation(input, out originalFlags)) {
                return false;
            }
            if (SetHandleInformation(input, HandleFlagInherit, HandleFlagInherit)) {
                return true;
            }
            input = IntPtr.Zero;
            return false;
        }

        public static void RestoreInputInheritance(IntPtr input, uint originalFlags) {
            SetHandleInformation(input, HandleFlagInherit, originalFlags & HandleFlagInherit);
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

function Invoke-Rewire {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $script:RewireExitCode = 0
    [IntPtr]$OriginalInput = [IntPtr]::Zero
    [IntPtr]$ConsoleInput = [IntPtr]::Zero
    [IntPtr]$InheritedInput = [IntPtr]::Zero
    [uint32]$OriginalInputFlags = 0
    $AttachConsole = [Console]::IsInputRedirected -and
        (Test-RewireUsesTerminalInput -Arguments $Arguments)
    try {
        if ($AttachConsole) {
            Initialize-RewireNativeConsole
            [void][Rewire.NativeConsole]::TryAttachInput(
                [ref]$OriginalInput,
                [ref]$ConsoleInput
            )
        } elseif ([Console]::IsInputRedirected -and
            (Test-RewireConsumesStandardInput -Arguments $Arguments)) {
            Initialize-RewireNativeConsole
            [void][Rewire.NativeConsole]::TryEnableInputInheritance(
                [ref]$InheritedInput,
                [ref]$OriginalInputFlags
            )
        }

        if ($ConsoleInput -ne [IntPtr]::Zero -or $InheritedInput -ne [IntPtr]::Zero) {
            $script:RewireExitCode = Invoke-RewireNativeProcess -Path $Path -Arguments $Arguments
            $global:LASTEXITCODE = $script:RewireExitCode
        } else {
            & $Path @Arguments
            $script:RewireExitCode = $LASTEXITCODE
        }
    } finally {
        if ($ConsoleInput -ne [IntPtr]::Zero) {
            [Rewire.NativeConsole]::RestoreInput($OriginalInput, $ConsoleInput)
        }
        if ($InheritedInput -ne [IntPtr]::Zero) {
            [Rewire.NativeConsole]::RestoreInputInheritance($InheritedInput, $OriginalInputFlags)
        }
    }
}

function Show-Usage {
    @'
Download, verify, and run Rewire without installing it.

Usage:
  run.ps1 [RUNNER OPTIONS] [--] [REWIRE ARGUMENTS...]

Runner options:
  --release <VERSION>       Release to run, for example 0.0.1 or v0.0.1
                            (default: latest)
  --asset-base-url <VALUE>  Release asset URL or local fixture/mirror directory
  --download-url <URL>      Exact platform archive URL or local file
  --checksum-url <URL>      Exact SHA256SUMS URL or local file
  --sha256 <DIGEST>         Expected archive SHA-256 instead of SHA256SUMS
  --installer-url <URL>     install.ps1 URL or local file used by this runner
  -h, --help                Print this help

The verified binary is staged in a private temporary directory and removed
after it exits. With no Rewire arguments, the runner starts `rewire configure`.
All arguments after `--` are passed to Rewire unchanged.
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
        "--installer-url" {
            if ($Index + 1 -ge $args.Count) { throw "--installer-url requires a value" }
            $InstallerUrl = [string]$args[$Index + 1]
            $Index += 2
            continue ParseArguments
        }
        "--install-dir" {
            throw "--install-dir is reserved by the run-only entrypoint"
        }
        { $_ -in "--no-run", "--quiet" } {
            throw "$Argument is an installer-only option"
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

if ($DownloadUrl -and $AssetBaseUrl) {
    throw "--download-url conflicts with --asset-base-url"
}
if ($ExpectedSha256 -and $ChecksumUrl) {
    throw "--sha256 conflicts with --checksum-url"
}

function Copy-InstallerSource {
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
                throw "could not download installer from $Source after 3 attempts: $($_.Exception.Message)"
            }
            Start-Sleep -Seconds $Attempt
        }
    }
}

# Windows PowerShell defaults to older TLS versions on some supported systems.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("rewire-run-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null
try {
    if ($InstallerUrl) {
        $Installer = Join-Path $TemporaryDirectory "install.ps1"
        Copy-InstallerSource -Source $InstallerUrl -Destination $Installer
    } else {
        $SiblingInstaller = Join-Path $PSScriptRoot "install.ps1"
        if (Test-Path -LiteralPath $SiblingInstaller -PathType Leaf) {
            $Installer = $SiblingInstaller
        } else {
            $Installer = Join-Path $TemporaryDirectory "install.ps1"
            Copy-InstallerSource `
                -Source "https://raw.githubusercontent.com/$Repository/master/scripts/install.ps1" `
                -Destination $Installer
        }
    }

    $InstallDir = Join-Path $TemporaryDirectory "bin"
    $InstallArguments = [System.Collections.Generic.List[string]]::new()
    $InstallArguments.Add("--release")
    $InstallArguments.Add($Release)
    if ($AssetBaseUrl) {
        $InstallArguments.Add("--asset-base-url")
        $InstallArguments.Add($AssetBaseUrl)
    }
    if ($DownloadUrl) {
        $InstallArguments.Add("--download-url")
        $InstallArguments.Add($DownloadUrl)
    }
    if ($ChecksumUrl) {
        $InstallArguments.Add("--checksum-url")
        $InstallArguments.Add($ChecksumUrl)
    }
    if ($ExpectedSha256) {
        $InstallArguments.Add("--sha256")
        $InstallArguments.Add($ExpectedSha256)
    }
    $InstallArguments.Add("--install-dir")
    $InstallArguments.Add($InstallDir)
    $InstallArguments.Add("--no-run")
    $InstallArguments.Add("--quiet")

    $InstallArgumentArray = $InstallArguments.ToArray()
    & $Installer @InstallArgumentArray

    $Binary = Join-Path $InstallDir "rewire.exe"
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
        throw "installer did not produce a rewire.exe binary"
    }

    [string[]]$RunArguments = if ($RewireArgs.Count -eq 0) { "configure" } else { $RewireArgs.ToArray() }
    Invoke-Rewire -Path $Binary -Arguments $RunArguments
    $ExitCode = $script:RewireExitCode
} finally {
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}

exit $ExitCode
