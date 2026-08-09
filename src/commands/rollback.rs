use crate::cli::Output;
use anyhow::Result;
use rewire::rollback;
use std::path::Path;

/// Restore a transaction and report success only after the core integrity checks pass.
pub(super) fn run(home: &Path, id: &str, output: Output) -> Result<()> {
    rollback(home, id)?;
    output.rollback(id)
}
