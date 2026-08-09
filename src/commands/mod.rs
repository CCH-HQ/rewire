mod apply;
mod backup;
mod completions;
mod configuration;
mod configure;
mod doctor;
mod plan;
mod remove;
mod rollback;

use crate::cli::{BackupCommand, Cli, Command, Output};
use anyhow::Result;
use rewire::home_from_override;
use std::io::{self, IsTerminal};

/// Route a parsed invocation to one command module and the shared output boundary.
///
/// # Errors
///
/// Returns an error from input validation, command execution, serialization, or filesystem I/O.
pub(crate) fn run(mut cli: Cli) -> Result<()> {
    let home = home_from_override(cli.home.as_deref());
    let output = Output::stdout(cli.display.json, !cli.display.no_color);
    let interactive_terminal = io::stdin().is_terminal() && io::stdout().is_terminal();
    let open_default_workflow = cli.command.is_none()
        && cli.baseurl.is_none()
        && !cli.execution.non_interactive
        && interactive_terminal;

    match cli.command.take() {
        Some(Command::Configure) => configure::run(&home, &cli, interactive_terminal),
        Some(Command::Plan) => plan::run(&home, &mut cli, output),
        Some(Command::Doctor) => doctor::run(&home, output),
        Some(Command::Rollback { id }) => rollback::run(&home, &id, output),
        Some(Command::Remove) => remove::run(&home, &mut cli, output),
        Some(Command::Completions { shell }) => {
            completions::run(shell);
            Ok(())
        }
        Some(Command::Backup {
            command: BackupCommand::List,
        }) => backup::run(&home, output),
        None if open_default_workflow => configure::run(&home, &cli, interactive_terminal),
        None => apply::run(&home, &mut cli, output),
    }
}
