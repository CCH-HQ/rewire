use crate::cli::Cli;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::io;

/// Generate completions directly from Clap's command tree to keep every option synchronized.
pub(super) fn run(shell: Shell) {
    generate(shell, &mut Cli::command(), "rewire", &mut io::stdout());
}
