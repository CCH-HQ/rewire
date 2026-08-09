use clap::{
    Args, ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};
use clap_complete::Shell;
use std::path::PathBuf;

const CLAP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Cyan.on_default().bold())
    .usage(AnsiColor::Cyan.on_default().bold())
    .literal(AnsiColor::Green.on_default().bold())
    .placeholder(AnsiColor::Yellow.on_default())
    .error(AnsiColor::Red.on_default().bold())
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());

const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\ncommit: ",
    env!("REWIRE_GIT_COMMIT"),
    "\ntarget: ",
    env!("REWIRE_BUILD_TARGET")
);

#[derive(Debug, Parser)]
#[command(
    name = "rewire",
    version,
    long_version = LONG_VERSION,
    propagate_version = true,
    about = "Safely configure compatible AI clients",
    styles = CLAP_STYLES,
    help_template = "Rewire {version}\n{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}"
)]
pub(crate) struct Cli {
    /// Compatible API base URL; adapters preserve existing path components.
    #[arg(long, global = true, env = "REWIRE_BASEURL", hide_env_values = true)]
    pub(crate) baseurl: Option<String>,
    /// API credential; prefer --token-stdin or the guided workflow on shared systems.
    #[arg(
        long,
        global = true,
        env = "REWIRE_TOKEN",
        conflicts_with = "token_stdin",
        hide_env_values = true
    )]
    pub(crate) token: Option<String>,
    /// Read the API token from standard input without exposing it in process arguments.
    #[arg(long, global = true)]
    pub(crate) token_stdin: bool,
    /// Comma-separated clients: claude,codex,opencode,hermes,openclaw.
    #[arg(long, global = true)]
    pub(crate) client: Option<String>,
    /// Optional model ID for clients that require an explicit custom-model catalog.
    #[arg(long, global = true)]
    pub(crate) model: Option<String>,
    #[command(flatten)]
    pub(crate) execution: ExecutionOptions,
    #[command(flatten)]
    pub(crate) display: DisplayOptions,
    /// Override the user home directory for fixtures or isolated operation.
    #[arg(long, global = true)]
    pub(crate) home: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Args)]
pub(crate) struct ExecutionOptions {
    /// Disable interactive prompting for scripts and agents.
    #[arg(long, global = true)]
    pub(crate) non_interactive: bool,
    /// Build and print a plan without changing configuration files.
    #[arg(long, global = true)]
    pub(crate) dry_run: bool,
    /// Apply a conflict-free plan without an additional confirmation prompt.
    #[arg(long, global = true)]
    pub(crate) yes: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DisplayOptions {
    /// Emit stable machine-readable JSON.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Disable colors in help, errors, and the guided workflow.
    #[arg(long, global = true)]
    pub(crate) no_color: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run the guided client-selection and numbered-review workflow.
    #[command(alias = "tui")]
    Configure,
    /// Generate a plan and exit without applying it.
    Plan,
    /// Detect supported client configuration files and report environment state.
    Doctor,
    /// Restore files recorded by a committed transaction.
    Rollback {
        /// Transaction identifier returned by a successful apply.
        id: String,
    },
    /// Inspect transaction backups.
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    /// Remove Rewire-owned client fields and credential files transactionally.
    Remove,
    /// Generate a shell completion script from the current command schema.
    Completions {
        /// Target shell.
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BackupCommand {
    /// List available transaction identifiers.
    List,
}

/// Parse the command line after applying color policy to Clap's early-exit paths.
pub(crate) fn parse() -> Cli {
    let color = if color_requested() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };
    let matches = Cli::command().color(color).get_matches();
    Cli::from_arg_matches(&matches).expect("Clap matches must satisfy the derived CLI schema")
}

/// Detect JSON mode before command execution so runtime errors use the requested transport.
pub(crate) fn json_requested() -> bool {
    std::env::args_os().any(|argument| argument == "--json")
}

/// Detect explicit monochrome policy before Clap handles help or validation errors.
pub(crate) fn color_requested() -> bool {
    std::env::var_os("NO_COLOR").is_none()
        && !std::env::args_os().any(|argument| argument == "--no-color")
}
