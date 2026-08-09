use crate::cli::Cli;
use anyhow::{Result, anyhow};
use rewire::{Client, Input, OpenCodeSdk, detect_clients, read_token, validate_model_name};
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
    let model = cli
        .model
        .take()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty());
    let model_name = cli
        .model_name
        .take()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty());
    if let Some(model_name) = model_name.as_deref() {
        validate_model_name(model_name)?;
    }
    let sdk = cli
        .sdk
        .take()
        .map(|sdk| OpenCodeSdk::parse(&sdk))
        .transpose()?;
    Client::validate_model_configuration(&clients, model.as_deref(), model_name.as_deref(), sdk)?;
    Ok(Input {
        base_url,
        token,
        clients,
        model,
        model_name,
        sdk,
    })
}
