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
            Self::Claude => claude_recipes(home, base_url, token),
            Self::Codex => codex_recipes(home, base_url, token),
            Self::OpenCode => opencode::recipes(
                home,
                base_url,
                token,
                model,
                model_name,
                sdk.unwrap_or_else(|| OpenCodeSdk::infer(model)),
                models,
            ),
            Self::Hermes => hermes_recipes(home, base_url, token, model, models),
            Self::OpenClaw => openclaw_recipes(home, base_url, token, model, model_name, models),
        }
    }

    /// Report process environment that may win over a planned Claude settings merge.
    pub(crate) fn environment_warnings(self, base_url: &str, token: &str) -> Vec<String> {
        if self != Self::Claude {
            return Vec::new();
        }
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

    /// Build field ownership recipes for a transactional client removal.
    #[must_use]
    pub fn removal_recipes(self, home: &Path) -> Vec<Recipe> {
        match self {
            Self::Claude => claude_removal_recipes(home),
            Self::Codex => codex_removal_recipes(home),
            Self::OpenCode => opencode::removal_recipes(home),
            Self::Hermes => hermes_removal_recipes(home),
            Self::OpenClaw => openclaw_removal_recipes(home),
        }
    }
}

fn claude_removal_recipes(home: &Path) -> Vec<Recipe> {
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

fn codex_removal_recipes(home: &Path) -> Vec<Recipe> {
    vec![removal_recipe(
        Client::Codex,
        client_directory(home, "CODEX_HOME", ".codex").join("config.toml"),
        Format::Toml,
        object([
            ("model_providers", object([("rewire", Value::Null)])),
            ("profiles", object([("rewire", Value::Null)])),
        ]),
        false,
    )]
}

fn hermes_removal_recipes(home: &Path) -> Vec<Recipe> {
    let directory = client_directory(home, "HERMES_HOME", ".hermes");
    let mut config = removal_recipe(
        Client::Hermes,
        directory.join("config.yaml"),
        Format::Yaml,
        object([
            ("model", Value::Null),
            ("providers", object([("rewire", Value::Null)])),
        ]),
        false,
    );
    config.conditional_removals.push(ConditionalRemoval {
        removal_pointer: "/model",
        alternatives: vec![vec![RemovalPredicate {
            pointer: "/model/provider",
            expected: "rewire".into(),
            prefix: false,
        }]],
    });
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

fn openclaw_removal_recipes(home: &Path) -> Vec<Recipe> {
    let directory = client_directory(home, "OPENCLAW_STATE_DIR", ".openclaw");
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

fn claude_recipes(home: &Path, base_url: &str, token: &str) -> Vec<Recipe> {
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

fn codex_recipes(home: &Path, base_url: &str, token: &str) -> Vec<Recipe> {
    vec![provider_recipe(
        Client::Codex,
        client_directory(home, "CODEX_HOME", ".codex").join("config.toml"),
        Format::Toml,
        object([
            (
                "model_providers",
                object([(
                    "rewire",
                    object([
                        ("name", string("Rewire")),
                        ("base_url", string(base_url)),
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

fn hermes_recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    models: &[ModelConfig],
) -> Vec<Recipe> {
    let directory = client_directory(home, "HERMES_HOME", ".hermes");
    vec![
        provider_recipe(
            Client::Hermes,
            directory.join("config.yaml"),
            Format::Yaml,
            object([
                ("model", hermes_model_selection(model)),
                (
                    "providers",
                    object([("rewire", hermes_provider(base_url, model, models))]),
                ),
            ]),
            false,
            "/providers/rewire/api",
            Some("/model"),
        ),
        structured_recipe(
            Client::Hermes,
            directory.join(".env"),
            Format::Dotenv,
            object([("REWIRE_TOKEN", string(token))]),
            true,
        ),
    ]
}

fn openclaw_recipes(
    home: &Path,
    base_url: &str,
    token: &str,
    model: Option<&str>,
    model_name: Option<&str>,
    models: &[ModelConfig],
) -> Vec<Recipe> {
    let directory = client_directory(home, "OPENCLAW_STATE_DIR", ".openclaw");
    let config = client_file_from_env(home, "OPENCLAW_CONFIG_PATH")
        .unwrap_or_else(|| directory.join("openclaw.json"));
    let secret_path = directory.join("secrets/rewire-token");
    let secret_ref = object([
        ("source", string("file")),
        ("provider", string("rewire")),
        ("id", string("value")),
    ]);
    vec![
        provider_recipe(
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
                                    ("baseUrl", string(base_url)),
                                    ("apiKey", secret_ref),
                                    ("api", string("openai-completions")),
                                    ("models", openclaw_models(models, model, model_name)),
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
        ),
        plain_recipe(Client::OpenClaw, secret_path, token),
    ]
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
    recipe.provider_endpoints.push(provider_endpoint);
    recipe.selected_model = selected_model;
    recipe
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

fn hermes_model_selection(model: Option<&str>) -> Value {
    object([
        ("provider", string("rewire")),
        ("name", string(model.unwrap_or_default())),
    ])
}

fn openclaw_models(models: &[ModelConfig], model: Option<&str>, model_name: Option<&str>) -> Value {
    if models.is_empty() {
        return Value::Array(
            model
                .map(|model| {
                    object([
                        ("id", string(model)),
                        ("name", string(model_name.unwrap_or(model))),
                    ])
                })
                .into_iter()
                .collect(),
        );
    }
    Value::Array(
        models
            .iter()
            .map(|model| {
                object([
                    ("id", string(&model.id)),
                    (
                        "name",
                        string(model.display_name.as_deref().unwrap_or(&model.id)),
                    ),
                ])
            })
            .collect(),
    )
}

fn hermes_provider(base_url: &str, model: Option<&str>, models: &[ModelConfig]) -> Value {
    let mut provider = Map::from_iter([
        ("name".to_owned(), string("Rewire")),
        ("api".to_owned(), string(base_url)),
        ("key_env".to_owned(), string("REWIRE_TOKEN")),
        ("transport".to_owned(), string("chat_completions")),
        (
            "default_model".to_owned(),
            string(model.unwrap_or_default()),
        ),
    ]);
    if !models.is_empty() {
        provider.insert(
            "models".to_owned(),
            Value::Array(models.iter().map(|model| string(&model.id)).collect()),
        );
    }
    Value::Object(provider)
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
