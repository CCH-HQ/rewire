# Fixture provenance

These fixtures are sanitized, hand-authored configuration shapes. They are not raw copies of a
developer home directory, and every credential, account identifier, host, command path, and project
path is synthetic.

The `machine-shaped` Claude and Codex variants retain only the key/table topology observed on a
development machine on 2026-08-09. Values were replaced before the fixtures were created.

Official configuration references used to build and review the fixture shapes:

- Claude Code settings and environment variables: <https://code.claude.com/docs/en/settings> and
  <https://code.claude.com/docs/en/env-vars>; bearer/API-key header semantics were checked against
  the page updated on 2026-08-10 and Claude Code `v2.1.186` revision
  `12281998d8c85813c4b5952ed9367784aae37d31`
- Codex config reference: <https://developers.openai.com/codex/config-reference/>; provider/profile
  fields were cross-checked against Codex `rust-v0.147.0` revision
  `be6e8eac029b183056b7e4402879f15d2c85f61b`
- OpenCode config and providers: <https://opencode.ai/docs/config/> and
  <https://opencode.ai/docs/providers/>; loader order and AI SDK endpoint roots were cross-checked
  against OpenCode `v1.18.16` revision `a3647eb025c7615159d417dcc49fc39fdaeba65b`
- Hermes Agent configuration and providers:
  <https://github.com/NousResearch/hermes-agent/blob/main/skills/autonomous-ai-agents/hermes-agent/references/configuration.md>
  and
  <https://github.com/NousResearch/hermes-agent/blob/main/skills/autonomous-ai-agents/hermes-agent/references/providers-and-models.md>;
  keyed-provider normalization and resolution are cross-checked at Hermes Agent revision
  `c0106e50e7ecedb3ce34e785d949725dc4e0e457` against `hermes_cli/config.py` and
  `hermes_cli/providers.py`
- OpenClaw configuration reference and examples:
  <https://docs.openclaw.ai/gateway/configuration-reference> and
  <https://docs.openclaw.ai/gateway/configuration-examples>; file SecretRef and per-model
  transport/base URL fields were cross-checked against revision
  `f0d6cc4adeeeed7319d3d947f2d7690e7c40ce24`

Official examples are used as schema evidence, not copied as golden files. Each fixture includes
unrelated operator-owned fields so the regression suite proves structured preservation as well as
adapter insertion.

The cross-client guide at <https://cc.autobits.cc/zh-CN/usage-doc> is retained as a secondary index
for config-file and environment-variable setup patterns. Adapter fields and paths are accepted only
after confirmation from the primary references above or a current installed client. The resulting
evidence matrix and known documentation drift are recorded in `docs/client-compatibility.md`.

OpenCode 1.18.16 reports `/Users/esap/.config/opencode` as its global config directory. Its current
loader prefers `opencode.jsonc`, then `opencode.json`, then the legacy `config.json` when choosing a
global file to patch; the fixtures therefore exercise the preferred `opencode.jsonc` target.
Its schema and provider loader keep the catalog key, API model ID, display name, and npm SDK
package separate for custom providers. The same official documentation confirms that any built-in
provider can override `options.baseURL`; OpenAI and Anthropic then keep their Models.dev-backed
catalogs without a hand-authored `models` map. Adapter integration tests cover both shapes.

CC Switch revision `8673e9d8d8508b89c48056523c5f86e7916b4c3c` was inspected on 2026-08-11 as
secondary cross-client implementation evidence. Its adapter code is not copied into fixtures;
current client repositories remain authoritative for on-disk schemas.
