use super::{client_directory, object, removal_recipe, string, structured_recipe};
use crate::model::{Client, Format, Recipe};
use serde_json::Value;
use std::env;
use std::path::Path;

pub(super) fn recipes(home: &Path, base_url: &str, token: &str) -> Vec<Recipe> {
    // Claude Code treats a custom Anthropic gateway as an origin and appends `/v1/messages`.
    // `ANTHROPIC_AUTH_TOKEN` is the compatible-gateway bearer credential; API-key mode has
    // different header semantics and is intentionally left to an explicit operator setting.
    vec![structured_recipe(
        Client::Claude,
        client_directory(home, "CLAUDE_CONFIG_DIR", ".claude").join("settings.json"),
        Format::Json,
        object([(
            "env",
            object([
                ("ANTHROPIC_BASE_URL", string(base_url)),
                ("ANTHROPIC_AUTH_TOKEN", string(token)),
            ]),
        )]),
        true,
    )]
}

pub(super) fn removal_recipes(home: &Path) -> Vec<Recipe> {
    vec![removal_recipe(
        Client::Claude,
        client_directory(home, "CLAUDE_CONFIG_DIR", ".claude").join("settings.json"),
        Format::Json,
        object([(
            "env",
            object([
                ("ANTHROPIC_BASE_URL", Value::Null),
                ("ANTHROPIC_AUTH_TOKEN", Value::Null),
            ]),
        )]),
        false,
    )]
}

pub(super) fn environment_warnings(base_url: &str, token: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if env_value_differs("ANTHROPIC_BASE_URL", base_url, true) {
        warnings.push(
            "claude process environment ANTHROPIC_BASE_URL differs from the planned endpoint"
                .into(),
        );
    }
    if env_value_differs("ANTHROPIC_AUTH_TOKEN", token, false) {
        warnings.push(
            "claude process environment ANTHROPIC_AUTH_TOKEN differs from the planned credential"
                .into(),
        );
    }
    if env::var_os("ANTHROPIC_API_KEY").is_some_and(|value| !value.is_empty()) {
        warnings.push(
            "claude process environment ANTHROPIC_API_KEY is also set; review authentication precedence"
                .into(),
        );
    }
    warnings
}

fn env_value_differs(name: &str, planned: &str, normalize_url: bool) -> bool {
    let Some(value) = env::var_os(name).filter(|value| !value.is_empty()) else {
        return false;
    };
    let Some(value) = value.to_str() else {
        return true;
    };
    if normalize_url {
        crate::security::validate_base_url(value).map_or(true, |value| value != planned)
    } else {
        value != planned
    }
}
