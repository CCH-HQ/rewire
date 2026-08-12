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
        && !has_configuration_input(&cli)
        && !cli.execution.non_interactive
        && interactive_terminal;

    match cli.command.take() {
        Some(Command::Configure) => configure::run(&home, &cli, interactive_terminal),
        Some(Command::Plan) => plan::run(&home, &mut cli, output, interactive_terminal),
        Some(Command::Doctor) => doctor::run(&home, output),
        Some(Command::Rollback { id }) => rollback::run(
            &home,
            id.as_deref(),
            output,
            interactive_terminal && !cli.execution.non_interactive && !cli.display.json,
            cli.execution.yes,
            !cli.display.no_color,
        ),
        Some(Command::Remove) => remove::run(&home, &mut cli, output),
        Some(Command::Completions { shell }) => {
            completions::run(shell);
            Ok(())
        }
        Some(Command::Backup {
            command: BackupCommand::List,
        }) => backup::run(&home, output),
        None if open_default_workflow => configure::run(&home, &cli, interactive_terminal),
        None => apply::run(&home, &mut cli, output, interactive_terminal),
    }
}

/// A root invocation with any supplied configuration value must preserve it and prompt only for
/// the remaining fields. The full workflow is reserved for a genuinely blank configuration form.
fn has_configuration_input(cli: &Cli) -> bool {
    cli.baseurl.is_some()
        || cli.token.is_some()
        || cli.token_stdin
        || cli.client.is_some()
        || cli.model.is_some()
        || cli.model_name.is_some()
        || cli.sdk.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn partial_root_configuration_does_not_get_replaced_by_full_workflow() {
        let blank = Cli::try_parse_from(["rewire"]).unwrap();
        assert!(!has_configuration_input(&blank));

        for arguments in [
            vec!["rewire", "--baseurl", "https://gateway.example"],
            vec!["rewire", "--token", "TOKEN"],
            vec!["rewire", "--token-stdin"],
            vec!["rewire", "--client", "claude"],
            vec!["rewire", "--model", "gpt-5.5"],
            vec!["rewire", "--model-name", "GPT-5.5"],
            vec!["rewire", "--sdk", "openai"],
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            assert!(has_configuration_input(&cli));
        }
    }
}
