use crate::model::OpenCodeSdk;

/// A curated model preset used by the guided workflow.
///
/// The catalog stores provider-native IDs. Adapters add client-specific qualification, such as
/// `OpenCode`'s native or Rewire-managed qualified selection at the configuration boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub sdk: OpenCodeSdk,
}

/// Popular text-generation and coding presets from the local official snapshot and gateway docs.
///
/// This is a reviewed local catalog rather than a runtime capability probe. The custom model
/// workflow option remains available for IDs released after this list was updated.
pub const POPULAR_MODELS: &[ModelPreset] = &[
    ModelPreset {
        id: "gpt-5.5",
        display_name: "GPT-5.5",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.6",
        display_name: "GPT-5.6",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.3-codex",
        display_name: "GPT-5.3 Codex",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.3-codex-spark",
        display_name: "GPT-5.3 Codex Spark",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.5-pro-2026-04-23",
        display_name: "GPT-5.5 Pro",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.2-codex",
        display_name: "GPT-5.2 Codex",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-5.1-codex-max",
        display_name: "GPT-5.1 Codex Max",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "gpt-oss-120b",
        display_name: "GPT OSS 120B",
        provider: "OpenAI",
        sdk: OpenCodeSdk::OpenAi,
    },
    ModelPreset {
        id: "claude-opus-5",
        display_name: "Claude Opus 5",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-sonnet-5",
        display_name: "Claude Sonnet 5",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-opus-4-6",
        display_name: "Claude Opus 4.6",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-opus-4-7",
        display_name: "Claude Opus 4.7",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-opus-4-8",
        display_name: "Claude Opus 4.8",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-sonnet-4-6",
        display_name: "Claude Sonnet 4.6",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "claude-haiku-4-5",
        display_name: "Claude Haiku 4.5",
        provider: "Anthropic",
        sdk: OpenCodeSdk::Anthropic,
    },
    ModelPreset {
        id: "gemini-3.6-flash",
        display_name: "Gemini 3.6 Flash",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-3.5-flash",
        display_name: "Gemini 3.5 Flash",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-3-pro",
        display_name: "Gemini 3 Pro",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-3-flash",
        display_name: "Gemini 3 Flash",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-3-1-pro",
        display_name: "Gemini 3.1 Pro",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-2.5-pro",
        display_name: "Gemini 2.5 Pro",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-3.5-flash-lite",
        display_name: "Gemini 3.5 Flash Lite",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "gemini-2.5-flash",
        display_name: "Gemini 2.5 Flash",
        provider: "Google",
        sdk: OpenCodeSdk::Google,
    },
    ModelPreset {
        id: "deepseek-v4-pro",
        display_name: "DeepSeek V4 Pro",
        provider: "DeepSeek",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "deepseek-v4-flash",
        display_name: "DeepSeek V4 Flash",
        provider: "DeepSeek",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "deepseek-chat",
        display_name: "DeepSeek Chat",
        provider: "DeepSeek",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "deepseek-v3",
        display_name: "DeepSeek V3",
        provider: "DeepSeek",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "Qwen3-Coder-Next",
        display_name: "Qwen3 Coder Next",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3.5-plus",
        display_name: "Qwen3.5 Plus",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "Qwen3-235B-A22B-Instruct-2507",
        display_name: "Qwen3 235B A22B Instruct",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "Qwen3-30B-A3B-Instruct-2507",
        display_name: "Qwen3 30B A3B Instruct",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3-coder-plus",
        display_name: "Qwen3 Coder Plus",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3-max",
        display_name: "Qwen3 Max",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3.5-397b-a17b",
        display_name: "Qwen3.5 397B A17B",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3.8-max",
        display_name: "Qwen3.8 Max",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3.7-max",
        display_name: "Qwen3.7 Max",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3.7-plus",
        display_name: "Qwen3.7 Plus",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "qwen3-7-flash",
        display_name: "Qwen3.7 Flash",
        provider: "Alibaba",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "glm-5.2",
        display_name: "GLM-5.2",
        provider: "Z.AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "glm-5.1",
        display_name: "GLM-5.1",
        provider: "Z.AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "glm-5",
        display_name: "GLM-5",
        provider: "Z.AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k3",
        display_name: "Kimi K3",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k2.7-code",
        display_name: "Kimi K2.7 Code",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k2.7-code-highspeed",
        display_name: "Kimi K2.7 Code HighSpeed",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k2.6",
        display_name: "Kimi K2.6",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k2.5",
        display_name: "Kimi K2.5",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "kimi-k2-thinking",
        display_name: "Kimi K2 Thinking",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "Kimi-K2",
        display_name: "Kimi K2",
        provider: "Moonshot AI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "MiniMax-M3",
        display_name: "MiniMax M3",
        provider: "MiniMax",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "MiniMax-M2.7",
        display_name: "MiniMax M2.7",
        provider: "MiniMax",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "MiniMax-M2.7-highspeed",
        display_name: "MiniMax M2.7 Highspeed",
        provider: "MiniMax",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "MiniMax-M2.5-highspeed",
        display_name: "MiniMax M2.5 Highspeed",
        provider: "MiniMax",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "MiniMax-M2.1-highspeed",
        display_name: "MiniMax M2.1 Highspeed",
        provider: "MiniMax",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-4.5-latest",
        display_name: "Grok 4.5 Latest",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-4.3-latest",
        display_name: "Grok 4.3 Latest",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-4-1-fast-reasoning-latest",
        display_name: "Grok 4.1 Fast Reasoning",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-code-fast",
        display_name: "Grok Code Fast",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-3",
        display_name: "Grok 3",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "grok-3-mini",
        display_name: "Grok 3 Mini",
        provider: "xAI",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mistral-code-latest",
        display_name: "Mistral Code Latest",
        provider: "Mistral",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mistral-medium-3-5",
        display_name: "Mistral Medium 3.5",
        provider: "Mistral",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "devstral-latest",
        display_name: "Devstral 2",
        provider: "Mistral",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "devstral-medium-latest",
        display_name: "Devstral 2 Medium",
        provider: "Mistral",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mimo-v2.5-pro",
        display_name: "MiMo V2.5 Pro",
        provider: "Xiaomi",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mimo-v2.5-pro-ultraspeed",
        display_name: "MiMo V2.5 Pro UltraSpeed",
        provider: "Xiaomi",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mimo-v2-pro",
        display_name: "MiMo V2 Pro",
        provider: "Xiaomi",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "mimo-v2-flash",
        display_name: "MiMo V2 Flash",
        provider: "Xiaomi",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "nemotron-3-ultra-550b-a55b",
        display_name: "Nemotron 3 Ultra 550B A55B",
        provider: "NVIDIA",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "nemotron-3-super-120b-a12b",
        display_name: "Nemotron 3 Super 120B A12B",
        provider: "NVIDIA",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "doubao-seed-2-0-code",
        display_name: "Doubao Seed 2.0 Code",
        provider: "ByteDance",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "doubao-seed-2-1-pro",
        display_name: "Doubao Seed 2.1 Pro",
        provider: "ByteDance",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "doubao-seed-2-1-turbo",
        display_name: "Doubao Seed 2.1 Turbo",
        provider: "ByteDance",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
    ModelPreset {
        id: "command-a-plus-05-2026",
        display_name: "Command A Plus",
        provider: "Cohere",
        sdk: OpenCodeSdk::OpenAiCompatible,
    },
];

/// Return the local catalog in guided-workflow display order.
#[must_use]
pub const fn popular_models() -> &'static [ModelPreset] {
    POPULAR_MODELS
}

/// Find a preset by its exact provider-native model ID.
#[must_use]
pub fn find_model(id: &str) -> Option<ModelPreset> {
    POPULAR_MODELS
        .iter()
        .copied()
        .find(|preset| preset.id == id)
}
