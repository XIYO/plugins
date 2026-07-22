use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result};
use chrono_tz::Tz;
use clap::{Args, Parser, Subcommand, ValueEnum};
use msgpipe::{
    benchmark,
    command::{CommandRunner, SystemCommandRunner},
    export::{ExportConfig, ExportFormat, render},
    extract::{ExtractRequest, extract_source, resolve_binary},
    model::SourceKind,
    optimizer::{OptimizationProfile, optimize},
    state::{ContextScope, StateStore},
    time_range::DateRange,
};
use tracing::error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "msgpipe",
    version,
    about = "Read-only local message optimizer and exporter"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Extract, optimize, alias, and write a model-ready transcript to stdout.
    Export(PipelineArgs),
    /// Count tokens and transformations without printing any message content.
    Benchmark(PipelineArgs),
    /// Verify that a source reader exists without reading message content.
    Doctor(DoctorArgs),
    /// Store or retrieve derived per-thread/global analysis context.
    Context(ContextArgs),
    /// Resolve a stable thread/speaker alias map from protected local state.
    Identities(IdentityArgs),
    /// Print the compact CCT legend used in model prompts.
    CctSpec,
}

#[derive(Debug, Args)]
struct PipelineArgs {
    #[arg(value_enum)]
    source: SourceArg,
    /// Inclusive start, as YYYY-MM-DD in --timezone or RFC 3339.
    #[arg(long)]
    start: String,
    /// Exclusive end, as YYYY-MM-DD in --timezone or RFC 3339.
    #[arg(long)]
    end: String,
    #[arg(long, default_value = "Asia/Seoul")]
    timezone: String,
    #[arg(long, value_enum, default_value_t = ProfileArg::Schedule)]
    profile: ProfileArg,
    #[arg(long, value_enum, default_value_t = FormatArg::Cct)]
    format: FormatArg,
    #[arg(long, default_value_t = 30)]
    session_gap_minutes: u32,
    /// Explicit path to kakaocli or imsg; otherwise PATH is searched.
    #[arg(long)]
    binary: Option<PathBuf>,
    /// Protected SQLite mapping/audit database.
    #[arg(long)]
    state: Option<PathBuf>,
    #[arg(long, default_value_t = 10_000)]
    chat_limit: usize,
    #[arg(long, default_value_t = 1_000_000)]
    message_limit_per_chat: usize,
    /// Export or benchmark only this stable thread alias after registering all aliases.
    #[arg(long)]
    thread: Option<String>,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(value_enum)]
    source: SourceArg,
    #[arg(long)]
    binary: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[command(subcommand)]
    command: ContextCommand,
}

#[derive(Debug, Args)]
struct IdentityArgs {
    /// Stable K001/I001 thread alias.
    thread: String,
    #[arg(long)]
    state: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    /// Append a derived summary read from stdin.
    Put(ContextPutArgs),
    /// Print the latest derived summary for a scope.
    Get(ContextGetArgs),
    /// List context metadata without summary bodies.
    List(ContextListArgs),
}

#[derive(Debug, Args)]
struct ContextPutArgs {
    #[arg(value_enum)]
    scope: ContextScopeArg,
    /// Required for thread scope; use the stable K001/I001 alias.
    #[arg(long)]
    thread: Option<String>,
    #[arg(long)]
    start: String,
    #[arg(long)]
    end: String,
    #[arg(long, default_value = "Asia/Seoul")]
    timezone: String,
    #[arg(long, default_value = "gpt-5.6-terra")]
    model: String,
    #[arg(long, default_value = "medium")]
    reasoning_effort: String,
    #[arg(long)]
    state: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ContextGetArgs {
    #[arg(value_enum)]
    scope: ContextScopeArg,
    #[arg(long)]
    thread: Option<String>,
    #[arg(long)]
    state: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ContextListArgs {
    #[arg(long)]
    state: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ContextScopeArg {
    Global,
    Thread,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SourceArg {
    #[value(alias = "kakaotalk")]
    Kakao,
    #[value(alias = "imsg")]
    Imessage,
}

impl SourceArg {
    fn kind(self) -> SourceKind {
        match self {
            Self::Kakao => SourceKind::KakaoTalk,
            Self::Imessage => SourceKind::IMessage,
        }
    }

    fn program(self) -> &'static str {
        match self {
            Self::Kakao => "kakaocli",
            Self::Imessage => "imsg",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileArg {
    Exact,
    Schedule,
}

impl From<ProfileArg> for OptimizationProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Exact => Self::Exact,
            ProfileArg::Schedule => Self::Schedule,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Cct,
    Tsv,
    Json,
}

impl From<FormatArg> for ExportFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Cct => Self::Cct,
            FormatArg::Tsv => Self::Tsv,
            FormatArg::Json => Self::Json,
        }
    }
}

fn main() -> ExitCode {
    init_logging();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_value) => {
            error!(
                error = ?error_value,
                "[cli:run:failure] msgpipe command failed"
            );
            eprintln!("msgpipe: {error_value:#}");
            ExitCode::FAILURE
        }
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Export(args) => run_export(args),
        Command::Benchmark(args) => run_benchmark(args),
        Command::Doctor(args) => run_doctor(args),
        Command::Context(args) => run_context(args),
        Command::Identities(args) => run_identities(args),
        Command::CctSpec => {
            println!(
                "CCT3: !CCT3|g=minutes|z=timezone; T=thread; D=YYMMDD; S=HHmm; row=speaker|text; blank speaker inherits; A=self; Y/N/?=reaction"
            );
            Ok(())
        }
    }
}

fn run_export(args: PipelineArgs) -> Result<()> {
    let prepared = prepare(&args)?;
    let profile = OptimizationProfile::from(args.profile);
    let outcome = optimize(&prepared.messages, profile)?;
    let state_path = args.state.unwrap_or(StateStore::default_path()?);
    let mut state = StateStore::open(&state_path)?;
    let aliased = select_thread(state.register(&outcome)?, args.thread.as_deref())?;
    let rendered = render(
        args.format.into(),
        &aliased,
        ExportConfig {
            profile,
            timezone: prepared.timezone,
            session_gap_minutes: args.session_gap_minutes,
        },
    )?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(rendered.as_bytes())
        .context("unable to write exported transcript")?;
    output
        .flush()
        .context("unable to flush exported transcript")
}

fn run_benchmark(args: PipelineArgs) -> Result<()> {
    let prepared = prepare(&args)?;
    let profile = OptimizationProfile::from(args.profile);
    let exact = optimize(&prepared.messages, OptimizationProfile::Exact)?;
    let optimized = optimize(&prepared.messages, profile)?;
    let state_path = args.state.unwrap_or(StateStore::default_path()?);
    let mut state = StateStore::open(&state_path)?;
    let raw_aliased = select_thread(state.register(&exact)?, args.thread.as_deref())?;
    let optimized_aliased = select_thread(state.register(&optimized)?, args.thread.as_deref())?;
    let report = benchmark::measure(
        args.source.kind(),
        &raw_aliased,
        &optimized_aliased,
        &optimized,
        ExportConfig {
            profile,
            timezone: prepared.timezone,
            session_gap_minutes: args.session_gap_minutes,
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_doctor(args: DoctorArgs) -> Result<()> {
    let binary = resolve_binary(args.binary, args.source.program())?;
    let runner = SystemCommandRunner;
    let output = runner.run("doctor.version", &binary, &[OsString::from("--version")])?;
    let version = String::from_utf8(output.stdout).context("version output is not UTF-8")?;
    println!("{}: {}", args.source.program(), version.trim());
    Ok(())
}

fn run_context(args: ContextArgs) -> Result<()> {
    match args.command {
        ContextCommand::Put(args) => {
            let timezone: Tz = args
                .timezone
                .parse()
                .context("invalid IANA timezone name")?;
            let range = DateRange::parse(&args.start, &args.end, timezone)?;
            let scope = context_scope(args.scope, args.thread)?;
            let state_path = args.state.unwrap_or(StateStore::default_path()?);
            let mut state = StateStore::open(&state_path)?;
            let mut summary = String::new();
            io::stdin()
                .lock()
                .take(1_048_577)
                .read_to_string(&mut summary)
                .context("unable to read analysis summary from stdin")?;
            if summary.len() > 1_048_576 {
                anyhow::bail!("analysis summary exceeds the 1 MiB limit")
            }
            let id = state.save_analysis_context(
                &scope,
                range.start,
                range.end,
                &args.model,
                &args.reasoning_effort,
                &summary,
            )?;
            println!("{id}");
            Ok(())
        }
        ContextCommand::Get(args) => {
            let scope = context_scope(args.scope, args.thread)?;
            let state_path = args.state.unwrap_or(StateStore::default_path()?);
            let state = StateStore::open(&state_path)?;
            let context = state
                .latest_analysis_context(&scope)?
                .context("no analysis context exists for this scope")?;
            print!("{}", context.summary);
            Ok(())
        }
        ContextCommand::List(args) => {
            let state_path = args.state.unwrap_or(StateStore::default_path()?);
            let state = StateStore::open(&state_path)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&state.list_analysis_contexts()?)?
            );
            Ok(())
        }
    }
}

fn run_identities(args: IdentityArgs) -> Result<()> {
    let state_path = args.state.unwrap_or(StateStore::default_path()?);
    let state = StateStore::open(&state_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&state.identity_map(&args.thread)?)?
    );
    Ok(())
}

fn context_scope(scope: ContextScopeArg, thread: Option<String>) -> Result<ContextScope> {
    match (scope, thread) {
        (ContextScopeArg::Global, None) => Ok(ContextScope::Global),
        (ContextScopeArg::Global, Some(_)) => {
            anyhow::bail!("--thread is not valid for global context")
        }
        (ContextScopeArg::Thread, Some(alias)) if !alias.trim().is_empty() => {
            Ok(ContextScope::Thread(alias))
        }
        (ContextScopeArg::Thread, _) => {
            anyhow::bail!("--thread is required for thread context")
        }
    }
}

struct PreparedPipeline {
    timezone: Tz,
    messages: Vec<msgpipe::NormalizedMessage>,
}

fn prepare(args: &PipelineArgs) -> Result<PreparedPipeline> {
    if args.session_gap_minutes == 0 {
        anyhow::bail!("session gap must be greater than zero")
    }
    if args.chat_limit == 0 || args.message_limit_per_chat == 0 {
        anyhow::bail!("source limits must be greater than zero")
    }
    let timezone: Tz = args
        .timezone
        .parse()
        .context("invalid IANA timezone name")?;
    let range = DateRange::parse(&args.start, &args.end, timezone)?;
    let binary = resolve_binary(args.binary.clone(), args.source.program())?;
    let mut request = ExtractRequest::for_binary(range, binary);
    request.chat_limit = args.chat_limit;
    request.message_limit_per_chat = args.message_limit_per_chat;
    let messages = extract_source(args.source.kind(), &request)?;
    Ok(PreparedPipeline { timezone, messages })
}

fn select_thread(
    messages: Vec<msgpipe::AliasedMessage>,
    thread_alias: Option<&str>,
) -> Result<Vec<msgpipe::AliasedMessage>> {
    let Some(thread_alias) = thread_alias else {
        return Ok(messages);
    };
    let selected: Vec<_> = messages
        .into_iter()
        .filter(|message| message.thread_alias == thread_alias)
        .collect();
    if selected.is_empty() {
        anyhow::bail!("thread alias was not found in the selected source and date range")
    }
    Ok(selected)
}
