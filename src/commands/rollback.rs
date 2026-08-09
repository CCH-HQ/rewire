use crate::cli::{Output, input};
use anyhow::{Result, anyhow};
use rewire::{available_transactions, rollback};
use std::path::Path;

/// Restore a transaction and report success only after the core integrity checks pass.
pub(super) fn run(
    home: &Path,
    id: Option<&str>,
    output: Output,
    prompt_allowed: bool,
    yes: bool,
    color: bool,
) -> Result<()> {
    let selected = if let Some(id) = id {
        id.to_owned()
    } else {
        let latest = available_transactions(home)?
            .pop()
            .ok_or_else(|| anyhow!("no committed transactions are available to roll back"))?;
        if yes {
            latest
        } else if !prompt_allowed {
            return Err(anyhow!(
                "transaction ID is required outside an interactive terminal; pass an ID or --yes to roll back the latest transaction"
            ));
        } else if input::confirm_latest_rollback(&latest, color)? {
            latest
        } else {
            return output.cancelled("Cancelled. No files were changed.");
        }
    };
    rollback(home, &selected)?;
    output.rollback(&selected)
}
