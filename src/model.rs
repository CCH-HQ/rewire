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
    /// JSON pointer for the adapter-owned provider endpoint, when this recipe defines one.
    pub(crate) provider_endpoint: Option<&'static str>,
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
            .field("provider_endpoint", &self.provider_endpoint)
            .field("removal", &self.removal)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
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
