use crate::cli::{Cli, Output};
use anyhow::{Result, anyhow};
use rewire::{Client, apply_plan, build_remove_plan};
use std::path::Path;

/// Plan or apply transactional removal of fields owned by selected client adapters.
pub(super) fn run(home: &Path, cli: &mut Cli, output: Output) -> Result<()> {
    let clients = Client::parse_list(
        cli.client
            .take()
            .ok_or_else(|| anyhow!("remove requires --client"))?
            .as_str(),
    )?;
    let plan = build_remove_plan(home, &clients)?;
    if cli.execution.dry_run || !cli.execution.yes {
        return output.plan(&plan);
    }
    let transaction = apply_plan(home, &plan)?;
    output.applied(&transaction, &plan)
}
