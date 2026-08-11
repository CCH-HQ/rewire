# Client compatibility

Rewire treats gateway configuration and credential persistence as one transaction. The main
configuration file, any client-native environment file, and any restricted token file are planned,
reviewed, written, verified, and rolled back together.

## Supported adapters

| Client | Main configuration | Credential strategy | Model behavior |
| --- | --- | --- | --- |
| Claude Code | `$CLAUDE_CONFIG_DIR/settings.json` or `~/.claude/settings.json` | `env.ANTHROPIC_AUTH_TOKEN` in the client settings | Existing model selection is preserved |
| Codex | `$CODEX_HOME/config.toml` or `~/.codex/config.toml` | Isolated provider `experimental_bearer_token`; existing `auth.json` or keyring login is untouched | Adds `profiles.rewire` without setting a model in that profile or globally; a bare gateway origin becomes the Responses `/v1` base |
| OpenCode | `$OPENCODE_CONFIG`, `$OPENCODE_CONFIG_DIR`, XDG config, or the native global-file preference | `{file:...}` reference to a private Rewire token file | Requires `--model`; a single OpenAI/Anthropic model reuses its native provider, while Add all splits the discovered catalog into four Rewire-managed protocol providers with protocol-specific request roots |
| Hermes Agent | `$HERMES_HOME/config.yaml` or the platform-native Hermes directory | `REWIRE_TOKEN` in the matching `.env` through `key_env` | Requires `--model`; writes `model.default`, `model.provider`, and a keyed `providers.rewire` entry; Add all writes a model dictionary under that provider |
| OpenClaw | `$OPENCLAW_CONFIG_PATH` or `$OPENCLAW_STATE_DIR/openclaw.json` | `file` SecretRef backed by the selected state directory | Requires `--model`; catalogs raw `<id>` and selects `rewire/<id>` as `agents.defaults.model.primary`; every catalog entry retains its own transport and request root |

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

Client request bases are derived independently from the discovery URL. When the supplied URL is a
bare origin, Rewire writes the version root expected by the receiving SDK or transport:

| Adapter route | Stored request base |
| --- | --- |
| Claude Code Anthropic gateway | Original origin; Claude appends `/v1/messages` |
| Codex Responses | `<origin>/v1` |
| OpenCode OpenAI, Anthropic, or OpenAI-compatible SDK | `<origin>/v1` |
| OpenCode Google SDK | `<origin>/v1beta` |
| OpenClaw OpenAI Responses or Completions model | `<origin>/v1` |
| OpenClaw Anthropic Messages model | Original origin |
| OpenClaw Google Generative AI model | `<origin>/v1beta` |

An explicit path such as `/api/anthropic`, `/coding`, or an already versioned endpoint is treated
as a complete operator-owned routing prefix and is not rewritten. This distinction lets a single
gateway origin drive standard protocol routes without corrupting gateways that publish custom
prefixes.

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
the first attempt. Explicit paths remain the first candidate. After HTTP 404/405 on a known
Anthropic-compatible request prefix such as `/api/claudecode`, `/apps/anthropic`, or `/coding`,
Rewire strips that suffix and tries the protocol-standard Models endpoint while preserving any
earlier path prefix. Other explicit paths are never rewritten speculatively.
Choosing `Add all` makes the successful, deduplicated discovery result the provider catalog and
opens a second single-select prompt for the primary/default model. The catalog is emitted only for
OpenCode, OpenClaw, and Hermes; the single-model path remains unchanged. OpenCode uses the four
protocol partitions above. Hermes retains one Rewire provider. OpenClaw also retains one provider,
but each model entry overrides `api` and `baseUrl`: OpenAI catalog models use `openai-responses`,
Claude uses `anthropic-messages`, Gemini uses `google-generative-ai`, and compatible/unknown models
use `openai-completions`.

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
  <https://github.com/NousResearch/hermes-agent/blob/main/skills/autonomous-ai-agents/hermes-agent/references/configuration.md>
  and
  <https://github.com/NousResearch/hermes-agent/blob/main/skills/autonomous-ai-agents/hermes-agent/references/providers-and-models.md>
- OpenClaw model providers, configuration, and secrets:
  <https://docs.openclaw.ai/concepts/model-providers>,
  <https://docs.openclaw.ai/gateway/configuration-reference>, and
  <https://docs.openclaw.ai/gateway/secrets>

The Autobits usage guide at <https://cc.autobits.cc/zh-CN/usage-doc> is used as a secondary
compatibility index. It usefully places config-file and environment-variable workflows side by
side for Claude Code, Codex CLI, Gemini CLI, OpenCode, and Droid CLI. Its snippets are not copied
into adapters until the current client documentation or implementation confirms the path and
field contract.

CC Switch `farion1231/cc-switch` revision `8673e9d8d8508b89c48056523c5f86e7916b4c3c` was inspected
on 2026-08-11 as a cross-client implementation reference. Its OpenCode provider forms and runtime
model enumeration corroborate the SDK/model separation. Its Hermes adapter reads both the legacy
`custom_providers` list and the v12+ keyed `providers` map, while keeping dict-only entries outside
CC Switch's own edit path. Its Codex universal-provider conversion also confirms that a bare origin
needs `/v1`, but its live switching path projects credentials through `auth.json` or provider TOML.
Rewire instead keeps `auth.json` untouched and uses a dedicated profile/provider, preventing a
compatible gateway from replacing an existing ChatGPT login. Rewire keeps its own transaction,
credential, JSONC, three-protocol discovery, and client-path policies; CC Switch presets and local
proxy behavior are not treated as upstream protocol authority.

The client adapters were source-checked on 2026-08-11 against the following concrete revisions:

- Claude Code `v2.1.186` revision `12281998d8c85813c4b5952ed9367784aae37d31`. The current
  environment reference states that `ANTHROPIC_AUTH_TOKEN` supplies a Bearer authorization value,
  while `ANTHROPIC_API_KEY` supplies `X-Api-Key`; Rewire therefore uses the bearer field for a
  compatible gateway and reports a simultaneously set API-key credential as a precedence warning.
- Codex `rust-v0.147.0` revision `be6e8eac029b183056b7e4402879f15d2c85f61b`. Its generated
  schema retains `wire_api = "responses"`, `requires_openai_auth`, provider profiles, and
  `experimental_bearer_token`; the latter is discouraged for manually authored secrets but remains
  the documented programmatic credential escape hatch. The Rewire output was also parsed by the
  locally installed Codex CLI 0.147.0 under an isolated `CODEX_HOME`.
- OpenCode `v1.18.16` revision `a3647eb025c7615159d417dcc49fc39fdaeba65b`. Its loader merges
  `config.json`, `opencode.json`, then `opencode.jsonc`, and its AI SDK routes use `/v1` for OpenAI
  and Anthropic plus `/v1beta` for Google.
- OpenClaw revision `f0d6cc4adeeeed7319d3d947f2d7690e7c40ce24`. Its strict schema accepts
  `file` SecretRefs with `singleValue`, requires custom providers to declare `baseUrl` and a model
  list, and permits model-level `api` and `baseUrl` overrides. Rewire uses those model-level fields
  rather than forcing a mixed catalog through one provider-level API.

Hermes Agent revision `c0106e50e7ecedb3ce34e785d949725dc4e0e457` was inspected on 2026-08-11.
Its current resolver reads keyed provider `api`, `key_env`, and `transport` fields directly, and its
own legacy-to-v12 conversion emits `default_model` plus a model dictionary. Rewire therefore writes
`model.{default,provider,base_url}` and
`providers.rewire.{api,key_env,transport,default_model,models}`. Reconfigure migrates the accepted
`base_url`, `api_mode`, `model`, model-list, and `model.name` aliases only when the provider still
uses `REWIRE_TOKEN`; provider extensions and unrelated model fields remain intact. Removal is
field-scoped and preserves operator-owned model settings such as `context_length`.

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
