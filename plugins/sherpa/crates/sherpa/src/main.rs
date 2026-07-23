use std::{
    env,
    ffi::OsString,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::json;
use sherpa::{
    adapters::{
        approvals::FileApprovalRepository,
        kakaotalk::KakaoTalkGateway,
        system::{SecureTokenGenerator, SystemClock},
    },
    context::reply::{DEFAULT_TTL_SECONDS, ReplyService},
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "sherpa",
    version,
    about = "Collect personal context and manage plans through one local-first interface"
)]
struct Cli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Collect, review, and act on personal context.
    Context(ContextArgs),
    /// Manage events and tasks through Calendar and Reminders adapters.
    Planner(PlannerArgs),
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[command(subcommand)]
    command: ContextCommand,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    /// Prepare, confirm, or cancel a confirmation-bound reply.
    Reply(ReplyArgs),
    /// Store or retrieve derived session, conversation, or global summaries.
    Summary(AdapterArguments),
    /// Context operation such as sync, pending, or status.
    #[command(external_subcommand)]
    Operation(Vec<OsString>),
}

#[derive(Debug, Args)]
struct ReplyArgs {
    #[command(subcommand)]
    command: ReplyCommand,
}

#[derive(Debug, Subcommand)]
enum ReplyCommand {
    /// Resolve one exact conversation and create a short-lived approval preview.
    Prepare {
        #[arg(long, value_enum)]
        via: ReplyChannel,
        #[arg(long)]
        conversation: String,
        #[arg(long, default_value_t = DEFAULT_TTL_SECONDS, value_parser = clap::value_parser!(u64).range(60..=1800))]
        ttl_seconds: u64,
    },
    /// Dispatch exactly the text shown in the approved preview.
    Confirm {
        #[arg(long)]
        token: String,
    },
    /// Discard an unused approval preview.
    Cancel {
        #[arg(long)]
        token: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReplyChannel {
    Kakaotalk,
}

#[derive(Debug, Args)]
struct PlannerArgs {
    #[command(subcommand)]
    command: PlannerCommand,
}

#[derive(Debug, Subcommand)]
enum PlannerCommand {
    /// Manage events and calendars through the Apple EventKit adapter.
    Calendar(AdapterArguments),
    /// Manage tasks and lists through the Apple Reminders adapter.
    Reminders(AdapterArguments),
    /// Render or validate structured event metadata.
    Metadata(AdapterArguments),
}

#[derive(Debug, Args)]
struct AdapterArguments {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    arguments: Vec<OsString>,
}

fn main() -> ExitCode {
    init_logging();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_value) => {
            error!(error = ?error_value, "[application:sherpa:failure] command failed");
            eprintln!("sherpa: {error_value:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        RootCommand::Context(args) => run_context(args),
        RootCommand::Planner(args) => run_planner(args),
    }
}

fn run_context(args: ContextArgs) -> Result<()> {
    match args.command {
        ContextCommand::Reply(args) => run_reply(args),
        ContextCommand::Summary(args) => {
            let mut arguments = vec![OsString::from("context")];
            arguments.extend(args.arguments);
            sherpa_context::cli::run_from(arguments)
        }
        ContextCommand::Operation(arguments) => sherpa_context::cli::run_from(arguments),
    }
}

fn run_reply(args: ReplyArgs) -> Result<()> {
    let approvals = FileApprovalRepository::from_environment()?;
    let gateway = KakaoTalkGateway::from_environment()?;
    let clock = SystemClock;
    let tokens = SecureTokenGenerator;
    let service = ReplyService::new(&gateway, &approvals, &clock, &tokens);

    match args.command {
        ReplyCommand::Prepare {
            via: ReplyChannel::Kakaotalk,
            conversation,
            ttl_seconds,
        } => {
            let message = read_stdin_message()?;
            let preview = service.prepare(&conversation, message, ttl_seconds)?;
            println!("{}", serde_json::to_string(&preview)?);
        }
        ReplyCommand::Confirm { token } => {
            let message = read_stdin_message()?;
            service.confirm(&token, &message)?;
            println!(
                "{}",
                json!({
                    "status": "dispatched",
                    "token": token,
                    "channel": "kakaotalk"
                })
            );
        }
        ReplyCommand::Cancel { token } => {
            let existed = service.cancel(&token)?;
            println!(
                "{}",
                json!({
                    "status": "cancelled",
                    "token": token,
                    "existed": existed
                })
            );
        }
    }
    Ok(())
}

fn run_planner(args: PlannerArgs) -> Result<()> {
    match args.command {
        PlannerCommand::Calendar(args) => run_adapter(Adapter::Calendar, &args.arguments),
        PlannerCommand::Reminders(args) => run_adapter(Adapter::Reminders, &args.arguments),
        PlannerCommand::Metadata(args) => sherpa_planner_metadata::cli::run_from(args.arguments)
            .map_err(|error_value| anyhow::anyhow!("{error_value}")),
    }
}

#[derive(Debug, Clone, Copy)]
enum Adapter {
    Calendar,
    Reminders,
}

impl Adapter {
    fn command(self) -> &'static str {
        match self {
            Self::Calendar => "sherpa-calendar-adapter",
            Self::Reminders => "sherpa-reminders-adapter",
        }
    }

    fn override_variable(self) -> &'static str {
        match self {
            Self::Calendar => "SHERPA_PLANNER_CALENDAR_BIN",
            Self::Reminders => "SHERPA_PLANNER_REMINDERS_BIN",
        }
    }

    fn boundary(self) -> &'static str {
        match self {
            Self::Calendar => "planner.calendar",
            Self::Reminders => "planner.reminders",
        }
    }
}

fn run_adapter(adapter: Adapter, arguments: &[OsString]) -> Result<()> {
    if arguments.is_empty() {
        bail!("{} requires an operation", adapter.boundary())
    }
    let binary = resolve_adapter(adapter)?;
    info!(
        boundary = adapter.boundary(),
        argument_count = arguments.len(),
        "[adapter:command:start] starting domain adapter"
    );
    let status = ProcessCommand::new(&binary)
        .args(arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("unable to start {}", adapter.boundary()))?;
    if !status.success() {
        bail!(
            "{} failed with exit code {:?}",
            adapter.boundary(),
            status.code()
        )
    }
    info!(
        boundary = adapter.boundary(),
        "[adapter:command:success] domain adapter completed"
    );
    Ok(())
}

fn resolve_adapter(adapter: Adapter) -> Result<PathBuf> {
    if let Some(configured) = env::var_os(adapter.override_variable()) {
        let configured = PathBuf::from(configured);
        if is_executable(&configured) {
            return Ok(configured);
        }
        bail!(
            "configured {} adapter is not executable",
            adapter.boundary()
        )
    }
    if let Some(root) = install_root() {
        let managed = root.join("bin").join(adapter.command());
        if is_executable(&managed) {
            return Ok(managed);
        }
    }
    which::which(adapter.command())
        .with_context(|| format!("{} adapter is not installed", adapter.boundary()))
}

fn install_root() -> Option<PathBuf> {
    env::var_os("SHERPA_INSTALL_ROOT")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".local")))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn read_stdin_message() -> Result<String> {
    let mut message = String::new();
    io::stdin()
        .lock()
        .take(8_193)
        .read_to_string(&mut message)
        .context("unable to read reply text from standard input")?;
    if message.len() > 8_192 {
        bail!("reply input exceeds the byte limit")
    }
    Ok(message)
}

fn init_logging() {
    let filter = EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
