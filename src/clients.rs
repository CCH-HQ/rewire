mod claude;
mod codex;
mod hermes;
mod openclaw;
mod opencode;

use crate::model::{
    Client, ConditionalRemoval, Format, ModelConfig, OpenCodeSdk, Recipe, RemovalPredicate,
};
use serde_json::{Map, Value};
use std::env;
use std::path::{Path, PathBuf};

pub const CLIENTS: &[Client] = &[
    Client::Claude,
    Client::Codex,
    Client::OpenCode,
    Client::Hermes,
    Client::OpenClaw,
];

/// Borrowed model-selection context shared by the client recipe adapters.
#[derive(Clone, Copy)]
pub(crate) struct ModelCatalogOptions<'a> {
    pub(crate) selected: Option<&'a str>,
    pub(crate) display_name: Option<&'a str>,
    pub(crate) sdk: Option<OpenCodeSdk>,
    pub(crate) models: &'a [ModelConfig],
}

impl Client {
    /// Build client-specific file recipes without giving adapters ownership of I/O or rollback.
    #[must_use]
    pub fn recipes(
        self,
        home: &Path,
        base_url: &str,
        token: &str,
        model: Option<&str>,
    ) -> Vec<Recipe> {
        self.recipes_with_options(home, base_url, token, model, None, None)
    }

    /// Build recipes with the complete model catalog metadata selected by the operator.
    #[must_use]
    pub fn recipes_with_options(
        self,
        home: &Path,
        base_url: &str,
        token: &str,
        model: Option<&str>,
        model_name: Option<&str>,
        sdk: Option<OpenCodeSdk>,
    ) -> Vec<Recipe> {
        self.recipes_with_catalog(
            home,
            base_url,
            token,
            ModelCatalogOptions {
                selected: model,
                display_name: model_name,
                sdk,
                models: &[],
            },
        )
    }

    /// Build recipes that also publish a discovered model catalog for catalog-aware clients.
    #[must_use]
    pub(crate) fn recipes_with_catalog(
        self,
        home: &Path,
        base_url: &str,
        token: &str,
        options: ModelCatalogOptions<'_>,
    ) -> Vec<Recipe> {
        let ModelCatalogOptions {
            selected: model,
            display_name: model_name,
            sdk,
            models,
        } = options;
        match self {
            Self::Claude => claude::recipes(home, base_url, token),
            Self::Codex => codex::recipes(home, base_url, token),
            Self::OpenCode => opencode::recipes(
                home,
                base_url,
                token,
                model,
                model_name,
                sdk.unwrap_or_else(|| OpenCodeSdk::infer(model)),
                models,
            ),
            Self::Hermes => hermes::recipes(
                home,
                base_url,
                token,
                model,
                sdk.unwrap_or_else(|| OpenCodeSdk::infer(model)),
                models,
            ),
            Self::OpenClaw => openclaw::recipes(
                home,
                base_url,
                token,
                model,
                model_name,
                sdk.unwrap_or_else(|| OpenCodeSdk::infer(model)),
                models,
            ),
        }
    }

    /// Report process environment that may win over a planned Claude settings merge.
    pub(crate) fn environment_warnings(self, base_url: &str, token: &str) -> Vec<String> {
        if self == Self::Claude {
            claude::environment_warnings(base_url, token)
        } else {
            Vec::new()
        }
    }

    /// Build field ownership recipes for a transactional client removal.
    #[must_use]
    pub fn removal_recipes(self, home: &Path) -> Vec<Recipe> {
        match self {
            Self::Claude => claude::removal_recipes(home),
            Self::Codex => codex::removal_recipes(home),
            Self::OpenCode => opencode::removal_recipes(home),
            Self::Hermes => hermes::removal_recipes(home),
            Self::OpenClaw => openclaw::removal_recipes(home),
        }
    }
}

fn structured_recipe(
    client: Client,
    path: PathBuf,
    format: Format,
    values: Value,
    sensitive: bool,
) -> Recipe {
    Recipe {
        client,
        path,
        format,
        values,
        sensitive,
        provider_endpoints: Vec::new(),
        selected_model: None,
        conditional_removals: Vec::new(),
        removal: false,
    }
}

fn removal_recipe(
    client: Client,
    path: PathBuf,
    format: Format,
    values: Value,
    sensitive: bool,
) -> Recipe {
    let mut recipe = structured_recipe(client, path, format, values, sensitive);
    recipe.removal = true;
    recipe
}

fn provider_recipe(
    client: Client,
    path: PathBuf,
    format: Format,
    values: Value,
    sensitive: bool,
    provider_endpoint: &'static str,
    selected_model: Option<&'static str>,
) -> Recipe {
    let mut recipe = structured_recipe(client, path, format, values, sensitive);
    recipe
        .provider_endpoints
        .push((provider_endpoint, provider_endpoint));
    recipe.selected_model = selected_model;
    recipe
}

fn provider_endpoint_alias(
    recipe: &mut Recipe,
    existing_pointer: &'static str,
    requested_pointer: &'static str,
) {
    recipe
        .provider_endpoints
        .push((existing_pointer, requested_pointer));
}

fn plain_recipe(client: Client, path: PathBuf, token: &str) -> Recipe {
    structured_recipe(client, path, Format::Plain, string(token), true)
}

fn rewire_model_reference_removal(pointer: &'static str) -> ConditionalRemoval {
    ConditionalRemoval {
        removal_pointer: pointer,
        alternatives: vec![vec![RemovalPredicate {
            pointer,
            expected: "rewire/".into(),
            prefix: true,
        }]],
    }
}

fn provider_model_reference(model: Option<&str>) -> String {
    format!("rewire/{}", model.unwrap_or_default())
}

/// Add a protocol version only when the operator supplied a bare gateway origin.
/// Explicit paths can encode provider-specific routing and must remain untouched.
pub(super) fn versioned_root_url(base_url: &str, version: &str) -> String {
    let Ok(mut url) = url::Url::parse(base_url) else {
        return base_url.to_owned();
    };
    if matches!(url.path(), "" | "/") {
        url.set_path(&format!("/{version}"));
        return url.to_string().trim_end_matches('/').to_owned();
    }
    base_url.to_owned()
}

fn client_directory(home: &Path, variable: &str, fallback: &str) -> PathBuf {
    if honor_client_environment(home)
        && let Some(value) = env::var_os(variable).filter(|value| !value.is_empty())
    {
        return PathBuf::from(value);
    }
    home.join(fallback)
}

fn client_file_from_env(home: &Path, variable: &str) -> Option<PathBuf> {
    honor_client_environment(home)
        .then(|| env::var_os(variable).filter(|value| !value.is_empty()))
        .flatten()
        .map(PathBuf::from)
}

fn honor_client_environment(home: &Path) -> bool {
    directories::BaseDirs::new().is_some_and(|directories| directories.home_dir() == home)
}

fn string(value: impl Into<String>) -> Value {
    Value::String(value.into())
}
fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<Map<_, _>>(),
    )
}
