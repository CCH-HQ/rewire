use crate::cli::Cli;
use anyhow::{Result, anyhow};
use rewire::{Client, Input, detect_clients, read_token};
use std::path::Path;

/// Consume configuration credentials and selectors exactly once for plan/apply commands.
pub(super) fn input(cli: &mut Cli, home: &Path) -> Result<Input> {
    let base_url = cli
        .baseurl
        .take()
        .ok_or_else(|| anyhow!("--baseurl is required"))?;
    let token = read_token(cli.token_stdin, cli.token.take())?;
    let clients = if let Some(value) = cli.client.take() {
        Client::parse_list(&value)?
    } else {
        let detected = detect_clients(home);
        if detected.is_empty() {
            return Err(anyhow!("no client selected; use --client"));
        }
        detected
    };
    Ok(Input {
        base_url,
        token,
        clients,
        model: cli.model.take().filter(|model| !model.trim().is_empty()),
    })
}
