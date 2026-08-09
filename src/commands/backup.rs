use crate::cli::Output;
use anyhow::Result;
use rewire::available_transactions;
use std::path::Path;

/// List transaction directories without exposing journal internals.
pub(super) fn run(home: &Path, output: Output) -> Result<()> {
    let ids = available_transactions(home)?;
    output.backups(&ids)
}
