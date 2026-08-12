# Installation, one-time runs, and embedding

Rewire ships persistent installers and one-time runners for POSIX shell and PowerShell. All four
entrypoints share the installer implementation for the platform-sensitive operations:

1. detect the current platform and select its release archive;
2. download the archive and checksum manifest;
3. verify SHA-256 before changing the existing installation;
4. extract and atomically stage the destination binary; and
5. run `rewire configure` by default, or pass supplied arguments to Rewire unchanged.

`install.sh` and `install.ps1` persist the binary. The default Unix destination is
`$HOME/.local/bin/rewire`; the default Windows destination is
`%LOCALAPPDATA%\Programs\rewire\bin\rewire.exe`. A destination outside `PATH` is allowed and
reported after installation. `run.sh` and `run.ps1` instead use a private temporary directory and
remove the binary after Rewire exits, including non-zero exits and bootstrap failures.

## Shared download options

Installers and runners accept the same release source and integrity options:

| Option | Behavior |
| --- | --- |
| `--release <VERSION>` | Use `latest` or a specific version such as `0.0.1` or `v0.0.1` |
| `--asset-base-url <VALUE>` | Use a mirror URL or local directory containing all release assets |
| `--download-url <URL>` | Use one exact platform-specific archive URL or local file |
| `--checksum-url <URL>` | Use an exact `SHA256SUMS` URL or local file |
| `--sha256 <DIGEST>` | Verify against a supplied archive digest instead of downloading `SHA256SUMS` |
| `--` | End bootstrap options and pass every remaining argument to Rewire |

Equivalent environment variables are `REWIRE_RELEASE`, `REWIRE_ASSET_BASE_URL`,
`REWIRE_DOWNLOAD_URL`, `REWIRE_CHECKSUM_URL`, and `REWIRE_SHA256`. Command-line options take
precedence.

`--download-url` and `--asset-base-url` are mutually exclusive. `--sha256` and `--checksum-url`
are also mutually exclusive. When an exact download URL is supplied without either checksum
option, the installer requests `SHA256SUMS` beside that URL. Signed or routed download endpoints
should pass `--sha256` or an explicit `--checksum-url` instead of relying on the sibling path.

Persistent installers additionally accept `--install-dir <DIR>`, `--no-run`, and `--quiet`.
`REWIRE_INSTALL_DIR` provides the default installation directory. One-time runners accept
`--installer-url <URL>` or `REWIRE_INSTALLER_URL` when a sibling `install.sh` or `install.ps1` is
not available or a trusted hosted installer should be selected explicitly. Runners reserve the
temporary install directory internally, so installer-only options are rejected before download.

## Unix persistent install

The installer recovers the controlling terminal when its own source arrives through a pipe. This
keeps partial interactive configuration working with `curl | sh`; explicit `--token-stdin` and
`--non-interactive` calls continue to consume their original standard input.

```bash
# Install latest and open the workflow.
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.sh | sh

# Install a fixed release without running it.
sh /tmp/rewire-install.sh --release 0.0.1 --install-dir "$HOME/bin" --no-run

# Install and run a non-interactive configuration.
printf '%s\n' "$REWIRE_TOKEN" | sh /tmp/rewire-install.sh -- \
  --baseurl https://api.example.com \
  --token-stdin \
  --client claude,codex \
  --yes
```

## Unix one-time run

```bash
# Download, verify, open the workflow, and clean up after it exits.
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/run.sh | sh

# Run a fixed release without writing to $HOME/.local/bin.
sh /tmp/rewire-run.sh --release 0.0.1 -- doctor

# A standalone runner can obtain the installer from a trusted HTTPS location.
sh /tmp/rewire-run.sh \
  --installer-url https://downloads.example/rewire/install.sh \
  --asset-base-url https://downloads.example/rewire/current -- doctor
```

## Windows PowerShell persistent install

```powershell
# Install latest and open the workflow.
Invoke-RestMethod `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.ps1 | `
  Invoke-Expression

# Download first when passing bootstrap or Rewire arguments.
$installer = Join-Path $env:TEMP "rewire-install.ps1"
Invoke-WebRequest `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.ps1 `
  -OutFile $installer

# Install only.
& $installer --release 0.0.1 --install-dir "$HOME\bin" --no-run

# Install and configure through normal Rewire arguments.
& $installer -- `
  --baseurl https://api.example.com `
  --client claude,codex `
  --yes
```

## Windows PowerShell one-time run

```powershell
Invoke-RestMethod `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/run.ps1 | `
  Invoke-Expression

$runner = Join-Path $env:TEMP "rewire-run.ps1"
Invoke-WebRequest `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/run.ps1 `
  -OutFile $runner

# Run a fixed release without writing to the user Programs directory.
& $runner --release 0.0.1 -- doctor
```

## Sub2API embedding

A frontend can embed a one-time runner when configuration should not permanently install Rewire.
When the download service exposes release filenames unchanged, the runner delegates to the
installer, which appends the selected archive name and `SHA256SUMS`:

```bash
sh /tmp/rewire-run.sh \
  --asset-base-url https://sub2api.example/rewire/releases/current -- \
  --baseurl https://api.example.com --client claude,codex --yes
```

For a backend-generated or expiring platform-specific URL, embed the exact archive URL and digest:

```bash
sh /tmp/rewire-run.sh \
  --download-url "$SIGNED_ARCHIVE_URL" \
  --sha256 "$ARCHIVE_SHA256" -- \
  --baseurl https://api.example.com --client opencode --model gpt-5.5 --yes
```

```powershell
& $runner `
  --download-url $SignedArchiveUrl `
  --sha256 $ArchiveSha256 -- `
  --baseurl https://api.example.com `
  --client opencode `
  --model gpt-5.5 `
  --yes
```

The frontend should avoid embedding API tokens in a command string. Use the guided prompt,
`REWIRE_TOKEN`, or `--token-stdin` so shell history and process listings do not retain credentials.
Use the persistent installer instead only when leaving a reusable `rewire` command on the user's
machine is the intended product behavior.

## Docker end-to-end verification

The live installer test builds a release archive inside the pinned Rust container, serves that
archive and `SHA256SUMS` over an isolated Docker network, installs it in a fresh container, and
configures all five clients under a temporary Home. It then checks adapter output, a second
idempotent plan, credential file permissions, transaction redaction, and an authenticated
`/v1/models` response. By default it also installs pinned official Claude Code, Codex, OpenCode,
Hermes Agent, and OpenClaw CLIs, makes one real model call through each Rewire-generated
configuration, validates each native success record, and scans every captured runtime file for the
API token. The host's real client configuration is never mounted.

Place the API token and base URL in ignored local files, then run:

```bash
mkdir -p tmp
(umask 077 && printf '%s' "$REWIRE_TOKEN" > tmp/key)
printf '%s' 'https://api.example.com/' > tmp/domain
sh scripts/tests/install-docker-e2e.sh
```

Alternate paths can be supplied with `--key-file` and `--domain-file`. Use `--skip-api-probe`
to omit only the authenticated Models endpoint check, and `--skip-client-calls` to omit the real
client runtime layer. An offline installer/configuration run needs both flags. The token is mounted
read-only and sent to Rewire through standard input, never through Docker arguments, environment
variables, image layers, or logs.

## Platform assets

| Platform | Release asset |
| --- | --- |
| macOS arm64 | `rewire-aarch64-apple-darwin.tar.gz` |
| macOS x86-64 | `rewire-x86_64-apple-darwin.tar.gz` |
| Linux arm64 | `rewire-aarch64-unknown-linux-gnu.tar.gz` |
| Linux x86-64 | `rewire-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `rewire-x86_64-pc-windows-msvc.zip` |

Windows arm64 is rejected because the release workflow does not currently publish a native
Windows arm64 artifact. Explicit rejection is preferable to installing an incompatible binary.
