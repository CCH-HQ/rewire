use crate::clients::CLIENTS;
use crate::format::{merge_recipe, parse_structured};
use crate::model::{
    Action, Client, Conflict, FileSnapshot, Input, Plan, PlannedChange, PreparedChange, Recipe,
};
use crate::security::{ensure_safe_path, hash_bytes, redact, validate_base_url};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

struct ClientPlan {
    change: PlannedChange,
    prepared: PreparedChange,
    conflicts: Vec<Conflict>,
}

/// Detect supported clients that already have a configuration file under `home`.
#[must_use]
pub fn detect_clients(home: &Path) -> Vec<Client> {
    CLIENTS
        .iter()
        .copied()
        .filter(|client| {
            client
                .recipes(home, "https://example.invalid", "TOKEN", None)
                .first()
                .is_some_and(|recipe| recipe.path.exists())
        })
        .collect()
}

/// Build a complete, redacted plan without modifying client files.
///
/// # Errors
///
/// Returns an error when the base URL is invalid. Per-client filesystem and parse failures are
/// retained as blocking conflicts so the caller can present the entire review in one pass.
pub fn build_plan(home: &Path, input: &Input) -> Result<Plan> {
    let base_url = validate_base_url(&input.base_url)?;
    let mut changes = Vec::new();
    let mut conflicts = Vec::new();
    let mut warnings = Vec::new();
    let mut prepared = Vec::new();
    for &client in &input.clients {
        // Plan every candidate before writing anything, preventing partial application on parse errors.
        for recipe in client.recipes(
            home,
            &base_url,
            input.token.expose(),
            input.model.as_deref(),
        ) {
            match plan_recipe(home, recipe, input.token.expose()) {
                Ok(client_plan) => {
                    changes.push(client_plan.change);
                    prepared.push(client_plan.prepared);
                    conflicts.extend(client_plan.conflicts);
                }
                Err(conflict) => conflicts.push(conflict),
            }
        }
        warnings.extend(client_warnings(
            client,
            input.model.as_deref(),
            &base_url,
            input.token.expose(),
        ));
    }
    Ok(Plan {
        base_url,
        model: input.model.clone(),
        clients: input.clients.clone(),
        changes,
        conflicts,
        warnings,
        prepared,
    })
}

/// Build a transactional plan that removes only fields owned by the selected adapters.
///
/// # Errors
///
/// Filesystem, path, and parse failures are retained as blocking conflicts in the plan.
pub fn build_remove_plan(home: &Path, clients: &[Client]) -> Result<Plan> {
    let mut changes = Vec::new();
    let mut conflicts = Vec::new();
    let mut prepared = Vec::new();
    for &client in clients {
        for recipe in client.removal_recipes(home) {
            match plan_recipe(home, recipe, "") {
                Ok(client_plan) => {
                    changes.push(client_plan.change);
                    prepared.push(client_plan.prepared);
                    conflicts.extend(client_plan.conflicts);
                }
                Err(conflict) => conflicts.push(conflict),
            }
        }
    }
    Ok(Plan {
        base_url: String::new(),
        model: None,
        clients: clients.to_vec(),
        changes,
        conflicts,
        warnings: Vec::new(),
        prepared,
    })
}

fn plan_recipe(
    home: &Path,
    recipe: Recipe,
    token: &str,
) -> std::result::Result<ClientPlan, Conflict> {
    let client = recipe.client;
    validate_target(client, &recipe.path, home, token)?;
    let metadata = recipe.path.metadata().ok();
    // Existence remains separate from metadata readability so inaccessible files become conflicts.
    let exists = recipe.path.exists();
    let existing = if exists {
        fs::read(&recipe.path)
            .with_context(|| format!("read {}", recipe.path.display()))
            .map_err(|error| blocking_conflict(client, &recipe.path, error.to_string(), token))?
    } else {
        Vec::new()
    };
    let before = FileSnapshot {
        exists,
        sha256: exists.then(|| hash_bytes(&existing)),
        bytes: existing.clone(),
        mode: file_mode(metadata.as_ref()),
    };
    let conflicts = provider_endpoint_conflicts(&recipe, exists.then_some(existing.as_slice()))
        .map_err(|error| blocking_conflict(client, &recipe.path, error.to_string(), token))?;
    let merged = merge_recipe(&recipe, exists.then_some(existing.as_slice()))
        .map_err(|error| blocking_conflict(client, &recipe.path, error.to_string(), token))?;
    let action = action_for(&before, &merged, &recipe);
    let resulting_sha256 = hash_bytes(&merged);
    let diff = redact(
        &redacted_diff(before.exists.then_some(before.bytes.as_slice()), &merged),
        token,
    );
    let after_mode = output_mode(&before, recipe.sensitive);
    Ok(ClientPlan {
        change: PlannedChange {
            client,
            path: recipe.path.clone(),
            action: action.clone(),
            original_sha256: before.sha256.clone(),
            resulting_sha256: resulting_sha256.clone(),
            diff,
        },
        prepared: PreparedChange {
            client,
            path: recipe.path.clone(),
            action,
            before,
            after: merged,
            after_sha256: resulting_sha256,
            after_mode,
            recipe,
        },
        conflicts,
    })
}

fn provider_endpoint_conflicts(recipe: &Recipe, existing: Option<&[u8]>) -> Result<Vec<Conflict>> {
    let (Some(pointer), Some(existing)) = (recipe.provider_endpoint, existing) else {
        return Ok(Vec::new());
    };
    let Some(root) = parse_structured(recipe.format, existing)? else {
        return Ok(Vec::new());
    };
    let Some(current) = root.pointer(pointer) else {
        return Ok(Vec::new());
    };
    let requested = recipe
        .values
        .pointer(pointer)
        .and_then(Value::as_str)
        .expect("provider endpoint recipes contain a string URL");
    let same_endpoint = current
        .as_str()
        .and_then(|value| validate_base_url(value).ok())
        .is_some_and(|value| value == requested);
    if same_endpoint {
        return Ok(Vec::new());
    }
    Ok(vec![Conflict {
        client: recipe.client,
        path: recipe.path.clone(),
        reason: "provider `rewire` already uses a different base URL; applying will replace it"
            .into(),
        blocking: false,
    }])
}

fn client_warnings(
    client: Client,
    model: Option<&str>,
    base_url: &str,
    token: &str,
) -> Vec<String> {
    let mut warnings = client.environment_warnings(base_url, token);
    if client == Client::Codex {
        warnings.push(
            "Codex auth remains untouched; activate the isolated profile with --profile rewire"
                .into(),
        );
    }
    if model.is_none() && matches!(client, Client::OpenCode | Client::OpenClaw) {
        warnings.push(format!(
            "{client} provider added without a model catalog; supply --model or select a model separately"
        ));
    }
    warnings
}

fn validate_target(
    client: Client,
    path: &Path,
    home: &Path,
    token: &str,
) -> std::result::Result<(), Conflict> {
    if path.is_symlink() {
        return Err(blocking_conflict(
            client,
            path,
            "target is a symlink",
            token,
        ));
    }
    ensure_safe_path(home, path)
        .map_err(|error| blocking_conflict(client, path, error.to_string(), token))?;
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.permissions().readonly())
    {
        return Err(blocking_conflict(
            client,
            path,
            "target is read-only",
            token,
        ));
    }
    Ok(())
}

fn blocking_conflict(
    client: Client,
    path: &Path,
    reason: impl AsRef<str>,
    token: &str,
) -> Conflict {
    Conflict {
        client,
        path: path.to_path_buf(),
        reason: redact(reason.as_ref(), token),
        blocking: true,
    }
}

fn action_for(before: &FileSnapshot, after: &[u8], recipe: &Recipe) -> Action {
    if !before.exists {
        if recipe.removal {
            Action::Noop
        } else {
            Action::Create
        }
    } else if recipe.removal && recipe.format == crate::model::Format::Plain {
        Action::Delete
    } else if before.bytes == after {
        Action::Noop
    } else {
        Action::Merge
    }
}

fn redacted_diff(before: Option<&[u8]>, after: &[u8]) -> String {
    let before = before.map(String::from_utf8_lossy).unwrap_or_default();
    let after = String::from_utf8_lossy(after);
    if before == after {
        String::new()
    } else {
        let mut diff = String::from("--- before\n+++ after\n");
        for line in after.lines().take(80) {
            writeln!(diff, "+{line}").expect("writing to a String cannot fail");
        }
        diff
    }
}

fn file_mode(metadata: Option<&std::fs::Metadata>) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.map(|metadata| metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn output_mode(before: &FileSnapshot, sensitive: bool) -> Option<u32> {
    #[cfg(unix)]
    {
        if sensitive { Some(0o600) } else { before.mode }
    }
    #[cfg(not(unix))]
    {
        let _ = sensitive;
        before.mode
    }
}
