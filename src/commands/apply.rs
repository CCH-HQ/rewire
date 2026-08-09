use super::configuration;
use crate::cli::{Cli, Output};
use anyhow::Result;
use rewire::{apply_plan, build_plan};
use std::path::Path;

/// Plan a root invocation and either review it or commit it when `--yes` is present.
pub(super) fn run(home: &Path, cli: &mut Cli, output: Output) -> Result<()> {
    let input = configuration::input(cli, home)?;
    let plan = build_plan(home, &input)?;
    if cli.execution.dry_run || !cli.execution.yes {
        return output.plan(&plan);
    }
    let transaction = apply_plan(home, &plan)?;
    output.applied(&transaction, &plan)
}
