use anstyle::{AnsiColor, Style};
use anyhow::Result;
use rewire::{Action, DoctorReport, Plan, Transaction, stable_json};
use serde::Serialize;
use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write as _};

const ACCENT: Style = AnsiColor::Cyan.on_default().bold();
const SUCCESS: Style = AnsiColor::Green.on_default().bold();
const WARNING: Style = AnsiColor::Yellow.on_default().bold();
const DANGER: Style = AnsiColor::Red.on_default().bold();
const MUTED: Style = AnsiColor::BrightBlack.on_default();

#[derive(Debug, Clone, Copy)]
struct Palette {
    enabled: bool,
}

impl Palette {
    fn stdout(requested: bool) -> Self {
        Self::new(requested && io::stdout().is_terminal())
    }

    fn stderr(requested: bool) -> Self {
        Self::new(requested && io::stderr().is_terminal())
    }

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

/// Shared command output boundary: human text by default, stable JSON only on request.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Output {
    json: bool,
    palette: Palette,
}

impl Output {
    /// Build an output layer for stdout while honoring terminal and monochrome policy.
    pub(crate) fn stdout(json: bool, color_requested: bool) -> Self {
        Self {
            json,
            palette: Palette::stdout(color_requested && !json),
        }
    }

    /// Render doctor state in the selected human or JSON representation.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or stdout writing fails.
    pub(crate) fn doctor(self, report: &DoctorReport) -> Result<()> {
        if self.json {
            return Self::json(report);
        }
        let mut text = format!("{}\n", self.palette.accent("Rewire doctor"));
        writeln!(
            text,
            "{} {}",
            self.palette.muted("Home:"),
            report.home.display()
        )?;
        if report.detected.is_empty() {
            writeln!(
                text,
                "{}",
                self.palette
                    .warning("No supported client configurations were detected.")
            )?;
        }
        for diagnostic in &report.clients {
            let marker = match (diagnostic.installed, diagnostic.configuration_detected) {
                (true, true) => self.palette.success("[READY]"),
                (true, false) => self.palette.accent("[INSTALLED]"),
                (false, true) => self.palette.warning("[CONFIG]"),
                (false, false) => self.palette.muted("[MISSING]"),
            };
            writeln!(text, "  {marker} {}", diagnostic.client)?;
            writeln!(
                text,
                "    {} {}",
                self.palette.muted("Config:"),
                diagnostic.config_path.display()
            )?;
            if let Some(version) = &diagnostic.version {
                writeln!(text, "    {} {version}", self.palette.muted("Version:"))?;
            }
            if !diagnostic.environment.is_empty() {
                writeln!(
                    text,
                    "    {} {} (values hidden)",
                    self.palette.warning("Environment:"),
                    diagnostic.environment.join(", ")
                )?;
            }
        }
        write_stdout(&text)
    }

    /// Render a numbered plan or its stable serialized contract.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or stdout writing fails.
    pub(crate) fn plan(self, plan: &Plan) -> Result<()> {
        if self.json {
            Self::json(plan)
        } else {
            write_stdout(&render_plan(plan, self.palette)?)
        }
    }

    /// Report a verified apply operation without leaking prepared credentials.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or stdout writing fails.
    pub(crate) fn applied(self, transaction: &Transaction, plan: &Plan) -> Result<()> {
        if self.json {
            return Self::json(&serde_json::json!({
                "ok": true,
                "transaction": transaction,
                "plan": plan,
            }));
        }
        let mut text = format!("{}\n", self.palette.accent("Rewire apply"));
        writeln!(
            text,
            "{}",
            self.palette.success(format!(
                "Applied and verified {} modification(s).",
                transaction.entries.len()
            ))
        )?;
        writeln!(
            text,
            "{} {}",
            self.palette.muted("Transaction:"),
            self.palette.accent(&transaction.id)
        )?;
        for entry in &transaction.entries {
            writeln!(
                text,
                "  {} {} ({})",
                self.palette.success("[WRITTEN]"),
                entry.path.display(),
                entry.client
            )?;
        }
        write_stdout(&text)
    }

    /// Report a completed rollback in the selected representation.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or stdout writing fails.
    pub(crate) fn rollback(self, id: &str) -> Result<()> {
        if self.json {
            return Self::json(&serde_json::json!({"ok": true, "rolled_back": id}));
        }
        let text = format!(
            "{}\n{} {}\n",
            self.palette.accent("Rewire rollback"),
            self.palette.success("Restored transaction"),
            self.palette.accent(id)
        );
        write_stdout(&text)
    }

    /// Render a human cancellation without turning an intentional `No` into an error.
    ///
    /// # Errors
    ///
    /// Returns an error when stdout cannot be written.
    pub(crate) fn cancelled(self, message: &str) -> Result<()> {
        if self.json {
            return Self::json(&serde_json::json!({
                "ok": true,
                "cancelled": true,
            }));
        }
        write_stdout(&format!("{}\n", self.palette.warning(message)))
    }

    /// Render transaction backups for manual selection or automation.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or stdout writing fails.
    pub(crate) fn backups(self, ids: &[String]) -> Result<()> {
        if self.json {
            return Self::json(&serde_json::json!({"transactions": ids}));
        }
        let mut text = format!("{}\n", self.palette.accent("Rewire backups"));
        if ids.is_empty() {
            writeln!(
                text,
                "{}",
                self.palette.warning("No transaction backups were found.")
            )?;
        } else {
            for (index, id) in ids.iter().enumerate() {
                writeln!(
                    text,
                    "{} {id}",
                    self.palette.accent(format!("{}.", index + 1))
                )?;
            }
        }
        write_stdout(&text)
    }

    fn json<T: Serialize>(value: &T) -> Result<()> {
        write_stdout(&stable_json(value)?)
    }

    /// Print a runtime error as JSON for automation or colored text for an operator.
    pub(crate) fn error(error: &anyhow::Error, json: bool, color_requested: bool) {
        let message = error
            .chain()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(": ");
        let text = if json {
            stable_json(&serde_json::json!({
                "ok": false,
                "error": message,
            }))
            .expect("the fixed error payload is always JSON serializable")
        } else {
            let palette = Palette::stderr(color_requested);
            format!("{} {message}\n", palette.danger("Error:"))
        };
        let _ = io::stderr().lock().write_all(text.as_bytes());
    }
}

fn render_plan(plan: &Plan, palette: Palette) -> Result<String> {
    let mut text = format!("{}\n", palette.accent("Rewire plan"));
    if !plan.base_url.is_empty() {
        writeln!(text, "{} {}", palette.muted("Endpoint:"), plan.base_url)?;
    }
    writeln!(
        text,
        "{} {}\n",
        palette.muted("Clients:"),
        plan.clients
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    if let Some(model) = &plan.model {
        writeln!(text, "{} {}", palette.muted("Model ID:"), model)?;
    }
    if let Some(model_name) = &plan.model_name {
        writeln!(text, "{} {}", palette.muted("Model name:"), model_name)?;
    }
    if !plan.models.is_empty() {
        writeln!(
            text,
            "{} {}",
            palette.muted("Catalog models:"),
            plan.models.len()
        )?;
    }
    if let Some(sdk) = &plan.sdk {
        let (label, value) = match sdk.as_str() {
            "@ai-sdk/openai" => ("OpenCode provider:", "openai (native model catalog)"),
            "@ai-sdk/anthropic" => ("OpenCode provider:", "anthropic (native model catalog)"),
            _ => ("OpenCode SDK:", sdk.as_str()),
        };
        writeln!(text, "{} {}", palette.muted(label), value)?;
    }
    if plan.model.is_some() || plan.sdk.is_some() {
        writeln!(text)?;
    }

    let mut number = 1;
    for change in &plan.changes {
        let marker = match change.action {
            Action::Create => palette.success("[CREATE]"),
            Action::Merge => palette.accent("[UPDATE]"),
            Action::Delete => palette.warning("[DELETE]"),
            Action::Noop => palette.muted("[UNCHANGED]"),
        };
        writeln!(
            text,
            "{} {} {}",
            palette.accent(format!("{number}.")),
            marker,
            change.client
        )?;
        writeln!(text, "   {}", change.path.display())?;
        number += 1;
    }
    for conflict in &plan.conflicts {
        let marker = if conflict.blocking {
            palette.danger("[BLOCKED]")
        } else {
            palette.warning("[REVIEW]")
        };
        writeln!(
            text,
            "{} {} {}",
            palette.accent(format!("{number}.")),
            marker,
            conflict.client
        )?;
        writeln!(text, "   {}", conflict.path.display())?;
        let reason = format!("Reason: {}", conflict.reason);
        writeln!(
            text,
            "   {}",
            if conflict.blocking {
                palette.danger(reason)
            } else {
                palette.warning(reason)
            }
        )?;
        number += 1;
    }
    if plan.changes.is_empty() && plan.conflicts.is_empty() {
        writeln!(
            text,
            "{}",
            palette.success("No client files require review.")
        )?;
    }
    for warning in &plan.warnings {
        writeln!(text, "{}", palette.warning(format!("Warning: {warning}")))?;
    }
    append_plan_summary(&mut text, plan, palette)?;
    Ok(text)
}

fn append_plan_summary(text: &mut String, plan: &Plan, palette: Palette) -> Result<()> {
    let modified = plan
        .changes
        .iter()
        .filter(|change| !matches!(change.action, Action::Noop))
        .count();
    let unchanged = plan.changes.len() - modified;
    writeln!(text)?;
    writeln!(
        text,
        "{}",
        palette.muted(format!(
            "Summary: {modified} modification(s), {unchanged} unchanged, {} conflict(s).",
            plan.conflicts.len()
        ))
    )?;
    Ok(())
}

fn write_stdout(text: &str) -> Result<()> {
    io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests;
