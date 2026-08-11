# Client compatibility

Rewire treats gateway configuration and credential persistence as one transaction. The main
configuration file, any client-native environment file, and any restricted token file are planned,
reviewed, written, verified, and rolled back together.

## Supported adapters

| Client | Main configuration | Credential strategy | Model behavior |
| --- | --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json` | `env.ANTHROPIC_AUTH_TOKEN` in the client settings | Existing model selection is preserved |
| Codex | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | Isolated provider `experimental_bearer_token`; existing `auth.json` or keyring login is untouched | Adds `profiles.rewire` without setting a model in that profile or globally |
| OpenCode | `$OPENCODE_CONFIG`, `$OPENCODE_CONFIG_DIR`, XDG config, or the native global-file preference | `{file:...}` reference to a private Rewire token file | Requires `--model`; a single OpenAI/Anthropic model reuses its native provider, while Add all splits the discovered catalog into four Rewire-managed protocol providers |
| Hermes Agent | `$HERMES_HOME/config.yaml` or `~/.hermes/config.yaml` | `REWIRE_TOKEN` in the matching `.env` through `key_env` | Requires `--model`; writes `{ provider: rewire, name: <id> }` and provider `default_model`; Add all also writes every discovered ID to the provider catalog |
| OpenClaw | `$OPENCLAW_CONFIG_PATH` or `$OPENCLAW_STATE_DIR/openclaw.json` | `file` SecretRef backed by the selected state directory | Requires `--model`; catalogs raw `<id>` and selects `rewire/<id>` as `agents.defaults.model.primary`; Add all writes the complete catalog |

Secret-bearing targets are written with mode `0600` on Unix. Transaction directories use `0700`,
their files use `0600`, and before snapshots are authenticated-encrypted so a previous token is not
left as searchable plaintext. The transaction manifest contains the random backup key for later
rollback but is not part of CLI or JSON output.

The backup key is stored beside the ciphertext so authenticated encryption is defense against
accidental plaintext exposure and secret scanning, not a replacement for filesystem access control.
The private directory and file permissions are the confidentiality boundary.

`rewire remove --client LIST` reverses these adapter-owned fields through the same plan and
transaction engine. It preserves unrelated configuration and dotenv entries, deletes only the
dedicated OpenCode/OpenClaw token files, and can itself be rolled back. A selected-model field is
removed only while it still points to a Rewire-owned provider configuration; a later operator switch
or an official provider with an independently managed credential is kept.

The CLI accepts the provider-native model ID, not a client-qualified reference. For OpenCode,
`--model gpt-5.5 --sdk openai` becomes `openai/gpt-5.5`, and a Claude model becomes
`anthropic/<id>`. A compatible `coder-model` remains `rewire/coder-model`; OpenClaw also selects
`rewire/<id>`, while Hermes stores the raw ID under its structured Rewire provider. A leading
`rewire/` is rejected to prevent double qualification. Replacing an existing current/default model
is a nonblocking review item and therefore requires guided confirmation or explicit `--yes`.

OpenCode supports overriding `options.baseURL` for any provider and uses Models.dev for its built-in
model catalog. For one selected model, Rewire writes only `provider.openai.options` or
`provider.anthropic.options` plus the qualified selection for those native protocols; Google and
compatible models use the custom `provider.rewire` entry. Unknown IDs default to
`@ai-sdk/openai-compatible`.

An Add all catalog is different: one OpenCode provider has one effective AI SDK package, so a mixed
catalog cannot safely live under one `provider.rewire`. Rewire partitions it as follows:

| Provider ID | AI SDK package | Model families |
| --- | --- | --- |
| `rewire-oairesp` | `@ai-sdk/openai` | GPT, o-series, and Codex IDs |
| `rewire-anthropic` | `@ai-sdk/anthropic` | Claude IDs |
| `rewire-google` | `@ai-sdk/google` | Gemini IDs |
| `rewire-oaicomp` | `@ai-sdk/openai-compatible` | Unknown and compatible API families |

The default model is qualified with its partition, for example
`rewire-anthropic/claude-sonnet-5`. Each catalog entry keeps its own inferred or preset SDK family;
the default model no longer assigns one SDK to the entire catalog. A previous Rewire-owned mixed
`provider.rewire` is migrated during the next Add all apply only when its `options.apiKey` still
points at Rewire's private token file. Reconciliation replaces only managed model maps, removes
obsolete Rewire-owned partitions, and preserves unrelated providers and extra operator options.
OpenCode 1.18.16 exposes `opencode models [provider] --refresh` for inspecting provider catalogs.

The guided workflow's model picker performs a separate best-effort capability probe. A root URL
uses `/v1/models` for OpenAI and Anthropic and `/v1beta/models` for Google; an explicitly supplied
path prefix is preserved and extended with `models`. It sends `Authorization: Bearer` to the OpenAI shape,
`x-api-key` plus `anthropic-version` to the Anthropic shape, and `x-goog-api-key` to the Google
shape. Results are deduplicated by provider-native ID and retain all successful protocol
sources, while HTTP, timeout, malformed-response, and size-limit failures become colored warnings.
`configure --debug` adds credential-free URL, status, Content-Type, byte-count, and sanitized
redirect-path diagnostics plus the final attempt count without printing authentication headers,
tokens, or response bodies. Transient request/read failures, timeouts, HTTP 429, and HTTP 5xx retry
per API up to three total attempts; permanent HTTP, redirect, limit, and schema failures stop after
the first attempt.
Choosing `Add all` makes the successful, deduplicated discovery result the provider catalog and
opens a second single-select prompt for the primary/default model. The catalog is emitted only for
OpenCode, OpenClaw, and Hermes; the single-model path remains unchanged. OpenCode uses the four
protocol partitions above, while Hermes and OpenClaw retain their single Rewire provider formats.

## Evidence boundary

Primary schema and behavior references:

- Claude Code settings: <https://code.claude.com/docs/en/settings>
- Codex configuration and authentication:
  <https://developers.openai.com/codex/config-reference/> and
  <https://developers.openai.com/codex/auth/>
- OpenCode configuration and providers: <https://opencode.ai/docs/config/> and
  <https://opencode.ai/docs/providers/>
- OpenCode managed model catalog: <https://models.dev/>
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
1.18.16 on macOS reports `~/.config/opencode` and its current global-file selection prefers
`opencode.jsonc`, then `opencode.json`, then legacy `config.json`. Rewire follows that observed and
source-verified order. Current OpenCode builds are also migrating credential persistence from the
documented `auth.json` shape toward an internal credential database, so Rewire uses the officially
supported `{file:...}` substitution instead of writing a version-sensitive auth store.

Native-provider removal is credential-scoped. Rewire removes `openai` or `anthropic` endpoint and
key options, and their selected model, only while the provider's `apiKey` still points to Rewire's
dedicated token file. Other provider options and operator-owned native credentials remain intact.

## Investigated clients outside the first release

The secondary guide also documents Gemini CLI (`~/.gemini/.env` plus `settings.json`) and Droid CLI
(`~/.factory/config.json` custom models). They remain candidates rather than silently expanding the
first-release CLI enum. Adding either client requires its own detection, conflict, rollback,
credential, model-catalog, fixture, and platform validation matrix.
