# Fixture provenance

These fixtures are sanitized, hand-authored configuration shapes. They are not raw copies of a
developer home directory, and every credential, account identifier, host, command path, and project
path is synthetic.

The `machine-shaped` Claude and Codex variants retain only the key/table topology observed on a
development machine on 2026-08-09. Values were replaced before the fixtures were created.

Official configuration references used to build and review the fixture shapes:

- Claude Code settings: <https://code.claude.com/docs/en/settings>
- Codex config reference: <https://developers.openai.com/codex/config-reference/>
- OpenCode config and providers: <https://opencode.ai/docs/config/> and
  <https://opencode.ai/docs/providers/>
- Hermes Agent configuration and providers:
  <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/configuration.md>
  and
  <https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md>
- OpenClaw configuration reference and examples:
  <https://docs.openclaw.ai/gateway/configuration-reference> and
  <https://docs.openclaw.ai/gateway/configuration-examples>

Official examples are used as schema evidence, not copied as golden files. Each fixture includes
unrelated operator-owned fields so the regression suite proves structured preservation as well as
adapter insertion.

The cross-client guide at <https://cc.autobits.cc/zh-CN/usage-doc> is retained as a secondary index
for config-file and environment-variable setup patterns. Adapter fields and paths are accepted only
after confirmation from the primary references above or a current installed client. The resulting
evidence matrix and known documentation drift are recorded in `docs/client-compatibility.md`.

OpenCode 1.18.15 reports `/Users/esap/.config/opencode` as its global config directory. Its current
loader prefers `opencode.jsonc`, then `opencode.json`, then the legacy `config.json` when choosing a
global file to patch; the fixtures therefore exercise the preferred `opencode.jsonc` target.
