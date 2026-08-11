# Model Catalog

Rewire ships a small, reviewable local catalog for the guided workflow. Entries contain the
provider-native model ID, a display label, the provider family, and the OpenCode AI SDK package
family. The catalog is a convenience for selection, not a claim that every configured gateway
has access to every model.

## Sources

The current snapshot was inspected on 2026-08-10:

- `/Users/esap/Downloads/models.official.json`: `cchp.pricing-table/v1`, version
  `848cc4b00d694a9b`, refreshed `2026-08-09T16:51:04.746Z`.
- [AutoBits usage documentation](https://cc.autobits.cc/zh-CN/usage-doc): confirms the gateway's
  common `gpt-5.5`, Claude 4.5, and Gemini 2.5/3 model IDs and its `provider_id/model_id` format.
- [Kimi API documentation](https://platform.kimi.ai/docs/models): confirms the current Kimi K3,
  K2.7 Code, and K2.6 families and the exact `kimi-k3` API model ID.
- [Artificial Analysis](https://artificialanalysis.ai/): used as a current popularity and capability
  signal for the curated shortlist, including Kimi K3, Qwen3.8 Max, Grok 4.5, MiniMax-M3,
  MiMo-V2.5-Pro, and Nemotron 3 Ultra. Its benchmark labels are not treated as API model IDs.

The downloaded pricing snapshots are deliberately not copied into the repository. A future
catalog update should re-check the source version and refresh date, then update the static entries
and this document in the same change.

## Presets

| Provider | Model ID | Display name | SDK family |
| --- | --- | --- | --- |
| OpenAI | `gpt-5.5` | GPT-5.5 | `@ai-sdk/openai` |
| OpenAI | `gpt-5.6` | GPT-5.6 | `@ai-sdk/openai` |
| OpenAI | `gpt-5.3-codex` | GPT-5.3 Codex | `@ai-sdk/openai` |
| OpenAI | `gpt-5.3-codex-spark` | GPT-5.3 Codex Spark | `@ai-sdk/openai` |
| OpenAI | `gpt-5.5-pro-2026-04-23` | GPT-5.5 Pro | `@ai-sdk/openai` |
| OpenAI | `gpt-5.2-codex` | GPT-5.2 Codex | `@ai-sdk/openai` |
| OpenAI | `gpt-5.1-codex-max` | GPT-5.1 Codex Max | `@ai-sdk/openai` |
| OpenAI | `gpt-oss-120b` | GPT OSS 120B | `@ai-sdk/openai` |
| Anthropic | `claude-opus-5` | Claude Opus 5 | `@ai-sdk/anthropic` |
| Anthropic | `claude-sonnet-5` | Claude Sonnet 5 | `@ai-sdk/anthropic` |
| Anthropic | `claude-opus-4-6` | Claude Opus 4.6 | `@ai-sdk/anthropic` |
| Anthropic | `claude-opus-4-7` | Claude Opus 4.7 | `@ai-sdk/anthropic` |
| Anthropic | `claude-opus-4-8` | Claude Opus 4.8 | `@ai-sdk/anthropic` |
| Anthropic | `claude-sonnet-4-6` | Claude Sonnet 4.6 | `@ai-sdk/anthropic` |
| Anthropic | `claude-haiku-4-5` | Claude Haiku 4.5 | `@ai-sdk/anthropic` |
| Google | `gemini-3.6-flash` | Gemini 3.6 Flash | `@ai-sdk/google` |
| Google | `gemini-3.5-flash` | Gemini 3.5 Flash | `@ai-sdk/google` |
| Google | `gemini-3-pro` | Gemini 3 Pro | `@ai-sdk/google` |
| Google | `gemini-3-flash` | Gemini 3 Flash | `@ai-sdk/google` |
| Google | `gemini-3-1-pro` | Gemini 3.1 Pro | `@ai-sdk/google` |
| Google | `gemini-2.5-pro` | Gemini 2.5 Pro | `@ai-sdk/google` |
| Google | `gemini-3.5-flash-lite` | Gemini 3.5 Flash Lite | `@ai-sdk/google` |
| Google | `gemini-2.5-flash` | Gemini 2.5 Flash | `@ai-sdk/google` |
| DeepSeek | `deepseek-v4-pro` | DeepSeek V4 Pro | `@ai-sdk/openai-compatible` |
| DeepSeek | `deepseek-v4-flash` | DeepSeek V4 Flash | `@ai-sdk/openai-compatible` |
| DeepSeek | `deepseek-chat` | DeepSeek Chat | `@ai-sdk/openai-compatible` |
| DeepSeek | `deepseek-v3` | DeepSeek V3 | `@ai-sdk/openai-compatible` |
| Alibaba | `Qwen3-Coder-Next` | Qwen3 Coder Next | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3.5-plus` | Qwen3.5 Plus | `@ai-sdk/openai-compatible` |
| Alibaba | `Qwen3-235B-A22B-Instruct-2507` | Qwen3 235B A22B Instruct | `@ai-sdk/openai-compatible` |
| Alibaba | `Qwen3-30B-A3B-Instruct-2507` | Qwen3 30B A3B Instruct | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3-coder-plus` | Qwen3 Coder Plus | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3-max` | Qwen3 Max | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3.5-397b-a17b` | Qwen3.5 397B A17B | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3.8-max` | Qwen3.8 Max | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3.7-max` | Qwen3.7 Max | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3.7-plus` | Qwen3.7 Plus | `@ai-sdk/openai-compatible` |
| Alibaba | `qwen3-7-flash` | Qwen3.7 Flash | `@ai-sdk/openai-compatible` |
| Z.AI | `glm-5.2` | GLM-5.2 | `@ai-sdk/openai-compatible` |
| Z.AI | `glm-5.1` | GLM-5.1 | `@ai-sdk/openai-compatible` |
| Z.AI | `glm-5` | GLM-5 | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k3` | Kimi K3 | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k2.7-code` | Kimi K2.7 Code | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k2.7-code-highspeed` | Kimi K2.7 Code HighSpeed | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k2.6` | Kimi K2.6 | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k2.5` | Kimi K2.5 | `@ai-sdk/openai-compatible` |
| Moonshot AI | `kimi-k2-thinking` | Kimi K2 Thinking | `@ai-sdk/openai-compatible` |
| Moonshot AI | `Kimi-K2` | Kimi K2 | `@ai-sdk/openai-compatible` |
| MiniMax | `MiniMax-M3` | MiniMax M3 | `@ai-sdk/openai-compatible` |
| MiniMax | `MiniMax-M2.7` | MiniMax M2.7 | `@ai-sdk/openai-compatible` |
| MiniMax | `MiniMax-M2.7-highspeed` | MiniMax M2.7 Highspeed | `@ai-sdk/openai-compatible` |
| MiniMax | `MiniMax-M2.5-highspeed` | MiniMax M2.5 Highspeed | `@ai-sdk/openai-compatible` |
| MiniMax | `MiniMax-M2.1-highspeed` | MiniMax M2.1 Highspeed | `@ai-sdk/openai-compatible` |
| xAI | `grok-4.5-latest` | Grok 4.5 Latest | `@ai-sdk/openai-compatible` |
| xAI | `grok-4.3-latest` | Grok 4.3 Latest | `@ai-sdk/openai-compatible` |
| xAI | `grok-4-1-fast-reasoning-latest` | Grok 4.1 Fast Reasoning | `@ai-sdk/openai-compatible` |
| xAI | `grok-code-fast` | Grok Code Fast | `@ai-sdk/openai-compatible` |
| xAI | `grok-3` | Grok 3 | `@ai-sdk/openai-compatible` |
| xAI | `grok-3-mini` | Grok 3 Mini | `@ai-sdk/openai-compatible` |
| Mistral | `mistral-code-latest` | Mistral Code Latest | `@ai-sdk/openai-compatible` |
| Mistral | `mistral-medium-3-5` | Mistral Medium 3.5 | `@ai-sdk/openai-compatible` |
| Mistral | `devstral-latest` | Devstral 2 | `@ai-sdk/openai-compatible` |
| Mistral | `devstral-medium-latest` | Devstral 2 Medium | `@ai-sdk/openai-compatible` |
| Xiaomi | `mimo-v2.5-pro` | MiMo V2.5 Pro | `@ai-sdk/openai-compatible` |
| Xiaomi | `mimo-v2.5-pro-ultraspeed` | MiMo V2.5 Pro UltraSpeed | `@ai-sdk/openai-compatible` |
| Xiaomi | `mimo-v2-pro` | MiMo V2 Pro | `@ai-sdk/openai-compatible` |
| Xiaomi | `mimo-v2-flash` | MiMo V2 Flash | `@ai-sdk/openai-compatible` |
| NVIDIA | `nemotron-3-ultra-550b-a55b` | Nemotron 3 Ultra 550B A55B | `@ai-sdk/openai-compatible` |
| NVIDIA | `nemotron-3-super-120b-a12b` | Nemotron 3 Super 120B A12B | `@ai-sdk/openai-compatible` |
| ByteDance | `doubao-seed-2-0-code` | Doubao Seed 2.0 Code | `@ai-sdk/openai-compatible` |
| ByteDance | `doubao-seed-2-1-pro` | Doubao Seed 2.1 Pro | `@ai-sdk/openai-compatible` |
| ByteDance | `doubao-seed-2-1-turbo` | Doubao Seed 2.1 Turbo | `@ai-sdk/openai-compatible` |
| Cohere | `command-a-plus-05-2026` | Command A Plus | `@ai-sdk/openai-compatible` |

The workflow probes OpenAI and Anthropic at `/v1/models` and Google at `/v1beta/models` when the
entered base URL is a site root. An explicitly supplied path prefix is preserved and receives one
`models` segment. The three authentication shapes are attempted in parallel. Discovered IDs are
shown first and marked `AVAILABLE`; a provider failure is
rendered as a warning while other results remain usable. The initial single-select list includes
`Add all N available models`, discovered models, `Show all catalog models`, and `Custom model ID`.
Selecting `Add all` writes the complete discovered catalog for OpenCode, OpenClaw, and Hermes, then
asks for one primary/default model from that same catalog. Selecting `Show all` instead adds the
local presets after the discovered results and removes duplicate IDs. A preset supplies the initial
SDK and display name; both remain editable before the final numbered review. The scan
uses the entered token only for the request, never stores response bodies, caps each response at
1 MiB, and times out each protocol independently. Each API retries transient request/read failures,
timeouts, HTTP 429, and HTTP 5xx up to three total attempts with short exponential backoff. Other
HTTP statuses, redirects, oversized responses, malformed JSON, and incompatible schemas fail on the
first attempt. A 404/405 on a known Anthropic-compatible routing suffix tries a suffix-stripped
protocol candidate while leaving the supplied path first. Debug diagnostics report the final
endpoint and total attempt count without response bodies or secrets.

For OpenCode, each Add all entry retains its SDK family and is written to one of four protocol
providers: `rewire-oairesp`, `rewire-anthropic`, `rewire-google`, or `rewire-oaicomp`. The primary
selection points to the matching partition. This prevents a mixed catalog's Claude and Gemini
entries from inheriting `@ai-sdk/openai-compatible` merely because they share a gateway endpoint.
Hermes and OpenClaw keep their existing single-provider catalog representations.
