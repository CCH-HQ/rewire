# Changelog

All notable changes to Rewire are documented here.

## [Unreleased]

## [0.0.1] - 2026-08-11

### Added

- Vendor-neutral configuration for Claude Code, Codex CLI, OpenCode, Hermes Agent, and OpenClaw, with environment-aware config discovery and isolated `--home` support.
- Guided terminal workflow for client multi-selection, API endpoint and masked-token input, live model discovery, catalog filtering, numbered change review, and one final confirmation before writing.
- Human-readable output by default, opt-in stable JSON, semantic terminal colors, shell completions, client/version diagnostics, backup inspection, transactional removal, and latest-transaction rollback.
- Protocol-aware model configuration for OpenAI Responses, Anthropic Messages, Google Generative AI, and OpenAI-compatible APIs, including curated local models and best-effort live discovery with bounded retries.
- Structured, comment-preserving updates for JSON, JSONC, JSON5, TOML, YAML, and dotenv files while retaining unrelated operator configuration.
- Multi-file transactions with prepared snapshots, atomic replacement, post-write syntax verification, encrypted backups, field-level three-way rollback, and conflict detection for later edits.
- Checksum-verified Unix and PowerShell bootstrap entrypoints for persistent installation or a single temporary run, supporting GitHub releases, mirrors, exact download URLs, explicit checksums, custom install directories, cleanup after non-zero exits, and argument forwarding for embedded distribution.
- CI and release automation for macOS arm64/x86-64, Linux arm64/x86-64, and Windows x86-64, including checksums, SPDX SBOMs, Cosign OIDC signatures, and GitHub build provenance.
- Credential-isolated Docker E2E coverage that installs pinned official clients and verifies real model calls through every supported adapter.

### Fixed

- Migrated Codex configuration to the current profile-v2 `rewire.config.toml` layout while cleaning legacy Rewire profile/provider tables and preserving operator-owned base configuration.
- Selected the Hermes wire transport from the chosen model family so Claude, OpenAI, and compatible models use the endpoint shape their client runtime expects.
- Preserved actionable field-level diagnostics when rollback detects an adapter-owned value changed after commit.
- Installed the MinGW toolchain required for the Windows GNU CI target.

### Security

- Tokens are zeroized in memory, omitted from plans and transaction manifests, redacted from diagnostics and diffs, and accepted through protected terminal or standard-input paths.
- Secret-bearing files and transaction state use private permissions; backups are authenticated-encrypted and plaintext credential leakage is checked in integration and live-client E2E tests.
- Configuration writes reject unsafe paths, symlinks, read-only targets, and time-of-check/time-of-use changes before committing a transaction.

[Unreleased]: https://github.com/CCH-HQ/rewire/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/CCH-HQ/rewire/releases/tag/v0.0.1
