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

## Quick start

```bash
cargo run -- --baseurl https://api.example.com --token-stdin --client claude --yes
printf '%s\n' "$REWIRE_TOKEN" | rewire --baseurl https://api.example.com --token-stdin --client codex --yes
rewire --baseurl https://api.example.com --token TOKEN --client claude,opencode --dry-run --json
rewire --baseurl https://api.example.com --token-stdin --client opencode,openclaw --model coder-model --yes
rewire --baseurl https://api.example.com --token-stdin --client claude,codex plan
rewire doctor
rewire doctor --json
rewire backup list --json
rewire rollback TX_ID --json
rewire remove --client opencode,openclaw --yes
rewire completions zsh > ~/.zfunc/_rewire
rewire configure --no-color
```

`--token` is convenient but can be visible in shell history and process listings. Prefer `--token-stdin`, `REWIRE_TOKEN`, or the guided workflow (`rewire configure`). Complete tokens are excluded from plans, manifests, diagnostics, and the custom `Secret` debug representation.

`--model` is an optional hint. OpenCode and OpenClaw keep an empty custom-provider catalog when it is omitted and report the follow-up in the plan; they do not switch the global/default model. Codex scopes the hint to `profiles.rewire`, and Hermes scopes it to the added provider. See [client compatibility](docs/client-compatibility.md) for credential locations, source evidence, and known upstream documentation drift.

Running `rewire` without arguments opens the workflow only when both standard input and standard output are terminals. Pipes and automation fail fast with the missing CLI input instead of waiting for prompts; `--non-interactive` makes that intent explicit. The previous `rewire tui` spelling remains an alias for `rewire configure`.

Doctor, plan, apply, remove, rollback, and backup commands render concise operator-facing text by default. Doctor reports all supported clients, selected config paths, local executable versions, and relevant environment variable names while hiding their values. Plans use numbered semantic markers for created, updated, deleted, unchanged, review-required, and blocked targets; headings and identifiers are cyan, successful states are green, warnings are yellow, and failures are red. `--json` switches successful results and runtime errors to stable machine-readable JSON. Color is emitted only to a terminal; `--no-color` and `NO_COLOR` also apply to Clap's early help and validation output.

An existing `rewire` provider at the same normalized URL is idempotent. A different URL remains a prepared modification but is also numbered `[REVIEW]`; the guided workflow's final selection or an explicit `--yes` accepts that replacement. Syntax errors, read-only files, symlinks, paths outside the selected Home, and concurrent edits remain blocking. `remove --client ...` uses the same review and transaction path, removes only adapter-owned fields, and deletes only dedicated secret files.

The workflow first presents a multi-select list for one or more detected clients, then asks for the compatible base URL, an ASCII-masked token, and an optional model hint. The `*` mask advances with the cursor so secret input has visible progress without exposing token characters. It generates a numbered list of file modifications and conflicts before presenting a single final choice: apply the numbered plan, return to edit the inputs, or cancel. Malformed, read-only, and symlinked targets are numbered as blocking items; while any are present, the apply choice is omitted. If a file changes after review, the workflow offers to rebuild the plan, edit the inputs, or stop. Success is green, cancellation and warnings are yellow, errors and blocking items are red, and plan headings and identifiers are cyan. The prompt layer uses Inquire with ASCII control labels and markers so legacy Windows code pages do not have to render decorative Unicode glyphs; `--no-color` and `NO_COLOR` preserve the same text without color sequences.

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
