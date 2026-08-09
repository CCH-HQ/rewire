use crate::model::{Client, Format, Recipe};
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
        match self {
            Self::Claude => claude_recipes(home, base_url, token),
            Self::Codex => codex_recipes(home, base_url, token, model),
            Self::OpenCode => opencode_recipes(home, base_url, token, model),
            Self::Hermes => hermes_recipes(home, base_url, token, model),
            Self::OpenClaw => openclaw_recipes(home, base_url, token, model),
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
            Self::Claude => vec![removal_recipe(
                self,
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
            )],
            Self::Codex => vec![removal_recipe(
                self,
                client_directory(home, "CODEX_HOME", ".codex").join("config.toml"),
                Format::Toml,
                object([
                    ("model_providers", object([("rewire", Value::Null)])),
                    ("profiles", object([("rewire", Value::Null)])),
                ]),
                false,
            )],
            Self::OpenCode => vec![
                removal_recipe(
                    self,
                    opencode_config_path(home),
                    Format::Json,
                    object([("provider", object([("rewire", Value::Null)]))]),
                    false,
                ),
                removal_recipe(
                    self,
                    home.join(".config/rewire/secrets/opencode-token"),
                    Format::Plain,
                    Value::Null,
                    true,
                ),
            ],
            Self::Hermes => {
                let directory = client_directory(home, "HERMES_HOME", ".hermes");
                vec![
                    removal_recipe(
                        self,
                        directory.join("config.yaml"),
                        Format::Yaml,
                        object([("providers", object([("rewire", Value::Null)]))]),
                        false,
                    ),
                    removal_recipe(
                        self,
                        directory.join(".env"),
                        Format::Dotenv,
                        object([("REWIRE_TOKEN", Value::Null)]),
                        true,
                    ),
                ]
            }
            Self::OpenClaw => {
                let directory = client_directory(home, "OPENCLAW_STATE_DIR", ".openclaw");
                let config = client_file_from_env(home, "OPENCLAW_CONFIG_PATH")
                    .unwrap_or_else(|| directory.join("openclaw.json"));
                vec![
                    removal_recipe(
                        self,
                        config,
                        Format::Json,
                        object([
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
                    ),
                    removal_recipe(
                        self,
                        directory.join("secrets/rewire-token"),
                        Format::Plain,
                        Value::Null,
                        true,
                    ),
                ]
            }
        }
    }
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

fn codex_recipes(home: &Path, base_url: &str, token: &str, model: Option<&str>) -> Vec<Recipe> {
    let mut profile = Map::from_iter([("model_provider".into(), string("rewire"))]);
    if let Some(model) = model {
        profile.insert("model".into(), string(model));
    }
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
            ("profiles", object([("rewire", Value::Object(profile))])),
        ]),
        true,
        "/model_providers/rewire/base_url",
    )]
}

fn opencode_recipes(home: &Path, base_url: &str, token: &str, model: Option<&str>) -> Vec<Recipe> {
    let secret_path = home.join(".config/rewire/secrets/opencode-token");
    let reference = format!("{{file:{}}}", secret_path.to_string_lossy());
    vec![
        provider_recipe(
            Client::OpenCode,
            opencode_config_path(home),
            Format::Json,
            object([(
                "provider",
                object([(
                    "rewire",
                    object([
                        ("name", string("Rewire")),
                        ("npm", string("@ai-sdk/openai-compatible")),
                        (
                            "options",
                            object([("baseURL", string(base_url)), ("apiKey", string(reference))]),
                        ),
                        ("models", opencode_models(model)),
                    ]),
                )]),
            )]),
            false,
            "/provider/rewire/options/baseURL",
        ),
        plain_recipe(Client::OpenCode, secret_path, token),
    ]
}

fn hermes_recipes(home: &Path, base_url: &str, token: &str, model: Option<&str>) -> Vec<Recipe> {
    let directory = client_directory(home, "HERMES_HOME", ".hermes");
    vec![
        provider_recipe(
            Client::Hermes,
            directory.join("config.yaml"),
            Format::Yaml,
            object([(
                "providers",
                object([(
                    "rewire",
                    object_with_optional_model(
                        [
                            ("name", string("Rewire")),
                            ("api", string(base_url)),
                            ("key_env", string("REWIRE_TOKEN")),
                            ("transport", string("chat_completions")),
                        ],
                        "default_model",
                        model,
                    ),
                )]),
            )]),
            false,
            "/providers/rewire/api",
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

fn openclaw_recipes(home: &Path, base_url: &str, token: &str, model: Option<&str>) -> Vec<Recipe> {
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
                                    ("models", openclaw_models(model)),
                                ]),
                            )]),
                        ),
                    ]),
                ),
            ]),
            false,
            "/models/providers/rewire/baseUrl",
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
        provider_endpoint: None,
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
) -> Recipe {
    let mut recipe = structured_recipe(client, path, format, values, sensitive);
    recipe.provider_endpoint = Some(provider_endpoint);
    recipe
}

fn plain_recipe(client: Client, path: PathBuf, token: &str) -> Recipe {
    structured_recipe(client, path, Format::Plain, string(token), true)
}

fn object_with_optional_model(
    entries: impl IntoIterator<Item = (&'static str, Value)>,
    key: &'static str,
    model: Option<&str>,
) -> Value {
    let mut value = object(entries);
    if let (Some(model), Some(map)) = (model, value.as_object_mut()) {
        map.insert(key.into(), string(model));
    }
    value
}

fn opencode_models(model: Option<&str>) -> Value {
    let mut models = Map::new();
    if let Some(model) = model {
        models.insert(model.into(), object([("name", string(model))]));
    }
    Value::Object(models)
}

fn openclaw_models(model: Option<&str>) -> Value {
    Value::Array(
        model
            .map(|model| object([("id", string(model)), ("name", string(model))]))
            .into_iter()
            .collect(),
    )
}

fn opencode_config_path(home: &Path) -> PathBuf {
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
    // Match OpenCode's own globalConfigFile() order and default to its preferred JSONC target.
    ["opencode.jsonc", "opencode.json", "config.json"]
        .into_iter()
        .map(|name| directory.join(name))
        .find(|path| path.exists())
        .unwrap_or_else(|| directory.join("opencode.jsonc"))
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
