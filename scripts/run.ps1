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
    $InstallerArgs = [System.Collections.Generic.List[string]]::new()
    $InstallerArgs.Add("--release")
    $InstallerArgs.Add($Release)
    if ($AssetBaseUrl) {
        $InstallerArgs.Add("--asset-base-url")
        $InstallerArgs.Add($AssetBaseUrl)
    }
    if ($DownloadUrl) {
        $InstallerArgs.Add("--download-url")
        $InstallerArgs.Add($DownloadUrl)
    }
    if ($ChecksumUrl) {
        $InstallerArgs.Add("--checksum-url")
        $InstallerArgs.Add($ChecksumUrl)
    }
    if ($ExpectedSha256) {
        $InstallerArgs.Add("--sha256")
        $InstallerArgs.Add($ExpectedSha256)
    }
    $InstallerArgs.Add("--install-dir")
    $InstallerArgs.Add($InstallDir)
    $InstallerArgs.Add("--no-run")
    $InstallerArgs.Add("--quiet")

    $InstallerArgumentArray = $InstallerArgs.ToArray()
    & $Installer @InstallerArgumentArray

    $Binary = Join-Path $InstallDir "rewire.exe"
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
        throw "installer did not produce a rewire.exe binary"
    }

    $RunArguments = if ($RewireArgs.Count -eq 0) { @("configure") } else { $RewireArgs.ToArray() }
    & $Binary @RunArguments
    $ExitCode = $LASTEXITCODE
} finally {
    Remove-Item -Recurse -Force -LiteralPath $TemporaryDirectory -ErrorAction SilentlyContinue
}

exit $ExitCode
