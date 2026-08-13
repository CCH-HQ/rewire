use super::configuration;
use crate::cli::{Cli, Output, input};
use anyhow::Result;
use rewire::{apply_plan, build_plan_with_catalog};
use std::path::Path;

/// Plan a root invocation and either review it or commit it when `--yes` is present.
pub(super) fn run(
    home: &Path,
    cli: &mut Cli,
    output: Output,
    interactive_terminal: bool,
) -> Result<()> {
    let Some(completed) = configuration::input(cli, home, interactive_terminal)? else {
        return Ok(());
    };
    let plan = build_plan_with_catalog(home, &completed.input, &completed.models)?;
    if cli.execution.dry_run {
        return output.plan(&plan);
    }
    if !cli.execution.yes {
        output.plan(&plan)?;
        if !interactive_terminal || cli.display.json {
            return Ok(());
        }
        if !input::confirm_plan(!cli.display.no_color)? {
            return output.cancelled("Plan was not applied.");
        }
    }
    let transaction = apply_plan(home, &plan)?;
    output.applied(&transaction, &plan)
}
