use crate::cli::Cli;
use anyhow::{Result, anyhow};
use std::path::Path;

/// Run the guided terminal workflow after enforcing its interaction boundary.
pub(super) fn run(home: &Path, cli: &Cli, interactive_terminal: bool) -> Result<()> {
    if cli.execution.non_interactive {
        return Err(anyhow!("configure conflicts with --non-interactive"));
    }
    if !interactive_terminal {
        return Err(anyhow!(
            "interactive workflow requires terminal stdin and stdout"
        ));
    }
    rewire::run_workflow_with_debug(home, !cli.display.no_color, cli.display.debug)
}
