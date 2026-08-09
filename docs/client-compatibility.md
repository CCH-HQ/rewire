# Client compatibility

Rewire treats gateway configuration and credential persistence as one transaction. The main
configuration file, any client-native environment file, and any restricted token file are planned,
reviewed, written, verified, and rolled back together.

## Supported adapters

| Client | Main configuration | Credential strategy | Model behavior |
| --- | --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json` | `env.ANTHROPIC_AUTH_TOKEN` in the client settings | Existing model selection is preserved |
| Codex | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | Isolated provider `experimental_bearer_token`; existing `auth.json` or keyring login is untouched | Adds `profiles.rewire`; `--model` is scoped to that profile |
| OpenCode | `$OPENCODE_CONFIG`, `$OPENCODE_CONFIG_DIR`, XDG config, or the native global-file preference | `{file:...}` reference to a private Rewire token file | Provider only by default; `--model` adds one catalog entry |
| Hermes Agent | `$HERMES_HOME/config.yaml` or `~/.hermes/config.yaml` | `REWIRE_TOKEN` in the matching `.env` through `key_env` | Provider only by default; `--model` sets its `default_model` |
| OpenClaw | `$OPENCLAW_CONFIG_PATH` or `$OPENCLAW_STATE_DIR/openclaw.json` | `file` SecretRef backed by the selected state directory | Provider only by default; `--model` adds one required catalog entry |

Secret-bearing targets are written with mode `0600` on Unix. Transaction directories use `0700`,
their files use `0600`, and before snapshots are authenticated-encrypted so a previous token is not
left as searchable plaintext. The transaction manifest contains the random backup key for later
rollback but is not part of CLI or JSON output.

The backup key is stored beside the ciphertext so authenticated encryption is defense against
accidental plaintext exposure and secret scanning, not a replacement for filesystem access control.
The private directory and file permissions are the confidentiality boundary.

`rewire remove --client LIST` reverses these adapter-owned fields through the same plan and
transaction engine. It preserves unrelated configuration and dotenv entries, deletes only the
dedicated OpenCode/OpenClaw token files, and can itself be rolled back.

## Evidence boundary

Primary schema and behavior references:

- Claude Code settings: <https://code.claude.com/docs/en/settings>
- Codex configuration and authentication:
  <https://developers.openai.com/codex/config-reference/> and
  <https://developers.openai.com/codex/auth/>
- OpenCode configuration and providers: <https://opencode.ai/docs/config/> and
  <https://opencode.ai/docs/providers/>
- Hermes Agent configuration and providers:
  <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/configuration.md>
  and
  <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md>
- OpenClaw model providers, configuration, and secrets:
  <https://docs.openclaw.ai/concepts/model-providers>,
  <https://docs.openclaw.ai/gateway/configuration-reference>, and
  <https://docs.openclaw.ai/gateway/secrets>

The Autobits usage guide at <https://cc.autobits.cc/zh-CN/usage-doc> is used as a secondary
compatibility index. It usefully places config-file and environment-variable workflows side by
side for Claude Code, Codex CLI, Gemini CLI, OpenCode, and Droid CLI. Its snippets are not copied
into adapters until the current client documentation or implementation confirms the path and
field contract.

`rewire doctor` reports relevant environment variable names for every supported client while
always hiding their values. Planning Claude Code also warns when process-level
`ANTHROPIC_BASE_URL` or `ANTHROPIC_AUTH_TOKEN` differs from the requested configuration, and when
`ANTHROPIC_API_KEY` introduces a second authentication source. These warnings describe precedence
risk without copying credentials or endpoint values into diagnostics.

Location overrides are honored only when configuring the process's effective Home. An explicit
fixture or automation `--home` stays isolated from the operator's ambient client-location
variables. Every selected target still has to remain under that Home and pass the symlink and
read-only checks.

One concrete drift example is OpenCode: the secondary guide names `opencode.json`, while OpenCode
1.18.15 on macOS reports `~/.config/opencode` and its current global-file selection prefers
`opencode.jsonc`, then `opencode.json`, then legacy `config.json`. Rewire follows that observed and
source-verified order. Current OpenCode builds are also migrating credential persistence from the
documented `auth.json` shape toward an internal credential database, so Rewire uses the officially
supported `{file:...}` substitution instead of writing a version-sensitive auth store.

## Investigated clients outside the first release

The secondary guide also documents Gemini CLI (`~/.gemini/.env` plus `settings.json`) and Droid CLI
(`~/.factory/config.json` custom models). They remain candidates rather than silently expanding the
first-release CLI enum. Adding either client requires its own detection, conflict, rollback,
credential, model-catalog, fixture, and platform validation matrix.
