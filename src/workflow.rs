use crate::{
    Action, Client, Input, OpenCodeSdk, Plan, apply_plan, build_plan, detect_clients,
    validate_base_url, validate_model_name,
};
use anstyle::{AnsiColor, Style};
use anyhow::Result;
use inquire::{
    InquireError, MultiSelect, Password, PasswordDisplayMode, Select, Text, ui::RenderConfig,
};
use std::path::Path;

const CLIENTS: [Client; 5] = [
    Client::Claude,
    Client::Codex,
    Client::OpenCode,
    Client::Hermes,
    Client::OpenClaw,
];
const WORKFLOW_TITLE: &str = "Rewire configuration workflow";
const WORKFLOW_HELP: &str = "Select clients, review numbered changes, then confirm once.";
const CLIENT_PROMPT: &str = "Choose one or more clients";
const BASE_URL_PROMPT: &str = "Compatible API base URL";
const TOKEN_PROMPT: &str = "API token";
const TOKEN_DISPLAY_MODE: PasswordDisplayMode = PasswordDisplayMode::Masked;
const MODEL_PROMPT: &str = "Model ID";
const MODEL_NAME_PROMPT: &str = "Model display name (optional; Enter uses the ID)";
const SDK_PROMPT: &str = "OpenCode AI SDK";
const REVIEW_PROMPT: &str = "Confirm the numbered plan";
const BLOCKED_PROMPT: &str = "Resolve blocking items before applying";
const ACCENT: Style = AnsiColor::Cyan.on_default().bold();
const SUCCESS: Style = AnsiColor::Green.on_default().bold();
const WARNING: Style = AnsiColor::Yellow.on_default().bold();
const DANGER: Style = AnsiColor::Red.on_default().bold();
const MUTED: Style = AnsiColor::BrightBlack.on_default();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewAction {
    Apply,
    Finish,
    Edit,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelNameChoice {
    Cancel,
    Omit,
    Value(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReviewSummary {
    updates: usize,
    unchanged: usize,
    conflicts: usize,
    blocking: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanMarker {
    Create,
    Update,
    Delete,
    Blocked,
    Review,
}

impl PlanMarker {
    fn label(self) -> &'static str {
        match self {
            Self::Create => "CREATE",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Blocked => "BLOCKED",
            Self::Review => "REVIEW",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Create => SUCCESS,
            Self::Update => ACCENT,
            Self::Delete | Self::Review => WARNING,
            Self::Blocked => DANGER,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumberedPlanItem {
    number: usize,
    marker: PlanMarker,
    client: Client,
    path: std::path::PathBuf,
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    enabled: bool,
}

impl Palette {
    fn new(requested: bool) -> Self {
        Self {
            enabled: requested && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn paint(self, style: Style, value: impl std::fmt::Display) -> String {
        if self.enabled {
            format!("{style}{value}{style:#}")
        } else {
            value.to_string()
        }
    }

    fn accent(self, value: impl std::fmt::Display) -> String {
        self.paint(ACCENT, value)
    }

    fn success(self, value: impl std::fmt::Display) -> String {
        self.paint(SUCCESS, value)
    }

    fn warning(self, value: impl std::fmt::Display) -> String {
        self.paint(WARNING, value)
    }

    fn danger(self, value: impl std::fmt::Display) -> String {
        self.paint(DANGER, value)
    }

    fn muted(self, value: impl std::fmt::Display) -> String {
        self.paint(MUTED, value)
    }
}

/// Run a prompt workflow while planning and writes remain in the shared core.
///
/// # Errors
///
/// Returns an error when prompting, planning, applying, or verification fails.
pub fn run(home: &Path, color: bool) -> Result<()> {
    let palette = Palette::new(color);
    let render_config = render_config(palette.enabled);
    eprintln!("{}", palette.accent(WORKFLOW_TITLE));
    eprintln!("{}", palette.muted(WORKFLOW_HELP));

    'configure: loop {
        let Some(input) = collect_input(&render_config, palette, home)? else {
            eprintln!("{}", palette.warning("Cancelled. No files were changed."));
            return Ok(());
        };

        loop {
            let plan = build_plan(home, &input)?;
            let summary = review_summary(&plan);
            print_numbered_plan(&plan, summary, palette);

            match choose_review_action(&render_config, summary)? {
                None | Some(ReviewAction::Cancel) => {
                    eprintln!("{}", palette.warning("Cancelled. No files were changed."));
                    return Ok(());
                }
                Some(ReviewAction::Edit) => continue 'configure,
                Some(ReviewAction::Finish) => {
                    eprintln!(
                        "{}",
                        palette.success("Configuration is already up to date.")
                    );
                    return Ok(());
                }
                Some(ReviewAction::Apply) => match apply_plan(home, &plan) {
                    Ok(transaction) => {
                        eprintln!();
                        eprintln!(
                            "{}",
                            palette.success(format!(
                                "Applied and verified {} modification(s).",
                                transaction.entries.len()
                            ))
                        );
                        eprintln!(
                            "{} {}",
                            palette.muted("Transaction:"),
                            palette.accent(transaction.id)
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        eprintln!();
                        eprintln!("{}", palette.danger(format!("Apply stopped: {error}")));
                        let options = vec!["Rebuild the plan", "Edit inputs", "Cancel"];
                        let choice = optional_prompt(
                            Select::new("What next?", options)
                                .with_starting_cursor(0)
                                .with_render_config(render_config)
                                .raw_prompt(),
                        )?;
                        match choice.map(|option| option.index) {
                            Some(0) => {}
                            Some(1) => continue 'configure,
                            _ => {
                                eprintln!(
                                    "{}",
                                    palette.warning("Cancelled. No additional files were changed.")
                                );
                                return Ok(());
                            }
                        }
                    }
                },
            }
        }
    }
}

fn render_config(color: bool) -> RenderConfig<'static> {
    if color {
        // The default also honors the conventional NO_COLOR environment variable.
        RenderConfig::default()
    } else {
        RenderConfig::empty()
    }
}

fn collect_input(
    render_config: &RenderConfig<'static>,
    palette: Palette,
    home: &Path,
) -> Result<Option<Input>> {
    let detected = detect_clients(home);
    let labels = client_labels(&detected);
    let defaults = CLIENTS
        .iter()
        .enumerate()
        .filter(|(_, client)| detected.contains(client))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    let selected = loop {
        let result = MultiSelect::new(CLIENT_PROMPT, labels.clone())
            .with_default(&defaults)
            .without_filtering()
            .with_render_config(*render_config)
            .raw_prompt();
        let Some(options) = optional_prompt(result)? else {
            return Ok(None);
        };
        if !options.is_empty() {
            break options;
        }
        eprintln!("{}", palette.danger("Select at least one client."));
    };

    let base_url = loop {
        let result = Text::new(BASE_URL_PROMPT)
            .with_placeholder("https://api.example.com")
            .with_render_config(*render_config)
            .prompt();
        let Some(value) = optional_prompt(result)? else {
            return Ok(None);
        };
        match validate_base_url(&value) {
            Ok(_) => break value,
            Err(error) => eprintln!("{}", palette.danger(format!("Invalid base URL: {error}"))),
        }
    };

    let token = loop {
        let result = Password::new(TOKEN_PROMPT)
            .without_confirmation()
            .with_display_mode(TOKEN_DISPLAY_MODE)
            .with_render_config(*render_config)
            .prompt();
        let Some(value) = optional_prompt(result)? else {
            return Ok(None);
        };
        match crate::Secret::new(value) {
            Ok(secret) => break secret,
            Err(error) => eprintln!("{}", palette.danger(format!("Invalid token: {error}"))),
        }
    };

    let clients = selected
        .into_iter()
        .map(|option| CLIENTS[option.index])
        .collect::<Vec<_>>();
    let model = if clients.iter().copied().any(Client::requires_model) {
        let Some(model) = prompt_model(render_config, palette)? else {
            return Ok(None);
        };
        Some(model)
    } else {
        None
    };
    let sdk = if clients.contains(&Client::OpenCode) {
        let Some(sdk) = prompt_sdk(model.as_deref(), render_config)? else {
            return Ok(None);
        };
        Some(sdk)
    } else {
        None
    };
    let model_name = if clients
        .iter()
        .any(|client| matches!(client, Client::OpenCode | Client::OpenClaw))
    {
        match prompt_model_name(render_config, palette)? {
            ModelNameChoice::Cancel => return Ok(None),
            ModelNameChoice::Omit => None,
            ModelNameChoice::Value(model_name) => Some(model_name),
        }
    } else {
        None
    };

    Ok(Some(Input {
        base_url,
        token,
        clients,
        model,
        model_name,
        sdk,
    }))
}

fn prompt_sdk(
    model: Option<&str>,
    render_config: &RenderConfig<'static>,
) -> Result<Option<OpenCodeSdk>> {
    let choices = OpenCodeSdk::choices();
    let inferred = OpenCodeSdk::infer(model);
    let cursor = choices
        .iter()
        .position(|choice| *choice == inferred)
        .expect("the inferred OpenCode SDK must be present in the workflow choices");
    optional_prompt(
        Select::new(SDK_PROMPT, choices.to_vec())
            // Put the likely wire protocol under the cursor while keeping the choice explicit.
            .with_starting_cursor(cursor)
            .with_render_config(*render_config)
            .prompt(),
    )
}

fn prompt_model(render_config: &RenderConfig<'static>, palette: Palette) -> Result<Option<String>> {
    loop {
        let result = Text::new(MODEL_PROMPT)
            .with_placeholder("e.g. gpt-4.1-mini")
            .with_render_config(*render_config)
            .prompt();
        let Some(value) = optional_prompt(result)? else {
            return Ok(None);
        };
        let value = value.trim().to_owned();
        match crate::validate_model_id(&value) {
            Ok(()) => return Ok(Some(value)),
            Err(error) => eprintln!("{}", palette.danger(format!("Invalid model ID: {error}"))),
        }
    }
}

fn prompt_model_name(
    render_config: &RenderConfig<'static>,
    palette: Palette,
) -> Result<ModelNameChoice> {
    loop {
        let result = Text::new(MODEL_NAME_PROMPT)
            .with_placeholder("e.g. GPT-5.5")
            .with_render_config(*render_config)
            .prompt();
        let Some(value) = optional_prompt(result)? else {
            return Ok(ModelNameChoice::Cancel);
        };
        let value = value.trim().to_owned();
        if value.is_empty() {
            return Ok(ModelNameChoice::Omit);
        }
        match validate_model_name(&value) {
            Ok(()) => return Ok(ModelNameChoice::Value(value)),
            Err(error) => eprintln!("{}", palette.danger(format!("Invalid model name: {error}"))),
        }
    }
}

fn optional_prompt<T>(result: std::result::Result<T, InquireError>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn client_labels(detected: &[Client]) -> Vec<String> {
    CLIENTS
        .iter()
        .map(|client| {
            if detected.contains(client) {
                format!("{client} (detected)")
            } else {
                client.to_string()
            }
        })
        .collect()
}

fn review_summary(plan: &Plan) -> ReviewSummary {
    ReviewSummary {
        updates: plan
            .changes
            .iter()
            .filter(|change| !matches!(change.action, Action::Noop))
            .count(),
        unchanged: plan
            .changes
            .iter()
            .filter(|change| matches!(change.action, Action::Noop))
            .count(),
        conflicts: plan.conflicts.len(),
        blocking: plan
            .conflicts
            .iter()
            .filter(|conflict| conflict.blocking)
            .count(),
    }
}

fn numbered_plan_items(plan: &Plan) -> Vec<NumberedPlanItem> {
    let mut items = Vec::new();
    let mut number = 1;
    for change in plan
        .changes
        .iter()
        .filter(|change| !matches!(change.action, Action::Noop))
    {
        items.push(NumberedPlanItem {
            number,
            marker: match &change.action {
                Action::Create => PlanMarker::Create,
                Action::Merge => PlanMarker::Update,
                Action::Delete => PlanMarker::Delete,
                Action::Noop => unreachable!("no-op changes are filtered above"),
            },
            client: change.client,
            path: change.path.clone(),
            reason: None,
        });
        number += 1;
    }
    for conflict in &plan.conflicts {
        items.push(NumberedPlanItem {
            number,
            marker: if conflict.blocking {
                PlanMarker::Blocked
            } else {
                PlanMarker::Review
            },
            client: conflict.client,
            path: conflict.path.clone(),
            reason: Some(conflict.reason.clone()),
        });
        number += 1;
    }
    items
}

fn print_numbered_plan(plan: &Plan, summary: ReviewSummary, palette: Palette) {
    eprintln!();
    eprintln!("{}", palette.accent("Planned modifications"));
    let items = numbered_plan_items(plan);
    if items.is_empty() {
        eprintln!("{}", palette.success("No files need modification."));
    } else {
        for item in items {
            eprintln!(
                "{} {} {}",
                palette.accent(format!("{}.", item.number)),
                palette.paint(item.marker.style(), format!("[{}]", item.marker.label())),
                item.client
            );
            eprintln!("   {}", item.path.display());
            if let Some(reason) = item.reason {
                eprintln!(
                    "   {}",
                    palette.paint(item.marker.style(), format!("Reason: {reason}"))
                );
            }
        }
    }
    for warning in &plan.warnings {
        eprintln!("{}", palette.warning(format!("Warning: {warning}")));
    }
    eprintln!(
        "{}",
        palette.muted(format!(
            "Summary: {} modification(s), {} unchanged, {} conflict(s).",
            summary.updates, summary.unchanged, summary.conflicts
        ))
    );
}

fn review_actions(summary: ReviewSummary) -> Vec<(ReviewAction, String)> {
    let mut actions = Vec::new();
    if summary.blocking == 0 && summary.updates > 0 {
        let review_suffix = if summary.conflicts > 0 {
            format!(", including {} reviewed overwrite(s)", summary.conflicts)
        } else {
            String::new()
        };
        actions.push((
            ReviewAction::Apply,
            format!(
                "Apply {} numbered modification(s){review_suffix}",
                summary.updates
            ),
        ));
    } else if summary.blocking == 0 {
        actions.push((ReviewAction::Finish, "Finish without writing".into()));
    }
    actions.push((ReviewAction::Edit, "Go back and edit inputs".into()));
    actions.push((ReviewAction::Cancel, "Cancel".into()));
    actions
}

fn choose_review_action(
    render_config: &RenderConfig<'static>,
    summary: ReviewSummary,
) -> Result<Option<ReviewAction>> {
    let actions = review_actions(summary);
    let labels = actions
        .iter()
        .map(|(_, label)| label.as_str())
        .collect::<Vec<_>>();
    let prompt = if summary.blocking > 0 {
        BLOCKED_PROMPT
    } else {
        REVIEW_PROMPT
    };
    Ok(optional_prompt(
        Select::new(prompt, labels)
            .with_starting_cursor(0)
            .with_render_config(*render_config)
            .raw_prompt(),
    )?
    .map(|option| actions[option.index].0))
}

#[cfg(test)]
mod tests;
