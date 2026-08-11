# Installation and embedding

Rewire ships separate POSIX shell and PowerShell bootstrap installers. Both installers:

1. detect the current platform and select its release archive;
2. download the archive and checksum manifest;
3. verify SHA-256 before changing the existing installation;
4. extract and atomically replace the destination binary; and
5. run `rewire configure` by default, or pass supplied arguments to Rewire unchanged.

The default Unix destination is `$HOME/.local/bin/rewire`. The default Windows destination is
`%LOCALAPPDATA%\Programs\rewire\bin\rewire.exe`. A destination outside `PATH` is allowed and
reported after installation.

## Installer options

The two scripts expose the same options:

| Option | Behavior |
| --- | --- |
| `--release <VERSION>` | Install `latest` or a specific version such as `0.1.0` or `v0.1.0` |
| `--install-dir <DIR>` | Override the platform default destination directory |
| `--asset-base-url <VALUE>` | Use a mirror URL or local directory containing all release assets |
| `--download-url <URL>` | Use one exact platform-specific archive URL or local file |
| `--checksum-url <URL>` | Use an exact `SHA256SUMS` URL or local file |
| `--sha256 <DIGEST>` | Verify against a supplied archive digest instead of downloading `SHA256SUMS` |
| `--no-run` | Install the binary without starting Rewire |
| `--` | End installer options and pass every remaining argument to Rewire |

Equivalent environment variables are `REWIRE_RELEASE`, `REWIRE_INSTALL_DIR`,
`REWIRE_ASSET_BASE_URL`, `REWIRE_DOWNLOAD_URL`, `REWIRE_CHECKSUM_URL`, and `REWIRE_SHA256`.
Command-line options take precedence.

`--download-url` and `--asset-base-url` are mutually exclusive. `--sha256` and `--checksum-url`
are also mutually exclusive. When an exact download URL is supplied without either checksum
option, the installer requests `SHA256SUMS` beside that URL. Signed or routed download endpoints
should pass `--sha256` or an explicit `--checksum-url` instead of relying on the sibling path.

## Unix

Download before execution when the installer should open the interactive workflow. Piping the
script directly into `sh` consumes terminal stdin, which makes an interactive prompt unavailable.

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.sh \
  -o /tmp/rewire-install.sh

# Install latest and open the workflow.
sh /tmp/rewire-install.sh

# Install a fixed release without running it.
sh /tmp/rewire-install.sh --release 0.1.0 --install-dir "$HOME/bin" --no-run

# Install and run a non-interactive configuration.
printf '%s\n' "$REWIRE_TOKEN" | sh /tmp/rewire-install.sh -- \
  --baseurl https://api.example.com \
  --token-stdin \
  --client claude,codex \
  --yes
```

## Windows PowerShell

```powershell
$installer = Join-Path $env:TEMP "rewire-install.ps1"
Invoke-WebRequest `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.ps1 `
  -OutFile $installer

# Install latest and open the workflow.
& $installer

# Install only.
& $installer --release 0.1.0 --install-dir "$HOME\bin" --no-run

# Install and configure through normal Rewire arguments.
& $installer -- `
  --baseurl https://api.example.com `
  --client claude,codex `
  --yes
```

## Sub2API embedding

A frontend can embed one platform-neutral mirror command when its download service exposes the
release filenames unchanged. The installer appends the selected archive name and `SHA256SUMS`:

```bash
sh /tmp/rewire-install.sh \
  --asset-base-url https://sub2api.example/rewire/releases/current -- \
  --baseurl https://api.example.com --client claude,codex --yes
```

For a backend-generated or expiring platform-specific URL, embed the exact archive URL and digest:

```bash
sh /tmp/rewire-install.sh \
  --download-url "$SIGNED_ARCHIVE_URL" \
  --sha256 "$ARCHIVE_SHA256" -- \
  --baseurl https://api.example.com --client opencode --model gpt-5.5 --yes
```

```powershell
& $installer `
  --download-url $SignedArchiveUrl `
  --sha256 $ArchiveSha256 -- `
  --baseurl https://api.example.com `
  --client opencode `
  --model gpt-5.5 `
  --yes
```

The frontend should avoid embedding API tokens in a command string. Use the guided prompt,
`REWIRE_TOKEN`, or `--token-stdin` so shell history and process listings do not retain credentials.

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
