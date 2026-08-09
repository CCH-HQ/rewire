use super::configuration;
use crate::cli::{Cli, Output};
use anyhow::Result;
use rewire::build_plan;
use std::path::Path;

/// Build and render a plan without applying any prepared bytes.
pub(super) fn run(home: &Path, cli: &mut Cli, output: Output) -> Result<()> {
    let input = configuration::input(cli, home)?;
    output.plan(&build_plan(home, &input)?)
}
