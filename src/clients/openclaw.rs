use super::{
    client_file_from_env, object, plain_recipe, provider_model_reference, provider_recipe,
    removal_recipe, rewire_model_reference_removal, string, versioned_root_url,
};
use crate::model::{Client, Format, ModelConfig, OpenCodeSdk, Recipe};
use serde_json::Value;
use std::path::Path;

pub(super) fn recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    model_name: Option<&str>,
    selected_sdk: OpenCodeSdk,
    models: &[ModelConfig],
) -> Vec<Recipe> {
    let directory = super::client_directory(home, "OPENCLAW_STATE_DIR", ".openclaw");
    let config = client_file_from_env(home, "OPENCLAW_CONFIG_PATH")
        .unwrap_or_else(|| directory.join("openclaw.json"));
    let secret_path = directory.join("secrets/rewire-token");
    let secret_ref = object([
        ("source", string("file")),
        ("provider", string("rewire")),
        ("id", string("value")),
    ]);
    let config_recipe = provider_recipe(
        Client::OpenClaw,
        config,
        Format::Json,
        object([
            (
                "secrets",
                object([(
                    "providers",
                    object([(
                        "rewire",
                        object([
                            ("source", string("file")),
                            ("path", string(secret_path.to_string_lossy())),
                            ("mode", string("singleValue")),
                        ]),
                    )]),
                )]),
            ),
            (
                "models",
                object([
                    ("mode", string("merge")),
                    (
                        "providers",
                        object([(
                            "rewire",
                            object([
                                ("baseUrl", string(openclaw_base_url(base_url, selected_sdk))),
                                ("apiKey", secret_ref),
                                ("api", string(openclaw_api(selected_sdk))),
                                (
                                    "models",
                                    openclaw_models(
                                        base_url,
                                        models,
                                        model,
                                        model_name,
                                        selected_sdk,
                                    ),
                                ),
                            ]),
                        )]),
                    ),
                ]),
            ),
            (
                "agents",
                object([(
                    "defaults",
                    object([(
                        "model",
                        object([("primary", string(provider_model_reference(model)))]),
                    )]),
                )]),
            ),
        ]),
        false,
        "/models/providers/rewire/baseUrl",
        Some("/agents/defaults/model/primary"),
    );
    vec![
        config_recipe,
        plain_recipe(Client::OpenClaw, secret_path, token),
    ]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    let directory = super::client_directory(home, "OPENCLAW_STATE_DIR", ".openclaw");
    let config_path = client_file_from_env(home, "OPENCLAW_CONFIG_PATH")
        .unwrap_or_else(|| directory.join("openclaw.json"));
    let mut config = removal_recipe(
        Client::OpenClaw,
        config_path,
        Format::Json,
        object([
            (
                "agents",
                object([(
                    "defaults",
                    object([("model", object([("primary", Value::Null)]))]),
                )]),
            ),
            (
                "secrets",
                object([("providers", object([("rewire", Value::Null)]))]),
            ),
            (
                "models",
                object([("providers", object([("rewire", Value::Null)]))]),
            ),
        ]),
        false,
    );
    config
        .conditional_removals
        .push(rewire_model_reference_removal(
            "/agents/defaults/model/primary",
        ));
    vec![
        config,
        removal_recipe(
            Client::OpenClaw,
            directory.join("secrets/rewire-token"),
            Format::Plain,
            Value::Null,
            true,
        ),
    ]
}

fn openclaw_models(
    base_url: &str,
    models: &[ModelConfig],
    selected: Option<&str>,
    selected_name: Option<&str>,
    selected_sdk: OpenCodeSdk,
) -> Value {
    if models.is_empty() {
        return Value::Array(
            selected
                .map(|model| model_entry(base_url, model, selected_name, selected_sdk))
                .into_iter()
                .collect(),
        );
    }
    Value::Array(
        models
            .iter()
            .map(|model| {
                model_entry(
                    base_url,
                    &model.id,
                    model.display_name.as_deref(),
                    model.sdk,
                )
            })
            .collect(),
    )
}

fn model_entry(base_url: &str, id: &str, name: Option<&str>, sdk: OpenCodeSdk) -> Value {
    object([
        ("id", string(id)),
        ("name", string(name.unwrap_or(id))),
        ("api", string(openclaw_api(sdk))),
        ("baseUrl", string(openclaw_base_url(base_url, sdk))),
    ])
}

const fn openclaw_api(sdk: OpenCodeSdk) -> &'static str {
    match sdk {
        OpenCodeSdk::OpenAi => "openai-responses",
        OpenCodeSdk::Anthropic => "anthropic-messages",
        OpenCodeSdk::Google => "google-generative-ai",
        OpenCodeSdk::OpenAiCompatible => "openai-completions",
    }
}

fn openclaw_base_url(base_url: &str, sdk: OpenCodeSdk) -> String {
    match sdk {
        // OpenClaw's Anthropic transport appends `/v1/messages` itself.
        OpenCodeSdk::Anthropic => base_url.to_owned(),
        OpenCodeSdk::Google => versioned_root_url(base_url, "v1beta"),
        OpenCodeSdk::OpenAi | OpenCodeSdk::OpenAiCompatible => versioned_root_url(base_url, "v1"),
    }
}

#[cfg(test)]
#[path = "openclaw/tests.rs"]
mod tests;
