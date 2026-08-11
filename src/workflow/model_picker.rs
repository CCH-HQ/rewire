use super::{Palette, optional_prompt};
use crate::{
    Client, DiscoveredModel, DiscoveryFailure, DiscoveryReport, ModelApi, ModelConfig, ModelPreset,
    OpenCodeSdk, discover_models, find_model, popular_models,
};
use anyhow::Result;
use inquire::{Select, Text, ui::RenderConfig};
use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write as _};
use std::time::Duration;

pub(super) const CUSTOM_MODEL_PROMPT: &str = "Custom model ID";
pub(super) const SHOW_ALL_MODELS: &str = "Show all catalog models";
pub(super) const SCAN_MESSAGE: &str = "Scanning OpenAI, Anthropic, and Google model endpoints...";
const SPINNER_FRAMES: [char; 4] = ['|', '/', '-', '\\'];
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedModel {
    pub(super) id: String,
    pub(super) display_name: Option<String>,
    pub(super) sdk: OpenCodeSdk,
    pub(super) models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelChoiceKind {
    AddAll,
    Discovered(DiscoveredModel),
    Preset(ModelPreset),
    ShowAll,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelChoice {
    kind: ModelChoiceKind,
    label: String,
}

impl std::fmt::Display for ModelChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub(super) fn prompt_model(
    clients: &[Client],
    base_url: &str,
    token: &str,
    render_config: &RenderConfig<'static>,
    palette: Palette,
    debug: bool,
) -> Result<Option<SelectedModel>> {
    let target = model_target_label(clients);
    let model_prompt = format!("Choose a model for {target}");
    let report = discover_with_spinner(base_url, token, palette);
    print_discovery_summary(&report, palette, debug);
    let mut expanded = false;

    loop {
        let choices = build_choices(&report.models, expanded, palette);
        let Some(choice) = optional_prompt(
            Select::new(&model_prompt, choices)
                .with_starting_cursor(0)
                .with_page_size(10)
                .with_render_config(*render_config)
                .prompt(),
        )?
        else {
            return Ok(None);
        };
        match choice.kind {
            ModelChoiceKind::AddAll => {
                return prompt_all_models(&report.models, &target, render_config, palette);
            }
            ModelChoiceKind::Discovered(model) => return Ok(Some(select_discovered(model))),
            ModelChoiceKind::Preset(preset) => {
                return Ok(Some(SelectedModel {
                    id: preset.id.to_owned(),
                    display_name: Some(preset.display_name.to_owned()),
                    sdk: preset.sdk,
                    models: Vec::new(),
                }));
            }
            ModelChoiceKind::ShowAll => expanded = true,
            ModelChoiceKind::Custom => {
                return prompt_custom_model(render_config, palette);
            }
        }
    }
}

fn model_target_label(clients: &[Client]) -> String {
    let names = clients
        .iter()
        .copied()
        .filter(|client| client.requires_model())
        .map(|client| match client {
            Client::OpenCode => "OpenCode",
            Client::Hermes => "Hermes",
            Client::OpenClaw => "OpenClaw",
            Client::Claude | Client::Codex => {
                unreachable!("model target labels include only model-aware clients")
            }
        })
        .collect::<Vec<_>>();
    match names.as_slice() {
        [] => "selected clients".to_owned(),
        [name] => (*name).to_owned(),
        [first, second] => format!("{first} and {second}"),
        _ => format!(
            "{}, and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

fn discover_with_spinner(base_url: &str, token: &str, palette: Palette) -> DiscoveryReport {
    std::thread::scope(|scope| {
        let handle = scope.spawn(|| discover_models(base_url, token));
        let terminal = io::stderr().is_terminal();
        let mut stderr = io::stderr().lock();
        if terminal {
            let mut frame = 0;
            while !handle.is_finished() {
                let marker = palette.accent(SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]);
                let _ = write!(stderr, "\r{marker} {SCAN_MESSAGE}");
                let _ = stderr.flush();
                frame += 1;
                std::thread::sleep(SPINNER_INTERVAL);
            }
            // Spaces plus carriage returns avoid depending on ANSI erase-line support.
            let _ = write!(stderr, "\r{:width$}\r", "", width = SCAN_MESSAGE.len() + 2);
            let _ = stderr.flush();
        } else {
            let _ = writeln!(stderr, "{SCAN_MESSAGE}");
        }
        handle.join().unwrap_or_else(|_| discovery_worker_failure())
    })
}

fn discovery_worker_failure() -> DiscoveryReport {
    DiscoveryReport {
        failures: [ModelApi::OpenAi, ModelApi::Anthropic, ModelApi::Google]
            .into_iter()
            .map(|api| DiscoveryFailure {
                api,
                reason: "model scan worker stopped unexpectedly".to_owned(),
            })
            .collect(),
        ..DiscoveryReport::default()
    }
}

fn print_discovery_summary(report: &DiscoveryReport, palette: Palette, debug: bool) {
    let model_count = report.models.len();
    let model_word = if model_count == 1 { "model" } else { "models" };
    eprintln!(
        "Found {} available {model_word} from {} of 3 APIs.",
        palette.success(model_count),
        palette.success(report.successful_api_count())
    );
    for failure in &report.failures {
        eprintln!(
            "{}",
            palette.warning(format!(
                "Warning: {} model scan failed: {}.",
                failure.api, failure.reason
            ))
        );
    }
    if debug {
        for diagnostic in &report.diagnostics {
            eprintln!(
                "{} {}",
                palette.accent("[DEBUG]"),
                palette.muted(format_discovery_diagnostic(diagnostic))
            );
        }
    }
}

fn format_discovery_diagnostic(diagnostic: &crate::DiscoveryDiagnostic) -> String {
    let status = diagnostic.status.map_or_else(
        || "transport-error".to_owned(),
        |status| format!("HTTP {status}"),
    );
    let content_type = diagnostic.content_type.as_deref().unwrap_or("unknown");
    let response_bytes = diagnostic
        .response_bytes
        .map_or_else(|| "unknown".to_owned(), |bytes| bytes.to_string());
    let location = diagnostic
        .location
        .as_deref()
        .map_or_else(String::new, |location| format!("; location={location}"));
    format!(
        "{} GET {} -> {status}; content-type={content_type}; bytes={response_bytes}; attempts={}{location}",
        diagnostic.api, diagnostic.endpoint, diagnostic.attempts
    )
}

fn build_choices(
    discovered: &[DiscoveredModel],
    expanded: bool,
    palette: Palette,
) -> Vec<ModelChoice> {
    let mut choices = Vec::new();
    if !discovered.is_empty() {
        let noun = if discovered.len() == 1 {
            "model"
        } else {
            "models"
        };
        choices.push(ModelChoice {
            kind: ModelChoiceKind::AddAll,
            label: format!(
                "Add all {} available {noun}",
                palette.success(discovered.len()),
            ),
        });
    }
    choices.extend(
        discovered
            .iter()
            .cloned()
            .map(|model| available_model_choice(model, palette)),
    );
    let discovered_ids = discovered
        .iter()
        .map(|model| model.id.as_str())
        .collect::<BTreeSet<_>>();
    let hidden_catalog_models = popular_models()
        .iter()
        .filter(|preset| !discovered_ids.contains(preset.id));
    if expanded {
        choices.extend(hidden_catalog_models.map(|preset| ModelChoice {
            label: format!(
                "{} [{}] ({})",
                preset.display_name, preset.provider, preset.id
            ),
            kind: ModelChoiceKind::Preset(*preset),
        }));
    } else if hidden_catalog_models.count() > 0 {
        choices.push(ModelChoice {
            kind: ModelChoiceKind::ShowAll,
            label: SHOW_ALL_MODELS.to_owned(),
        });
    }
    choices.push(ModelChoice {
        kind: ModelChoiceKind::Custom,
        label: CUSTOM_MODEL_PROMPT.to_owned(),
    });
    choices
}

fn select_discovered(model: DiscoveredModel) -> SelectedModel {
    let preset = find_model(&model.id);
    SelectedModel {
        sdk: preset.map_or_else(|| OpenCodeSdk::infer(Some(&model.id)), |preset| preset.sdk),
        display_name: model
            .display_name
            .or_else(|| preset.map(|preset| preset.display_name.to_owned())),
        id: model.id,
        models: Vec::new(),
    }
}

fn available_model_choice(model: DiscoveredModel, palette: Palette) -> ModelChoice {
    let display_name = model
        .display_name
        .as_deref()
        .or_else(|| find_model(&model.id).map(|preset| preset.display_name))
        .unwrap_or(&model.id);
    ModelChoice {
        label: format!(
            "{} {} ({})",
            display_name,
            palette.success("[AVAILABLE]"),
            model.id
        ),
        kind: ModelChoiceKind::Discovered(model),
    }
}

fn prompt_all_models(
    discovered: &[DiscoveredModel],
    target: &str,
    render_config: &RenderConfig<'static>,
    palette: Palette,
) -> Result<Option<SelectedModel>> {
    let choices = discovered
        .iter()
        .cloned()
        .map(|model| available_model_choice(model, palette))
        .collect();
    let Some(choice) = optional_prompt(
        Select::new(&format!("Choose the default model for {target}"), choices)
            .with_starting_cursor(0)
            .with_page_size(10)
            .with_render_config(*render_config)
            .prompt(),
    )?
    else {
        return Ok(None);
    };
    let ModelChoiceKind::Discovered(primary) = choice.kind else {
        unreachable!("the default-model picker contains only discovered models")
    };
    Ok(Some(select_all_models(discovered, primary)))
}

fn select_all_models(discovered: &[DiscoveredModel], primary: DiscoveredModel) -> SelectedModel {
    // The default remains a separate client selection so choosing it never drops the other
    // discovered entries; every catalog entry retains its own OpenCode wire protocol.
    let mut selected = select_discovered(primary);
    selected.models = discovered.iter().map(model_config).collect();
    selected
}

fn model_config(model: &DiscoveredModel) -> ModelConfig {
    let preset = find_model(&model.id);
    ModelConfig {
        id: model.id.clone(),
        display_name: model
            .display_name
            .clone()
            .or_else(|| preset.map(|preset| preset.display_name.to_owned())),
        sdk: preset.map_or_else(|| OpenCodeSdk::infer(Some(&model.id)), |preset| preset.sdk),
    }
}

fn prompt_custom_model(
    render_config: &RenderConfig<'static>,
    palette: Palette,
) -> Result<Option<SelectedModel>> {
    loop {
        let result = Text::new(CUSTOM_MODEL_PROMPT)
            .with_placeholder("e.g. gpt-5.5")
            .with_render_config(*render_config)
            .prompt();
        let Some(value) = optional_prompt(result)? else {
            return Ok(None);
        };
        let value = value.trim().to_owned();
        match crate::validate_model_id(&value) {
            Ok(()) => {
                return Ok(Some(SelectedModel {
                    sdk: OpenCodeSdk::infer(Some(&value)),
                    id: value,
                    display_name: None,
                    models: Vec::new(),
                }));
            }
            Err(error) => {
                eprintln!("{}", palette.danger(format!("Invalid model ID: {error}")));
            }
        }
    }
}

#[cfg(test)]
#[path = "model_picker/tests.rs"]
mod tests;
