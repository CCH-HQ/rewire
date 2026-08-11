use super::{
    client_file_from_env, honor_client_environment, object, plain_recipe, provider_model_reference,
    provider_recipe, removal_recipe, string, structured_recipe, versioned_root_url,
};
use crate::model::{
    Client, ConditionalRemoval, Format, ModelConfig, OpenCodeSdk, Recipe, RemovalPredicate,
};
use serde_json::{Map, Value};
use std::env;
use std::path::{Path, PathBuf};

const CATALOG_SDKS: [OpenCodeSdk; 4] = [
    OpenCodeSdk::OpenAi,
    OpenCodeSdk::Anthropic,
    OpenCodeSdk::Google,
    OpenCodeSdk::OpenAiCompatible,
];

pub(super) fn recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    model_name: Option<&str>,
    sdk: OpenCodeSdk,
    models: &[ModelConfig],
) -> Vec<Recipe> {
    let secret_path = home.join(".config/rewire/secrets/opencode-token");
    let reference = format!("{{file:{}}}", secret_path.to_string_lossy());
    let mut config = if !models.is_empty() {
        catalog_provider_recipe(home, base_url, model, reference.as_str(), sdk, models)
    } else if let Some(provider) = sdk.native_provider_id() {
        native_provider_recipe(home, base_url, model, provider, reference.as_str(), sdk)
    } else {
        custom_provider_recipe(
            home,
            base_url,
            model,
            model_name,
            reference.as_str(),
            sdk,
            models,
        )
    };
    attach_managed_provider_cleanup(&mut config, reference.as_str());
    vec![config, plain_recipe(Client::OpenCode, secret_path, token)]
}

fn catalog_provider_recipe(
    home: &Path,
    base_url: &str,
    model: Option<&str>,
    reference: &str,
    selected_sdk: OpenCodeSdk,
    catalog: &[ModelConfig],
) -> Recipe {
    // OpenCode chooses one AI SDK package per provider. A mixed discovery result must therefore
    // be split by wire protocol instead of inheriting one package from the default model.
    let mut providers = Map::new();
    let mut endpoint_pointers = Vec::new();
    for sdk in CATALOG_SDKS {
        let models = catalog_models(catalog, sdk);
        if models.is_empty() {
            continue;
        }
        let provider_id = catalog_provider_id(sdk);
        providers.insert(
            provider_id.to_owned(),
            object([
                ("name", string(catalog_provider_name(sdk))),
                ("npm", string(sdk.npm())),
                (
                    "options",
                    object([
                        ("baseURL", string(opencode_base_url(base_url, sdk))),
                        ("apiKey", string(reference)),
                    ]),
                ),
                ("models", Value::Object(models)),
            ]),
        );
        let endpoint_pointer = catalog_endpoint_pointer(sdk);
        endpoint_pointers.push((endpoint_pointer, endpoint_pointer));
    }
    let selected_provider = catalog_provider_id(selected_sdk);
    let mut recipe = structured_recipe(
        Client::OpenCode,
        config_path(home),
        Format::Json,
        object([
            ("model", string(model_reference(selected_provider, model))),
            ("provider", Value::Object(providers)),
        ]),
        false,
    );
    recipe.provider_endpoints = endpoint_pointers;
    recipe.selected_model = Some("/model");
    recipe
}

fn native_provider_recipe(
    home: &Path,
    base_url: &str,
    model: Option<&str>,
    provider: &'static str,
    reference: &str,
    sdk: OpenCodeSdk,
) -> Recipe {
    let endpoint_pointer = match sdk {
        OpenCodeSdk::OpenAi => "/provider/openai/options/baseURL",
        OpenCodeSdk::Anthropic => "/provider/anthropic/options/baseURL",
        OpenCodeSdk::Google | OpenCodeSdk::OpenAiCompatible => {
            unreachable!("only native OpenCode providers enter this branch")
        }
    };
    provider_recipe(
        Client::OpenCode,
        config_path(home),
        Format::Json,
        object([
            ("model", string(model_reference(provider, model))),
            (
                "provider",
                object([(
                    provider,
                    object([(
                        "options",
                        object([
                            ("baseURL", string(opencode_base_url(base_url, sdk))),
                            ("apiKey", string(reference)),
                        ]),
                    )]),
                )]),
            ),
        ]),
        false,
        endpoint_pointer,
        Some("/model"),
    )
}

fn custom_provider_recipe(
    home: &Path,
    base_url: &str,
    model: Option<&str>,
    model_name: Option<&str>,
    reference: &str,
    sdk: OpenCodeSdk,
    catalog: &[ModelConfig],
) -> Recipe {
    provider_recipe(
        Client::OpenCode,
        config_path(home),
        Format::Json,
        object([
            ("model", string(provider_model_reference(model))),
            (
                "provider",
                object([(
                    "rewire",
                    object([
                        ("name", string("Rewire")),
                        ("npm", string(sdk.npm())),
                        (
                            "options",
                            object([
                                ("baseURL", string(opencode_base_url(base_url, sdk))),
                                ("apiKey", string(reference)),
                            ]),
                        ),
                        ("models", models(catalog, model, model_name)),
                    ]),
                )]),
            ),
        ]),
        false,
        "/provider/rewire/options/baseURL",
        Some("/model"),
    )
}

fn attach_managed_provider_cleanup(recipe: &mut Recipe, reference: &str) {
    let active_providers = recipe
        .values
        .pointer("/provider")
        .and_then(Value::as_object)
        .map(|providers| providers.keys().map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for (provider_id, api_key_pointer) in managed_provider_pointers() {
        // The private token-file reference is the ownership marker. It lets Add all migrate the
        // legacy mixed `rewire` provider without deleting an operator's same-named provider.
        recipe.conditional_removals.push(ConditionalRemoval {
            removal_pointer: if active_providers.contains(&provider_id) {
                provider_models_pointer(provider_id)
            } else {
                provider_pointer(provider_id)
            },
            alternatives: vec![vec![exact_predicate(api_key_pointer, reference)]],
        });
    }
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    let reference = format!(
        "{{file:{}}}",
        home.join(".config/rewire/secrets/opencode-token")
            .to_string_lossy()
    );
    let mut config = removal_recipe(
        Client::OpenCode,
        config_path(home),
        Format::Json,
        object([
            ("model", Value::Null),
            (
                "provider",
                object([
                    ("rewire", Value::Null),
                    ("rewire-oairesp", Value::Null),
                    ("rewire-anthropic", Value::Null),
                    ("rewire-google", Value::Null),
                    ("rewire-oaicomp", Value::Null),
                    (
                        "openai",
                        object([(
                            "options",
                            object([("baseURL", Value::Null), ("apiKey", Value::Null)]),
                        )]),
                    ),
                    (
                        "anthropic",
                        object([(
                            "options",
                            object([("baseURL", Value::Null), ("apiKey", Value::Null)]),
                        )]),
                    ),
                ]),
            ),
        ]),
        false,
    );
    config.conditional_removals.push(ConditionalRemoval {
        removal_pointer: "/model",
        alternatives: vec![
            vec![prefix_predicate("/model", "rewire/")],
            vec![prefix_predicate("/model", "rewire-oairesp/")],
            vec![prefix_predicate("/model", "rewire-anthropic/")],
            vec![prefix_predicate("/model", "rewire-google/")],
            vec![prefix_predicate("/model", "rewire-oaicomp/")],
            vec![
                prefix_predicate("/model", "openai/"),
                exact_predicate("/provider/openai/options/apiKey", &reference),
            ],
            vec![
                prefix_predicate("/model", "anthropic/"),
                exact_predicate("/provider/anthropic/options/apiKey", &reference),
            ],
        ],
    });
    for provider in ["openai", "anthropic"] {
        let (base_url_pointer, api_key_pointer) = match provider {
            "openai" => (
                "/provider/openai/options/baseURL",
                "/provider/openai/options/apiKey",
            ),
            "anthropic" => (
                "/provider/anthropic/options/baseURL",
                "/provider/anthropic/options/apiKey",
            ),
            _ => unreachable!(),
        };
        for removal_pointer in [base_url_pointer, api_key_pointer] {
            config.conditional_removals.push(ConditionalRemoval {
                removal_pointer,
                alternatives: vec![vec![exact_predicate(api_key_pointer, &reference)]],
            });
        }
    }
    for (provider_id, api_key_pointer) in managed_provider_pointers() {
        if provider_id == "rewire" {
            // The legacy removal contract already owns this provider entry unconditionally.
            continue;
        }
        config.conditional_removals.push(ConditionalRemoval {
            removal_pointer: provider_pointer(provider_id),
            alternatives: vec![vec![exact_predicate(api_key_pointer, &reference)]],
        });
    }
    vec![
        config,
        removal_recipe(
            Client::OpenCode,
            home.join(".config/rewire/secrets/opencode-token"),
            Format::Plain,
            Value::Null,
            true,
        ),
    ]
}

fn model_reference(provider: &str, model: Option<&str>) -> String {
    format!("{provider}/{}", model.unwrap_or_default())
}

fn exact_predicate(pointer: &'static str, expected: &str) -> RemovalPredicate {
    RemovalPredicate {
        pointer,
        expected: expected.to_owned(),
        prefix: false,
    }
}

fn prefix_predicate(pointer: &'static str, expected: &str) -> RemovalPredicate {
    RemovalPredicate {
        pointer,
        expected: expected.to_owned(),
        prefix: true,
    }
}

fn models(catalog: &[ModelConfig], model: Option<&str>, model_name: Option<&str>) -> Value {
    let mut models = Map::new();
    if catalog.is_empty() {
        if let Some(model) = model {
            models.insert(
                model.into(),
                object([("name", string(model_name.unwrap_or(model)))]),
            );
        }
    } else {
        for model in catalog {
            models.insert(
                model.id.clone(),
                object([(
                    "name",
                    string(model.display_name.as_deref().unwrap_or(&model.id)),
                )]),
            );
        }
    }
    Value::Object(models)
}

fn catalog_models(catalog: &[ModelConfig], sdk: OpenCodeSdk) -> Map<String, Value> {
    catalog
        .iter()
        .filter(|model| model.sdk == sdk)
        .map(|model| {
            (
                model.id.clone(),
                object([(
                    "name",
                    string(model.display_name.as_deref().unwrap_or(&model.id)),
                )]),
            )
        })
        .collect()
}

const fn catalog_provider_id(sdk: OpenCodeSdk) -> &'static str {
    match sdk {
        OpenCodeSdk::OpenAi => "rewire-oairesp",
        OpenCodeSdk::Anthropic => "rewire-anthropic",
        OpenCodeSdk::Google => "rewire-google",
        OpenCodeSdk::OpenAiCompatible => "rewire-oaicomp",
    }
}

const fn catalog_provider_name(sdk: OpenCodeSdk) -> &'static str {
    match sdk {
        OpenCodeSdk::OpenAi => "Rewire OpenAI Responses",
        OpenCodeSdk::Anthropic => "Rewire Anthropic",
        OpenCodeSdk::Google => "Rewire Google",
        OpenCodeSdk::OpenAiCompatible => "Rewire OpenAI Compatible",
    }
}

const fn catalog_endpoint_pointer(sdk: OpenCodeSdk) -> &'static str {
    match sdk {
        OpenCodeSdk::OpenAi => "/provider/rewire-oairesp/options/baseURL",
        OpenCodeSdk::Anthropic => "/provider/rewire-anthropic/options/baseURL",
        OpenCodeSdk::Google => "/provider/rewire-google/options/baseURL",
        OpenCodeSdk::OpenAiCompatible => "/provider/rewire-oaicomp/options/baseURL",
    }
}

fn opencode_base_url(base_url: &str, sdk: OpenCodeSdk) -> String {
    // OpenCode passes `options.baseURL` directly to the selected AI SDK package. Their native
    // endpoint roots include the protocol version, unlike Claude Code's origin-style setting.
    let version = match sdk {
        OpenCodeSdk::Google => "v1beta",
        OpenCodeSdk::OpenAi | OpenCodeSdk::Anthropic | OpenCodeSdk::OpenAiCompatible => "v1",
    };
    versioned_root_url(base_url, version)
}

fn provider_pointer(provider_id: &str) -> &'static str {
    match provider_id {
        "rewire" => "/provider/rewire",
        "rewire-oairesp" => "/provider/rewire-oairesp",
        "rewire-anthropic" => "/provider/rewire-anthropic",
        "rewire-google" => "/provider/rewire-google",
        "rewire-oaicomp" => "/provider/rewire-oaicomp",
        _ => panic!("unknown Rewire-managed OpenCode provider"),
    }
}

fn provider_models_pointer(provider_id: &str) -> &'static str {
    match provider_id {
        "rewire" => "/provider/rewire/models",
        "rewire-oairesp" => "/provider/rewire-oairesp/models",
        "rewire-anthropic" => "/provider/rewire-anthropic/models",
        "rewire-google" => "/provider/rewire-google/models",
        "rewire-oaicomp" => "/provider/rewire-oaicomp/models",
        _ => panic!("unknown Rewire-managed OpenCode provider"),
    }
}

const fn managed_provider_pointers() -> [(&'static str, &'static str); 5] {
    [
        ("rewire", "/provider/rewire/options/apiKey"),
        ("rewire-oairesp", "/provider/rewire-oairesp/options/apiKey"),
        (
            "rewire-anthropic",
            "/provider/rewire-anthropic/options/apiKey",
        ),
        ("rewire-google", "/provider/rewire-google/options/apiKey"),
        ("rewire-oaicomp", "/provider/rewire-oaicomp/options/apiKey"),
    ]
}

fn config_path(home: &Path) -> PathBuf {
    if let Some(path) = client_file_from_env(home, "OPENCODE_CONFIG") {
        return path;
    }
    let directory = if honor_client_environment(home) {
        env::var_os("OPENCODE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("XDG_CONFIG_HOME")
                    .filter(|value| !value.is_empty())
                    .map(|value| PathBuf::from(value).join("opencode"))
            })
            .unwrap_or_else(|| home.join(".config/opencode"))
    } else {
        home.join(".config/opencode")
    };
    // Match OpenCode's globalConfigFile order and keep JSONC as the fresh default.
    ["opencode.jsonc", "opencode.json", "config.json"]
        .into_iter()
        .map(|name| directory.join(name))
        .find(|path| path.exists())
        .unwrap_or_else(|| directory.join("opencode.jsonc"))
}

#[cfg(test)]
#[path = "opencode/tests.rs"]
mod tests;
