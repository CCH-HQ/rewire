use super::{object, provider_recipe, removal_recipe, string, versioned_root_url};
use crate::model::{Client, Format, Recipe};
use serde_json::Value;
use std::path::Path;

pub(super) fn recipes(home: &Path, base_url: &str, token: &str) -> Vec<Recipe> {
    // Codex appends `/responses` to the configured provider base. A bare gateway origin therefore
    // needs the standard OpenAI `/v1` prefix, while an operator-supplied routing path is complete.
    let provider_base_url = versioned_root_url(base_url, "v1");
    vec![provider_recipe(
        Client::Codex,
        super::client_directory(home, "CODEX_HOME", ".codex").join("config.toml"),
        Format::Toml,
        object([
            (
                "model_providers",
                object([(
                    "rewire",
                    object([
                        ("name", string("Rewire")),
                        ("base_url", string(provider_base_url)),
                        // Codex 0.147 keeps this field for programmatic credentials. Using the
                        // isolated provider avoids replacing the user's auth.json login state.
                        ("experimental_bearer_token", string(token)),
                        ("wire_api", string("responses")),
                        ("requires_openai_auth", Value::Bool(false)),
                    ]),
                )]),
            ),
            (
                "profiles",
                object([("rewire", object([("model_provider", string("rewire"))]))]),
            ),
        ]),
        true,
        "/model_providers/rewire/base_url",
        None,
    )]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    vec![removal_recipe(
        Client::Codex,
        super::client_directory(home, "CODEX_HOME", ".codex").join("config.toml"),
        Format::Toml,
        object([
            ("model_providers", object([("rewire", Value::Null)])),
            ("profiles", object([("rewire", Value::Null)])),
        ]),
        false,
    )]
}

#[cfg(test)]
#[path = "codex/tests.rs"]
mod tests;
