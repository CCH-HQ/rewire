use crate::cli::Cli;
use anyhow::{Result, anyhow};
use rewire::{
    Client, CompletedInput, OpenCodeSdk, PartialInput, complete_input, read_token,
    validate_base_url, validate_model_name,
};
use std::path::Path;

/// Consume configuration credentials and selectors exactly once for plan/apply commands.
pub(super) fn input(
    cli: &mut Cli,
    home: &Path,
    interactive_terminal: bool,
) -> Result<Option<CompletedInput>> {
    let base_url = cli.baseurl.take();
    let token = if cli.token_stdin || cli.token.is_some() {
        Some(read_token(cli.token_stdin, cli.token.take())?)
    } else {
        None
    };
    let clients = cli
        .client
        .take()
        .map(|value| Client::parse_list(&value))
        .transpose()?;
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
    if let Some(base_url) = base_url.as_deref() {
        validate_base_url(base_url)?;
    }
    let partial = PartialInput {
        base_url,
        token,
        clients,
        model,
        model_name,
        sdk,
    };
    if required_input_is_missing(&partial)
        && (cli.execution.non_interactive || !interactive_terminal)
    {
        return Err(missing_input_error(&partial));
    }
    if interactive_terminal && !cli.execution.non_interactive && guided_input_is_missing(&partial) {
        let completed = complete_input(home, partial, !cli.display.no_color, cli.display.debug)?;
        if completed.is_none() {
            eprintln!("Cancelled. No files were changed.");
        }
        return Ok(completed);
    }

    Ok(Some(complete_without_prompts(partial)?))
}

fn required_input_is_missing(input: &PartialInput) -> bool {
    input.base_url.is_none()
        || input.token.is_none()
        || input.clients.is_none()
        || input.clients.as_ref().is_some_and(|clients| {
            input.model.is_none() && clients.iter().copied().any(Client::requires_model)
        })
}

fn guided_input_is_missing(input: &PartialInput) -> bool {
    if required_input_is_missing(input) {
        return true;
    }
    let clients = input.clients.as_deref().unwrap_or_default();
    (input.sdk.is_none() && clients.contains(&Client::OpenCode))
        || (input.model_name.is_none()
            && (clients.contains(&Client::OpenClaw)
                || (clients.contains(&Client::OpenCode)
                    && input
                        .sdk
                        .is_none_or(|sdk| sdk.native_provider_id().is_none()))))
}

fn complete_without_prompts(input: PartialInput) -> Result<CompletedInput> {
    let clients = input
        .clients
        .expect("required client input is checked before completion");
    Client::validate_model_configuration(
        &clients,
        input.model.as_deref(),
        input.model_name.as_deref(),
        input.sdk,
    )?;
    Ok(CompletedInput {
        input: rewire::Input {
            base_url: input
                .base_url
                .expect("required base URL input is checked before completion"),
            token: input
                .token
                .expect("required token input is checked before completion"),
            clients,
            model: input.model,
            model_name: input.model_name,
            sdk: input.sdk,
        },
        models: Vec::new(),
    })
}

fn missing_input_error(input: &PartialInput) -> anyhow::Error {
    if input.base_url.is_none() {
        anyhow!("--baseurl is required")
    } else if input.token.is_none() {
        anyhow!("token is required")
    } else if input.clients.is_none() {
        anyhow!("no client selected; use --client")
    } else {
        let clients = input.clients.as_deref().unwrap_or_default();
        let required_by = clients
            .iter()
            .copied()
            .filter(|client| client.requires_model())
            .map(Client::name)
            .collect::<Vec<_>>();
        anyhow!(
            "--model is required when configuring {}",
            required_by.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rewire::Secret;

    fn partial(clients: Option<Vec<Client>>, model: Option<&str>) -> PartialInput {
        PartialInput {
            base_url: Some("https://gateway.example".into()),
            token: Some(Secret::new("TOKEN").unwrap()),
            clients,
            model: model.map(str::to_owned),
            model_name: None,
            sdk: None,
        }
    }

    #[test]
    fn only_required_configuration_fields_trigger_completion() {
        assert!(!required_input_is_missing(&partial(
            Some(vec![Client::Claude]),
            None,
        )));
        assert!(required_input_is_missing(&partial(None, None)));
        assert!(required_input_is_missing(&partial(
            Some(vec![Client::OpenCode]),
            None,
        )));
        assert!(!required_input_is_missing(&partial(
            Some(vec![Client::OpenCode]),
            Some("gpt-5.5"),
        )));
    }

    #[test]
    fn guided_completion_matches_configure_specific_prompts() {
        let opencode = partial(Some(vec![Client::OpenCode]), Some("gpt-5.5"));
        assert!(!required_input_is_missing(&opencode));
        assert!(guided_input_is_missing(&opencode));

        let mut native = partial(Some(vec![Client::OpenCode]), Some("gpt-5.5"));
        native.sdk = Some(OpenCodeSdk::OpenAi);
        assert!(!guided_input_is_missing(&native));

        let openclaw = partial(Some(vec![Client::OpenClaw]), Some("gpt-5.5"));
        assert!(guided_input_is_missing(&openclaw));
    }

    #[test]
    fn missing_input_errors_follow_prompt_order() {
        let mut input = PartialInput::default();
        assert_eq!(
            missing_input_error(&input).to_string(),
            "--baseurl is required"
        );

        input.base_url = Some("https://gateway.example".into());
        assert_eq!(missing_input_error(&input).to_string(), "token is required");

        input.token = Some(Secret::new("TOKEN").unwrap());
        assert_eq!(
            missing_input_error(&input).to_string(),
            "no client selected; use --client"
        );

        input.clients = Some(vec![Client::Hermes, Client::OpenClaw]);
        assert_eq!(
            missing_input_error(&input).to_string(),
            "--model is required when configuring hermes, openclaw"
        );
    }
}
