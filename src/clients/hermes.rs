use super::{
    honor_client_environment, object, provider_endpoint_alias, provider_recipe, removal_recipe,
    string, structured_recipe, versioned_root_url,
};
use crate::model::{
    Client, ConditionalRemoval, Format, ModelConfig, OpenCodeSdk, Recipe, RemovalPredicate,
};
use serde_json::{Map, Value};
use std::env;
use std::path::{Path, PathBuf};

const PROVIDER_KEY_POINTER: &str = "/providers/rewire/key_env";
const PROVIDER_KEY_ENV: &str = "REWIRE_TOKEN";

pub(super) fn recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    selected_sdk: OpenCodeSdk,
    models: &[ModelConfig],
) -> Vec<Recipe> {
    let directory = config_directory(home);
    let runtime_base_url = hermes_base_url(base_url, selected_sdk);
    let mut config = provider_recipe(
        Client::Hermes,
        directory.join("config.yaml"),
        Format::Yaml,
        object([
            ("model", model_selection(&runtime_base_url, model)),
            (
                "providers",
                object([(
                    "rewire",
                    provider(&runtime_base_url, model, selected_sdk, models),
                )]),
            ),
        ]),
        false,
        "/providers/rewire/api",
        Some("/model/default"),
    );
    provider_endpoint_alias(
        &mut config,
        "/providers/rewire/base_url",
        "/providers/rewire/api",
    );
    attach_legacy_cleanup(&mut config);
    vec![
        config,
        structured_recipe(
            Client::Hermes,
            directory.join(".env"),
            Format::Dotenv,
            object([("REWIRE_TOKEN", string(token))]),
            true,
        ),
    ]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    let directory = config_directory(home);
    let mut config = removal_recipe(
        Client::Hermes,
        directory.join("config.yaml"),
        Format::Yaml,
        object([
            (
                "model",
                object([
                    ("default", Value::Null),
                    ("name", Value::Null),
                    ("provider", Value::Null),
                    ("base_url", Value::Null),
                ]),
            ),
            ("providers", object([("rewire", Value::Null)])),
        ]),
        false,
    );
    for pointer in [
        "/model/default",
        "/model/name",
        "/model/provider",
        "/model/base_url",
    ] {
        config.conditional_removals.push(ConditionalRemoval {
            removal_pointer: pointer,
            alternatives: vec![vec![exact_predicate("/model/provider", "rewire")]],
        });
    }
    vec![
        config,
        removal_recipe(
            Client::Hermes,
            directory.join(".env"),
            Format::Dotenv,
            object([("REWIRE_TOKEN", Value::Null)]),
            true,
        ),
    ]
}

fn model_selection(base_url: &str, model: Option<&str>) -> Value {
    object([
        ("default", string(model.unwrap_or_default())),
        ("provider", string("rewire")),
        ("base_url", string(base_url)),
    ])
}

fn provider(
    base_url: &str,
    model: Option<&str>,
    selected_sdk: OpenCodeSdk,
    models: &[ModelConfig],
) -> Value {
    object([
        ("name", string("Rewire")),
        ("api", string(base_url)),
        ("key_env", string(PROVIDER_KEY_ENV)),
        ("transport", string(hermes_transport(selected_sdk))),
        ("default_model", string(model.unwrap_or_default())),
        ("models", Value::Object(provider_models(models, model))),
    ])
}

const fn hermes_transport(sdk: OpenCodeSdk) -> &'static str {
    match sdk {
        OpenCodeSdk::OpenAi => "codex_responses",
        OpenCodeSdk::Anthropic => "anthropic_messages",
        // Hermes 0.19 exposes no user-provider Google transport. Google and generic compatible
        // models therefore use the gateway's OpenAI-compatible chat-completions surface.
        OpenCodeSdk::Google | OpenCodeSdk::OpenAiCompatible => "chat_completions",
    }
}

fn hermes_base_url(base_url: &str, sdk: OpenCodeSdk) -> String {
    match sdk {
        // The Anthropic SDK appends `/v1/messages`; all other supported custom transports append
        // their operation below an OpenAI-compatible `/v1` root.
        OpenCodeSdk::Anthropic => base_url.to_owned(),
        OpenCodeSdk::OpenAi | OpenCodeSdk::Google | OpenCodeSdk::OpenAiCompatible => {
            versioned_root_url(base_url, "v1")
        }
    }
}

fn provider_models(models: &[ModelConfig], selected: Option<&str>) -> Map<String, Value> {
    if models.is_empty() {
        return selected
            .map(|model| (model.to_owned(), Value::Object(Map::new())))
            .into_iter()
            .collect();
    }
    models
        .iter()
        .map(|model| (model.id.clone(), Value::Object(Map::new())))
        .collect()
}

fn attach_legacy_cleanup(recipe: &mut Recipe) {
    // Hermes accepts both normalizer aliases and its keyed provider fields. Converging on the
    // fields read directly by `resolve_user_provider` avoids depending on that compatibility pass.
    for pointer in [
        "/providers/rewire/base_url",
        "/providers/rewire/api_mode",
        "/providers/rewire/model",
        "/providers/rewire/models",
    ] {
        recipe.conditional_removals.push(ConditionalRemoval {
            removal_pointer: pointer,
            alternatives: vec![vec![exact_predicate(
                PROVIDER_KEY_POINTER,
                PROVIDER_KEY_ENV,
            )]],
        });
    }
    recipe.conditional_removals.push(ConditionalRemoval {
        removal_pointer: "/model/name",
        alternatives: vec![vec![exact_predicate("/model/provider", "rewire")]],
    });
}

fn exact_predicate(pointer: &'static str, expected: &str) -> RemovalPredicate {
    RemovalPredicate {
        pointer,
        expected: expected.to_owned(),
        prefix: false,
    }
}

fn config_directory(home: &Path) -> PathBuf {
    if honor_client_environment(home)
        && let Some(value) = env::var_os("HERMES_HOME").filter(|value| !value.is_empty())
    {
        return PathBuf::from(value);
    }
    default_directory(home)
}

#[cfg(target_os = "windows")]
fn default_directory(home: &Path) -> PathBuf {
    windows_default_directory(
        honor_client_environment(home)
            .then(|| env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()))
            .flatten()
            .as_deref(),
        home,
    )
}

#[cfg(not(target_os = "windows"))]
fn default_directory(home: &Path) -> PathBuf {
    home.join(".hermes")
}

#[cfg(any(target_os = "windows", test))]
fn windows_default_directory(local_app_data: Option<&std::ffi::OsStr>, home: &Path) -> PathBuf {
    local_app_data
        .map_or_else(|| home.join("AppData").join("Local"), PathBuf::from)
        .join("hermes")
}

#[cfg(test)]
#[path = "hermes/tests.rs"]
mod tests;
