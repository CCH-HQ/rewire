use super::{
    client_directory, object, provider_recipe, removal_recipe, string, versioned_root_url,
};
use crate::model::{Client, Format, Recipe};
use serde_json::Value;
use std::path::Path;

pub(super) fn recipes(home: &Path, base_url: &str, token: &str) -> Vec<Recipe> {
    // Codex appends `/responses` to the configured provider base. A bare gateway origin therefore
    // needs the standard OpenAI `/v1` prefix, while an operator-supplied routing path is complete.
    let provider_base_url = versioned_root_url(base_url, "v1");
    let directory = client_directory(home, "CODEX_HOME", ".codex");
    vec![
        legacy_cleanup_recipe(&directory),
        provider_recipe(
            Client::Codex,
            directory.join("rewire.config.toml"),
            Format::Toml,
            object([
                ("model_provider", string("rewire")),
                (
                    "model_providers",
                    object([(
                        "rewire",
                        object([
                            ("name", string("Rewire")),
                            ("base_url", string(provider_base_url)),
                            // Codex 0.147 keeps this field for programmatic credentials. The
                            // dedicated profile avoids replacing auth.json or the base model.
                            ("experimental_bearer_token", string(token)),
                            ("wire_api", string("responses")),
                            ("requires_openai_auth", Value::Bool(false)),
                        ]),
                    )]),
                ),
            ]),
            true,
            "/model_providers/rewire/base_url",
            None,
        ),
    ]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    let directory = client_directory(home, "CODEX_HOME", ".codex");
    vec![
        legacy_cleanup_recipe(&directory),
        // The v2 profile file is entirely Rewire-owned, so removal deletes it atomically.
        removal_recipe(
            Client::Codex,
            directory.join("rewire.config.toml"),
            Format::Plain,
            Value::Null,
            true,
        ),
    ]
}

fn legacy_cleanup_recipe(directory: &Path) -> Recipe {
    // Codex 0.134+ rejects `[profiles.rewire]` when `--profile rewire` is selected. Keep this
    // removal in both configure and remove so older Rewire output converges to profile-v2.
    removal_recipe(
        Client::Codex,
        directory.join("config.toml"),
        Format::Toml,
        object([
            ("model_providers", object([("rewire", Value::Null)])),
            ("profiles", object([("rewire", Value::Null)])),
        ]),
        false,
    )
}

#[cfg(test)]
#[path = "codex/tests.rs"]
mod tests;
