# Changelog

All notable changes to Rewire are documented here.

## [Unreleased]

### Changed

- Updated direct Rust dependencies to the highest available releases, including Inquire 0.9, JSONC Parser 0.33, SHA-2 0.11, TOML Edit 0.25, and URL 2.5; refreshed `Cargo.lock`, verified the root dependency set with `cargo outdated`, and retained behavior through the full test matrix.
- Adopted Rust edition 2024, pinned the Rust 1.94.0 minimal toolchain with Rustfmt and Clippy, and enabled the AsterForge strict lint baseline for pedantic diagnostics, justified suppressions, numeric casts, and unsafe-code hygiene.
- Split the binary into dedicated CLI input/output layers, diagnostics, and per-command modules so doctor, plan, apply, remove, rollback, and backup share one rendering and error policy.
- Made `rollback`'s transaction ID optional for terminal use: the command confirms the latest committed, still-available transaction, supports `--yes` for automation, rejects ambiguous non-TTY/JSON invocations without an ID, and records completed rollbacks so they are not selected again.
- Preserved chained rollback diagnostics in human and JSON errors so three-way conflicts identify the adapter-owned field that changed after the transaction.
- Aligned adapter recipes with current client configuration contracts: Claude gateway bearer tokens; an isolated Codex Responses provider/profile that preserves model selection; OpenCode's preferred `opencode.jsonc`, native OpenAI/Anthropic providers with managed model catalogs, custom-provider SDK fallback, restricted file substitution, and qualified selected model; Hermes named-provider fields, structured model selection, and `.env`; and OpenClaw's `models.providers` nesting, selected primary model, and file SecretRef.
- Made client config discovery honor Claude, Codex, OpenCode/XDG, Hermes, and OpenClaw location environment variables while preserving isolated `--home` boundaries.
- Separated OpenCode model IDs, catalog display names, and provider protocol selection; added family inference, native OpenAI/Anthropic catalog reuse, model-aware workflow preselection, explicit CLI/workflow overrides, credential-scoped native-provider removal, and a dedicated OpenCode adapter module.
- Added best-effort parallel Models endpoint discovery for OpenAI, Anthropic, and Google with protocol-standard root paths, explicit-prefix preservation, protocol-specific authentication, bounded responses, timeout isolation, source-aware deduplication, an ASCII-compatible spinner, colored `AVAILABLE` entries, an opt-in expansion of the local catalog, and credential-free `--debug` traces.
- Added `Add all N available models` to the guided picker for OpenCode, OpenClaw, and Hermes. The workflow writes the complete deduplicated discovery catalog, asks separately for the primary/default model, and keeps the existing single-model path unchanged.
- Split OpenCode Add all catalogs into `rewire-oairesp`, `rewire-anthropic`, `rewire-google`, and `rewire-oaicomp` providers so every model uses its protocol-specific AI SDK package; added ownership-scoped migration and reconciliation for the legacy mixed `provider.rewire` layout.
- Scoped model prompts to the selected model-aware clients and added bounded per-API retries for transient transport/read failures, timeouts, HTTP 429, and HTTP 5xx, with exponential backoff and credential-free attempt counts in debug diagnostics.

### Added

- Vendor-neutral `rewire` CLI with `--baseurl`, token input modes, client selection, client-aware required `--model` input, human-readable command output, opt-in stable JSON, environment/version-aware doctor, backup listing, transactional remove and rollback commands, generated shell completions, detailed build version output, and TTY-aware semantic colors for help, plans, statuses, and errors.
- Inquire-based guided terminal workflow with detected-client multi-select, validated endpoint and ASCII-masked token prompts with visible cursor progress, numbered modification and conflict review, single-choice confirmation, edit/rebuild/cancel paths, semantic status colors, ASCII-safe control labels for legacy Windows code pages, monochrome mode, and a compatibility alias for the former `tui` command spelling.
- Structured recipes for Claude Code, Codex, OpenCode, Hermes Agent, and OpenClaw with JSON/JSONC/JSON5, TOML, and YAML support.
- Multi-target before/after snapshots, SHA-256 integrity checks, exclusive locking, authenticated-encrypted private backups, atomic replacement, secret redaction, symlink guards, and field-level three-way rollback that preserves later unrelated edits and blocks owned-field conflicts before writing.
- Post-write syntax verification before a transaction is marked committed.
- Prepared before/after bytes, TOCTOU detection, mode preservation, journal-first writes, and failure recovery for multi-client transactions.
- CST-based JSONC/JSON5 edits that retain comments and source formatting instead of moving comments to a synthetic header.
- Curated local model catalog with provider-native IDs, display names, SDK families, source metadata, a capability-informed shortlist of current flagship and coding models, and a custom-model escape hatch for newly released IDs.
- Ten sanitized, source-documented fixture variants covering minimal and production-shaped configurations for all five client adapters, including local Claude/Codex topology and official provider examples.
- Unit and temporary-Home integration tests covering malformed input, JSON5 comments, dotenv special characters, unknown-field preservation, plaintext backup scans, private permissions, multi-file failure recovery, symlink rejection, rollback behavior, workflow review decisions, semantic output colors, and every subcommand's human/JSON contract.
- Provider and selected-model conflict review that treats matching values as idempotent and requires guided confirmation or `--yes` before replacing an existing endpoint or current model; model removal preserves later selections that no longer point to Rewire.
- GitHub Actions quality gates for formatting, strict Clippy, the full test suite, and Windows GNU cross-compilation, plus tag releases for five native targets with checksums, Cosign OIDC signatures, SPDX SBOMs, and provenance attestations.

### Security

- Tokens use zeroizing core and secure workflow password input, remain omitted from serialized plans/manifests, and are redacted from generated diffs, errors, and debug output.

[Unreleased]: https://github.com/AptS-1547/rewire/commits/master
