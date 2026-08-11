use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Claude,
    Codex,
    OpenCode,
    Hermes,
    OpenClaw,
}

impl Client {
    /// Parse the stable comma-separated CLI contract and remove duplicate selections.
    ///
    /// # Errors
    ///
    /// Returns an error when the list is empty or contains an unsupported client name.
    pub fn parse_list(input: &str) -> Result<Vec<Self>> {
        let mut clients = Vec::new();
        for raw in input.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let client = match raw.to_ascii_lowercase().as_str() {
                "claude" | "claude-code" => Self::Claude,
                "codex" => Self::Codex,
                "opencode" | "open-code" => Self::OpenCode,
                "hermes" | "hermes-agent" => Self::Hermes,
                "openclaw" => Self::OpenClaw,
                _ => return Err(anyhow!("unknown client: {raw}")),
            };
            if !clients.contains(&client) {
                clients.push(client);
            }
        }
        if clients.is_empty() {
            return Err(anyhow!("client list is empty"));
        }
        Ok(clients)
    }
    /// Return the stable lowercase identifier used by the CLI and transaction journal.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Hermes => "hermes",
            Self::OpenClaw => "openclaw",
        }
    }

    /// Whether this client needs a selected model in addition to a configured provider.
    #[must_use]
    pub const fn requires_model(self) -> bool {
        matches!(self, Self::OpenCode | Self::Hermes | Self::OpenClaw)
    }

    /// Check the shared raw model-ID contract before an adapter formats it for its client.
    ///
    /// # Errors
    ///
    /// Returns an error when a selected client requires a model or when the supplied ID would
    /// produce an ambiguous Rewire provider reference.
    pub fn validate_model_selection(clients: &[Self], model: Option<&str>) -> Result<()> {
        Self::validate_model_configuration(
            clients,
            model,
            None,
            clients
                .contains(&Self::OpenCode)
                .then_some(OpenCodeSdk::OpenAiCompatible),
        )
    }

    /// Validate the complete model contract shared by CLI, workflow, and adapters.
    ///
    /// # Errors
    ///
    /// Returns an error when a required model is missing, an ID/display name is malformed, or
    /// OpenCode-only options are supplied to another client.
    pub fn validate_model_configuration(
        clients: &[Self],
        model: Option<&str>,
        model_name: Option<&str>,
        sdk: Option<OpenCodeSdk>,
    ) -> Result<()> {
        let required_by = clients
            .iter()
            .copied()
            .filter(|client| client.requires_model())
            .map(Self::name)
            .collect::<Vec<_>>();
        if !required_by.is_empty() && model.is_none() {
            return Err(anyhow!(
                "--model is required when configuring {}",
                required_by.join(", ")
            ));
        }
        if let Some(model) = model {
            validate_model_id(model)?;
        }
        if model_name.is_some()
            && !clients
                .iter()
                .any(|client| matches!(client, Self::OpenCode | Self::OpenClaw))
        {
            return Err(anyhow!(
                "--model-name is only valid when configuring opencode or openclaw"
            ));
        }
        if let Some(model_name) = model_name {
            validate_model_name(model_name)?;
        }
        if sdk.is_some() && !clients.contains(&Self::OpenCode) {
            return Err(anyhow!("--sdk is only valid when configuring opencode"));
        }
        Ok(())
    }
}

/// Provider protocol used by the `OpenCode` adapter.
///
/// `OpenAI` and `Anthropic` can reuse `OpenCode`'s native providers for one-model configuration.
/// Full discovered catalogs use Rewire-managed provider IDs so each protocol retains its own AI
/// SDK package, including Google and OpenAI-compatible families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenCodeSdk {
    OpenAi,
    Anthropic,
    Google,
    OpenAiCompatible,
}

impl OpenCodeSdk {
    /// Parse friendly names and the package names accepted by the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is not one of the supported SDK aliases.
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        match value.to_ascii_lowercase().as_str() {
            "openai" | "@ai-sdk/openai" => Ok(Self::OpenAi),
            "anthropic" | "@ai-sdk/anthropic" => Ok(Self::Anthropic),
            "google" | "gemini" | "@ai-sdk/google" => Ok(Self::Google),
            "openai-compatible"
            | "openai_compatible"
            | "compatible"
            | "@ai-sdk/openai-compatible" => Ok(Self::OpenAiCompatible),
            _ => Err(anyhow!(
                "unknown OpenCode SDK `{value}`; choose openai, anthropic, google, or openai-compatible"
            )),
        }
    }

    #[must_use]
    pub const fn npm(self) -> &'static str {
        match self {
            Self::OpenAi => "@ai-sdk/openai",
            Self::Anthropic => "@ai-sdk/anthropic",
            Self::Google => "@ai-sdk/google",
            Self::OpenAiCompatible => "@ai-sdk/openai-compatible",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "openai (native provider; managed models)",
            Self::Anthropic => "anthropic (native provider; managed models)",
            Self::Google => "google (custom Rewire provider; @ai-sdk/google)",
            Self::OpenAiCompatible => {
                "openai-compatible (custom Rewire provider; @ai-sdk/openai-compatible)"
            }
        }
    }

    #[must_use]
    pub const fn choices() -> [Self; 4] {
        [
            Self::OpenAi,
            Self::Anthropic,
            Self::Google,
            Self::OpenAiCompatible,
        ]
    }

    /// Return the native `OpenCode` provider whose model catalog is managed by `OpenCode`.
    #[must_use]
    pub const fn native_provider_id(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("openai"),
            Self::Anthropic => Some("anthropic"),
            Self::Google | Self::OpenAiCompatible => None,
        }
    }

    /// Infer a sensible package for common model families while keeping an explicit
    /// SDK override available for gateways that expose a nonstandard model ID.
    #[must_use]
    pub fn infer(model: Option<&str>) -> Self {
        let model = model.unwrap_or_default().to_ascii_lowercase();
        let family_id = model.rsplit('/').next().unwrap_or(&model);
        if model.contains("claude") {
            Self::Anthropic
        } else if model.contains("gemini") {
            Self::Google
        } else if family_id.starts_with("gpt-")
            || family_id.starts_with("o1")
            || family_id.starts_with("o3")
            || family_id.starts_with("codex-")
        {
            Self::OpenAi
        } else {
            Self::OpenAiCompatible
        }
    }
}

impl std::fmt::Display for OpenCodeSdk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Validate a human-facing catalog name without conflating it with the API ID.
///
/// # Errors
///
/// Returns an error when the display name is empty, padded, or contains control characters.
pub fn validate_model_name(name: &str) -> Result<()> {
    if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
        return Err(anyhow!(
            "--model-name must be non-empty and must not contain whitespace padding or control characters"
        ));
    }
    Ok(())
}

/// Validate the provider-native model ID accepted by the single CLI input.
///
/// Adapters add their own `rewire/` reference syntax where their client requires it. Keeping the
/// raw ID unprefixed prevents accidental values such as `rewire/rewire/gpt-4.1-mini`.
///
/// # Errors
///
/// Returns an error for empty, whitespace-padded, control-character, or Rewire-prefixed IDs.
pub fn validate_model_id(model: &str) -> Result<()> {
    if model.is_empty()
        || model.trim() != model
        || model.chars().any(char::is_control)
        || model.starts_with("rewire/")
    {
        return Err(anyhow!(
            "--model must be a non-empty provider-native ID without whitespace padding or a rewire/ prefix"
        ));
    }
    Ok(())
}
impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Json,
    Toml,
    Yaml,
    Dotenv,
    Plain,
}

#[derive(Clone)]
pub struct Recipe {
    pub client: Client,
    pub path: PathBuf,
    pub format: Format,
    pub values: Value,
    /// Marks files that contain a credential and therefore require private permissions.
    pub sensitive: bool,
    /// Existing/requested JSON pointer pairs for adapter-owned provider endpoints.
    pub(crate) provider_endpoints: Vec<(&'static str, &'static str)>,
    /// JSON pointer for the client-native selected model written by this recipe.
    pub(crate) selected_model: Option<&'static str>,
    /// Selection fields removed only while they still refer to the Rewire provider.
    pub(crate) conditional_removals: Vec<ConditionalRemoval>,
    /// Removal recipes delete the named owned fields instead of merging values.
    pub(crate) removal: bool,
}

impl std::fmt::Debug for Recipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recipe")
            .field("client", &self.client)
            .field("path", &self.path)
            .field("format", &self.format)
            .field("values", &"[REDACTED]")
            .field("sensitive", &self.sensitive)
            .field("provider_endpoints", &self.provider_endpoints)
            .field("selected_model", &self.selected_model)
            .field("conditional_removals", &self.conditional_removals)
            .field("removal", &self.removal)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConditionalRemoval {
    pub(crate) removal_pointer: &'static str,
    /// Disjunctive normal form: any group may match, and every predicate in that group must match.
    pub(crate) alternatives: Vec<Vec<RemovalPredicate>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RemovalPredicate {
    pub(crate) pointer: &'static str,
    pub(crate) expected: String,
    pub(crate) prefix: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ModelConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<String>,
    pub clients: Vec<Client>,
    pub changes: Vec<PlannedChange>,
    pub conflicts: Vec<Conflict>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub(crate) prepared: Vec<PreparedChange>,
}

#[derive(Clone)]
pub(crate) struct PreparedChange {
    pub client: Client,
    pub path: PathBuf,
    pub action: Action,
    pub before: FileSnapshot,
    pub after: Vec<u8>,
    pub after_sha256: String,
    pub after_mode: Option<u32>,
    pub recipe: Recipe,
}

impl std::fmt::Debug for PreparedChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedChange")
            .field("client", &self.client)
            .field("path", &self.path)
            .field("action", &self.action)
            .field("before", &self.before)
            .field("after", &"[REDACTED]")
            .field("after_sha256", &self.after_sha256)
            .field("after_mode", &self.after_mode)
            .field("recipe", &self.recipe)
            .finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct FileSnapshot {
    pub exists: bool,
    pub bytes: Vec<u8>,
    pub sha256: Option<String>,
    pub mode: Option<u32>,
}

impl std::fmt::Debug for FileSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSnapshot")
            .field("exists", &self.exists)
            .field("bytes", &"[REDACTED]")
            .field("sha256", &self.sha256)
            .field("mode", &self.mode)
            .finish()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedChange {
    pub client: Client,
    pub path: PathBuf,
    pub action: Action,
    pub original_sha256: Option<String>,
    pub resulting_sha256: String,
    pub diff: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Merge,
    Delete,
    Noop,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub client: Client,
    pub path: PathBuf,
    pub reason: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub home: PathBuf,
    pub detected: Vec<Client>,
    pub clients: Vec<ClientDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientDiagnostic {
    pub client: Client,
    pub config_path: PathBuf,
    pub configuration_detected: bool,
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Names only. Environment values may contain credentials and never enter the report.
    pub environment: Vec<String>,
}

#[derive(Debug, Error)]
pub enum RewireError {
    #[error("token is required")]
    MissingToken,
    #[error("base URL must be an absolute http(s) URL without credentials, query, or fragment")]
    InvalidUrl,
    #[error("configuration path is a symbolic link: {0}")]
    Symlink(PathBuf),
    #[error("configuration path is outside home: {0}")]
    UnsafePath(PathBuf),
}
#[derive(Debug, Clone)]
pub struct Input {
    pub base_url: String,
    pub token: Secret,
    pub clients: Vec<Client>,
    pub model: Option<String>,
    pub model_name: Option<String>,
    pub sdk: Option<OpenCodeSdk>,
}

/// One provider-native model entry written when the workflow adds the discovered catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub display_name: Option<String>,
    pub sdk: OpenCodeSdk,
}
#[derive(Clone)]
pub struct Secret(String);
impl Secret {
    /// Reject empty credentials at the boundary and retain ownership for zeroization on drop.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied credential is empty.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(RewireError::MissingToken.into());
        }
        Ok(Self(value))
    }
    /// Borrow the credential only at the adapter boundary that needs to write it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([REDACTED_TOKEN])")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub entries: Vec<TransactionEntry>,
    pub committed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEntry {
    pub client: Client,
    pub path: PathBuf,
    pub before_exists: bool,
    pub before_sha256: Option<String>,
    pub mode: Option<u32>,
    pub after_sha256: String,
    #[serde(default = "default_true")]
    pub after_exists: bool,
    pub backup: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_backup: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,
}

const fn default_true() -> bool {
    true
}
