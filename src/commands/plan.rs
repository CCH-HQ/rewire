use super::configuration;
use crate::cli::{Cli, Output};
use anyhow::Result;
use rewire::build_plan_with_catalog;
use std::path::Path;

/// Build and render a plan without applying any prepared bytes.
pub(super) fn run(
    home: &Path,
    cli: &mut Cli,
    output: Output,
    interactive_terminal: bool,
) -> Result<()> {
    let Some(completed) = configuration::input(cli, home, interactive_terminal)? else {
        return Ok(());
    };
    output.plan(&build_plan_with_catalog(
        home,
        &completed.input,
        &completed.models,
    )?)
}
