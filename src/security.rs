use crate::model::{RewireError, Secret};
use anyhow::Result;
use directories::BaseDirs;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Normalize and validate a compatible HTTP(S) endpoint.
///
/// # Errors
///
/// Returns an error when the URL has an unsupported scheme or no host component.
pub fn validate_base_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let parsed = url::Url::parse(trimmed).map_err(|_| RewireError::InvalidUrl)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(RewireError::InvalidUrl.into());
    }
    Ok(trimmed.trim_end_matches('/').to_owned())
}
/// Replace complete credential occurrences before text reaches diagnostics or serialized plans.
#[must_use]
pub fn redact(input: &str, token: &str) -> String {
    if token.is_empty() {
        input.to_owned()
    } else {
        input.replace(token, "[REDACTED_TOKEN]")
    }
}
/// Return the lowercase SHA-256 digest used by planning and transaction integrity checks.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    // sha2 0.11 returns an opaque digest array, so encode bytes explicitly for a stable hash string.
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

/// Reject paths outside the selected home and paths redirected through symlink ancestors.
///
/// # Errors
///
/// Returns an error when the target escapes `home` or any existing path component is a symlink.
pub fn ensure_safe_path(home: &Path, path: &Path) -> Result<()> {
    // Check both lexical containment and every existing ancestor so a symlink cannot redirect writes.
    let canonical_home = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    let normalized = path.strip_prefix(home).map_or_else(
        |_| path.to_path_buf(),
        |relative| canonical_home.join(relative),
    );
    if !normalized.starts_with(&canonical_home) || has_symlink_ancestor(home, path) {
        return Err(RewireError::UnsafePath(path.to_path_buf()).into());
    }
    Ok(())
}

fn has_symlink_ancestor(home: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(home) else {
        return true;
    };
    let mut current = home.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if current.is_symlink() {
            return true;
        }
    }
    false
}
/// Resolve the effective home directory, preferring an explicit isolated override.
#[must_use]
pub fn home_from_override(home: Option<&Path>) -> PathBuf {
    home.map(Path::to_path_buf)
        .or_else(|| BaseDirs::new().map(|d| d.home_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}
/// Read a token from standard input or a supplied CLI/environment value.
///
/// # Errors
///
/// Returns an error when standard input cannot be read or the resulting credential is empty.
pub fn read_token(stdin: bool, value: Option<String>) -> Result<Secret> {
    if stdin {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        Secret::new(input.trim_end_matches(['\r', '\n']).to_owned())
    } else {
        Secret::new(value.unwrap_or_default())
    }
}
/// Return the platform-appropriate transaction directory for the effective home.
#[must_use]
pub fn transaction_root(home: &Path) -> PathBuf {
    // A fixture `--home` must stay self-contained; only the real home honors process-wide XDG state.
    let is_real_home = BaseDirs::new().is_some_and(|base_dirs| base_dirs.home_dir() == home);
    if is_real_home
        && let Some(state_home) = env::var_os("XDG_STATE_HOME")
        && !state_home.is_empty()
    {
        return PathBuf::from(state_home).join("rewire/transactions");
    }
    if cfg!(target_os = "windows") {
        home.join("AppData/Local/Rewire/transactions")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Rewire/transactions")
    } else {
        home.join(".local/state/rewire/transactions")
    }
}
/// Serialize a value as deterministic pretty JSON with a trailing newline.
///
/// # Errors
///
/// Returns an error when `value` cannot be represented as JSON.
pub fn stable_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)? + "\n")
}
