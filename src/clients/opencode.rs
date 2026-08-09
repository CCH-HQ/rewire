use super::{
    client_file_from_env, honor_client_environment, object, plain_recipe, provider_model_reference,
    provider_recipe, removal_recipe, rewire_model_reference_removal, string,
};
use crate::model::{Client, Format, OpenCodeSdk, Recipe};
use serde_json::{Map, Value};
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    model_name: Option<&str>,
    sdk: OpenCodeSdk,
) -> Vec<Recipe> {
    let secret_path = home.join(".config/rewire/secrets/opencode-token");
    let reference = format!("{{file:{}}}", secret_path.to_string_lossy());
    vec![
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
                                    ("baseURL", string(base_url)),
                                    ("apiKey", string(reference)),
                                ]),
                            ),
                            ("models", models(model, model_name)),
                        ]),
                    )]),
                ),
            ]),
            false,
            "/provider/rewire/options/baseURL",
            Some("/model"),
        ),
        plain_recipe(Client::OpenCode, secret_path, token),
    ]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    let mut config = removal_recipe(
        Client::OpenCode,
        config_path(home),
        Format::Json,
        object([
            ("model", Value::Null),
            ("provider", object([("rewire", Value::Null)])),
        ]),
        false,
    );
    config
        .conditional_removals
        .push(rewire_model_reference_removal("/model"));
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

fn models(model: Option<&str>, model_name: Option<&str>) -> Value {
    let mut models = Map::new();
    if let Some(model) = model {
        models.insert(
            model.into(),
            object([("name", string(model_name.unwrap_or(model)))]),
        );
    }
    Value::Object(models)
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
