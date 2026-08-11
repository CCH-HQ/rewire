# Rewire

Rewire is a vendor-neutral Rust CLI with a guided interactive workflow for connecting local AI coding clients to compatible API endpoints without destroying existing configuration.

## What ships

- Shared planner and transaction engine used by CLI, the guided workflow, and automation. One client may contribute a main config and a restricted credential target; plans carry prepared before/after bytes so apply cannot silently overwrite a file changed after planning.
- Post-write verifier that re-parses the bytes on disk before the transaction is committed.
- Client recipes for Claude Code, Codex, OpenCode, Hermes Agent, and OpenClaw.
- Ten sanitized fixture variants derived from official configuration references and local Claude/Codex topology, with source provenance recorded beside the fixtures.
- CST field-level merges for JSON/JSONC/JSON5, TOML, YAML, and dotenv while retaining unknown fields, comments, indentation, and trailing-comma style where the format supports it.
- Human-readable command output by default, opt-in stable JSON with `--json`, token redaction, SHA-256 before/after hashes, exclusive transaction locking, authenticated-encrypted backups, atomic replacement, and field-level three-way rollback.
- Home override support for fixtures and CI through `--home`.
- Environment-aware configuration locations, local executable/version diagnostics, and release builds for macOS, Linux, and Windows with checksums, keyless signatures, SBOMs, and GitHub provenance attestations.
- Credential-isolated Docker E2E that downloads the release asset and verifies real Claude Code, Codex, OpenCode, Hermes Agent, and OpenClaw model calls through the configured gateway.

## Quick start

```bash
cargo run -- --baseurl https://api.example.com --token-stdin --client claude --yes
printf '%s\n' "$REWIRE_TOKEN" | rewire --baseurl https://api.example.com --token-stdin --client codex --yes
rewire --baseurl https://api.example.com --token TOKEN --client claude,opencode --dry-run --json
rewire --baseurl https://api.example.com --token-stdin --client opencode,openclaw --model gpt-5.5 --model-name "GPT-5.5" --sdk openai --yes
rewire --baseurl https://api.example.com --token-stdin --client claude,codex plan
rewire doctor
rewire doctor --json
rewire backup list --json
rewire rollback TX_ID --json
rewire rollback --yes --json
rewire remove --client opencode,openclaw --yes
rewire completions zsh > ~/.zfunc/_rewire
rewire configure --no-color
```

## Install permanently or run once

Both bootstrap paths select the current OS and architecture and verify the release archive against
`SHA256SUMS`. Use `install` to keep Rewire in the normal user binary directory, or `run` to stage it
in a private temporary directory for one invocation and remove it afterward. Download the script
first so interactive `configure` keeps terminal stdin.

Unix persistent install:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.sh \
  -o /tmp/rewire-install.sh
sh /tmp/rewire-install.sh
```

Unix one-time run:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/run.sh \
  -o /tmp/rewire-run.sh
sh /tmp/rewire-run.sh
```

Windows PowerShell persistent install:

```powershell
$installer = Join-Path $env:TEMP "rewire-install.ps1"
Invoke-WebRequest `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/install.ps1 `
  -OutFile $installer
& $installer
```

Windows PowerShell one-time run:

```powershell
$runner = Join-Path $env:TEMP "rewire-run.ps1"
Invoke-WebRequest `
  https://raw.githubusercontent.com/CCH-HQ/rewire/master/scripts/run.ps1 `
  -OutFile $runner
& $runner
```

Arguments after `--` are passed to Rewire instead of opening the default workflow. The same
release, mirror, direct-download, and checksum options are accepted by install and run entrypoints:

```bash
sh /tmp/rewire-run.sh -- \
  --baseurl https://api.example.com --client claude,codex --yes
```

This supports commands embedded by a Sub2API frontend without permanently installing a binary or
hard-coding GitHub as the binary source:

```bash
sh /tmp/rewire-run.sh \
  --download-url "$SIGNED_ARCHIVE_URL" --sha256 "$ARCHIVE_SHA256" -- \
  --baseurl https://api.example.com --client opencode --model gpt-5.5 --yes
```

See [installation and embedding](docs/installation.md) for PowerShell equivalents, mirror URL
behavior, environment variables, platform asset names, persistent install mode, and cleanup
guarantees for one-time runs.

`--token` is convenient but can be visible in shell history and process listings. Prefer `--token-stdin`, `REWIRE_TOKEN`, or the guided workflow (`rewire configure`). Complete tokens are excluded from plans, manifests, diagnostics, and the custom `Secret` debug representation.

`--model` is required when OpenCode, Hermes, or OpenClaw is selected. It is the provider-native model ID, not a display label or a qualified client reference. `--sdk` selects the OpenCode provider protocol (`openai`, `anthropic`, `google`, or `openai-compatible`); common GPT, Claude, and Gemini IDs are inferred when it is omitted. A single OpenCode model reuses the native `openai` or `anthropic` provider where applicable; Google and compatible single-model selections use the isolated `rewire` provider. `--model-name` applies only to a custom OpenCode entry or to OpenClaw; a value supplied for an OpenCode native provider is reported and ignored. Hermes uses `model.default`, `model.provider`, and the keyed `providers.rewire` schema, derives its native transport from the selected model family, while OpenClaw uses `agents.defaults.model.primary: "rewire/<id>"`. Claude retains its current model selection, and Codex uses the isolated `rewire.config.toml` profile-v2 layer selected with `--profile rewire`. See [client compatibility](docs/client-compatibility.md) for credential locations, source evidence, and known upstream documentation drift.

For a bare gateway origin, adapters write the protocol request root their client actually consumes:
Codex and OpenAI-compatible routes use `/v1`, Google routes use `/v1beta`, and Claude/Anthropic
origin-style routes let the client append `/v1/messages`. OpenClaw Add all catalogs keep this on
each model through its own `api` and `baseUrl` instead of treating GPT, Claude, Gemini, and generic
compatible models as one OpenAI Completions protocol. Explicit routing paths are preserved.

Running `rewire` without arguments opens the workflow only when both standard input and standard output are terminals. Pipes and automation fail fast with the missing CLI input instead of waiting for prompts; `--non-interactive` makes that intent explicit. The previous `rewire tui` spelling remains an alias for `rewire configure`.

Doctor, plan, apply, remove, rollback, and backup commands render concise operator-facing text by default. Doctor reports all supported clients, selected config paths, local executable versions, and relevant environment variable names while hiding their values. Plans use numbered semantic markers for created, updated, deleted, unchanged, review-required, and blocked targets; headings and identifiers are cyan, successful states are green, warnings are yellow, and failures are red. `rollback` accepts an optional transaction ID: on a terminal it asks whether to restore the latest committed, still-available transaction; `--yes` selects that latest transaction without prompting, while JSON and non-interactive invocations require either an explicit ID or `--yes`. A successfully rolled-back transaction is removed from the default latest-transaction candidate list. `--json` switches successful results and runtime errors to stable machine-readable JSON. Color is emitted only to a terminal; `--no-color` and `NO_COLOR` also apply to Clap's early help and validation output.

An existing adapter target at the same normalized URL is idempotent. A different URL remains a prepared modification but is also numbered `[REVIEW]`; the guided workflow's final selection or an explicit `--yes` accepts that replacement. Syntax errors, read-only files, symlinks, paths outside the selected Home, and concurrent edits remain blocking. `remove --client ...` uses the same review and transaction path, removes only adapter-owned fields, and deletes only dedicated secret files.

The workflow first presents a multi-select list for one or more detected clients, then asks for the compatible base URL and an ASCII-masked token. A required model-ID prompt appears only when OpenCode, Hermes, or OpenClaw is selected; Claude/Codex-only workflows skip it. The `*` mask advances with the cursor so secret input has visible progress without exposing token characters. It generates a numbered list of file modifications and conflicts before presenting a single final choice: apply the numbered plan, return to edit the inputs, or cancel. Existing provider URLs or selected models that would be replaced appear as numbered review items. Malformed, read-only, and symlinked targets are blocking items; while any are present, the apply choice is omitted. If a file changes after review, the workflow offers to rebuild the plan, edit the inputs, or stop. Success is green, cancellation and warnings are yellow, errors and blocking items are red, and plan headings and identifiers are cyan. The prompt layer uses Inquire with ASCII control labels and markers so legacy Windows code pages do not have to render decorative Unicode glyphs; `--no-color` and `NO_COLOR` preserve the same text without color sequences.

The OpenCode workflow asks for the provider protocol after the model ID and starts on the protocol inferred from that ID. OpenAI and Anthropic selections use their native OpenCode providers and skip the redundant display-name prompt because OpenCode manages those catalogs. The optional display-name prompt remains for custom OpenCode providers and OpenClaw.

Before `Choose a model`, the guided workflow probes protocol-standard Models endpoints in parallel:
root URLs use `/v1/models` for OpenAI and Anthropic and `/v1beta/models` for Google, while an
explicit path prefix is preserved and receives one `/models` segment. A short ASCII spinner remains
compatible with legacy Windows code pages; each successful remote result is shown first with a
green `AVAILABLE` marker. Transient transport/read failures, timeouts, HTTP 429, and HTTP 5xx are
retried per API up to three total attempts with short exponential backoff; authentication, routing,
redirect, response-limit, and schema failures are reported immediately. A failed protocol is a
warning rather than a workflow stop, and `--debug` includes its final attempt count. The explicit
endpoint stays first; after HTTP 404/405 on a known Anthropic-compatible routing suffix
such as `/api/claudecode`, discovery also tries the protocol-standard endpoint outside that suffix.
The first picker view stays compact with `Add all N available models`, discovered models,
`Show all catalog models`, and `Custom model ID`. Choosing `Add all` publishes every successfully
discovered model
to OpenCode, OpenClaw, and Hermes, then opens a second picker for the primary/default model; the
catalog and default are reviewed together before one confirmation. Choosing a single discovered or
catalog model keeps the existing one-model path. The reviewed local catalog is expanded only after
the explicit `Show all` choice, and duplicate IDs remain listed once. For OpenCode, Add all groups
the catalog by protocol under `rewire-oairesp`, `rewire-anthropic`, `rewire-google`, and
`rewire-oaicomp`; the selected default uses the matching `<provider>/<model>` reference, so Claude
does not accidentally run through `@ai-sdk/openai-compatible`. The local catalog covers GPT, Claude, Gemini, DeepSeek, Qwen, GLM, Kimi,
MiniMax, Grok, Mistral, MiMo, Nemotron, Doubao, and Cohere families. See [model catalog](docs/model-catalog.md)
for source versions, refresh time, selection boundary, and current entries.

Model prompts name only the selected clients that consume the choice. Claude Code and Codex alone
skip model discovery; mixed selections use labels such as `Choose a model for OpenCode` or `Choose
a model for Hermes and OpenClaw` so their preserved model settings are not implied to change.

Use `rewire configure --debug` to print credential-free discovery traces containing the API,
resolved URL, HTTP status, Content-Type, response byte count, and a sanitized redirect path. Debug
mode never prints authentication headers, tokens, or response bodies.

## Architecture

```text
CLI input (Clap) --> Command router --> Per-command modules
                                      |             |
                                      |       Shared CLI output
                                      v       (human / JSON / color)
                             Planner ---- Client recipes
                                      |
                    Format codecs (JSON5 / TOML / YAML / dotenv)
                                      |
                    Prepared changes + journal + verifier + rollback
```

The binary is split into `cli/input`, `cli/output`, and one module per command under `commands`; `main.rs` only bootstraps parsing, dispatch, and the unified runtime-error boundary. The core is split into `model`, `clients`, `diagnostics`, `format`, `planner`, `transaction`, `security`, and `verifier` modules. The `workflow` module owns prompt orchestration and pure, testable numbered-review decisions. No client adapter owns file locking or rollback policy. The transaction journal snapshots every target before the first replacement and restores all completed replacements when a later write or verification fails. Before and after snapshots are authenticated-encrypted and all journal artifacts are private to the current user. Rollback restores the full before image when the target still matches transaction output; after unrelated edits it reverses only adapter-owned fields, and after an owned-field edit it stops before writing any target.

Client path overrides are honored for the real selected Home: `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `OPENCODE_CONFIG`, `OPENCODE_CONFIG_DIR`, `XDG_CONFIG_HOME`, `HERMES_HOME`, `OPENCLAW_CONFIG_PATH`, and `OPENCLAW_STATE_DIR`. An explicit isolated `--home` remains self-contained unless that path is also the process Home. Paths redirected outside the selected Home are rejected by the planner.

## Verification

The repository pins Rust 1.94.0 in `rust-toolchain.toml`, uses edition 2024, and applies the same strict Clippy baseline as AsterForge: `pedantic` plus denied unreasoned lint suppressions, lossy casts, grouped unsafe operations, and undocumented unsafe blocks. Deny overrides live at crate roots so editor Cargo schemas do not need to understand lint-priority tables.

GitHub Actions runs the same formatting, strict Clippy, and full test gates with `Cargo.lock`, then cross-checks all targets for `x86_64-pc-windows-gnu` on the pinned toolchain.

Tags matching `vX.Y.Z` additionally build native archives for macOS arm64/amd64, Linux arm64/amd64, and Windows amd64. The release job emits SPDX JSON SBOMs and aggregate SHA-256 checksums, signs archives and checksums through Cosign's GitHub OIDC flow, creates GitHub build-provenance attestations, and publishes the complete immutable artifact set. `rewire --version` includes the package version, source commit, and build target.

Fixture regressions cover a minimal and a production-shaped configuration for every adapter. See `tests/fixtures/SOURCES.md` for official references and the sanitization boundary; no raw home-directory configuration or credential is committed.

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo check --target x86_64-pc-windows-gnu --all-targets
```
