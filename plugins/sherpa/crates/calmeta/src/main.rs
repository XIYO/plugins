use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    process::ExitCode,
};

use calmeta::{SchemaRef, build, definition, definitions, parse, parse_and_validate, render};
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "calmeta",
    version,
    about = "Apple Calendar 메모 메타데이터를 파싱하고 검증합니다"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 메모 구문을 파싱해 JSON으로 출력합니다.
    Parse {
        /// 입력 파일. 생략하거나 -이면 표준 입력을 사용합니다.
        input: Option<PathBuf>,
    },
    /// 메모를 파싱하고 등록된 스키마로 검증합니다.
    Validate {
        /// 입력 파일. 생략하거나 -이면 표준 입력을 사용합니다.
        input: Option<PathBuf>,
        /// 검증된 문서를 JSON으로 출력합니다.
        #[arg(long)]
        json: bool,
    },
    /// 필드 인자로 정규화된 메모를 생성합니다.
    Render {
        #[arg(long)]
        schema: String,
        #[arg(long)]
        summary: String,
        /// 구역.필드=값 형식. 여러 번 사용할 수 있습니다.
        #[arg(long = "field", required = true)]
        fields: Vec<String>,
    },
    /// 지원 스키마 정의를 출력합니다.
    Spec {
        /// 스키마 이름과 버전. 생략하면 전체 정의를 출력합니다.
        schema: Option<String>,
    },
}

#[derive(Serialize)]
struct ValidationResult<'a> {
    valid: bool,
    schema: &'a SchemaRef,
}

fn main() -> ExitCode {
    init_logging();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error_value) => {
            error!(error = %error_value, "[calmeta:command:failed] 명령 실행 실패");
            eprintln!("오류: {error_value}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { input } => {
            let input = read_input(input.as_ref())?;
            info!(bytes = input.len(), "[calmeta:parse:start] 메모 파싱 시작");
            let document = parse(&input)?;
            info!(schema = %document.schema.canonical(), "[calmeta:parse:success] 메모 파싱 완료");
            print_json(&document)?;
        }
        Command::Validate { input, json } => {
            let input = read_input(input.as_ref())?;
            info!(
                bytes = input.len(),
                "[calmeta:validate:start] 메모 검증 시작"
            );
            let document = parse_and_validate(&input)?;
            info!(schema = %document.schema.canonical(), "[calmeta:validate:success] 메모 검증 완료");
            if json {
                print_json(&document)?;
            } else {
                print_json(&ValidationResult {
                    valid: true,
                    schema: &document.schema,
                })?;
            }
        }
        Command::Render {
            schema,
            summary,
            fields,
        } => {
            info!(
                field_count = fields.len(),
                "[calmeta:render:start] 메모 생성 시작"
            );
            let document = build(&schema, &summary, &fields)?;
            let output = render(&document)?;
            info!(schema = %document.schema.canonical(), "[calmeta:render:success] 메모 생성 완료");
            println!("{output}");
        }
        Command::Spec { schema } => match schema {
            Some(value) => {
                let schema = SchemaRef::parse(&value)?;
                print_json(definition(&schema)?)?;
            }
            None => print_json(definitions())?,
        },
    }
    Ok(())
}

fn read_input(path: Option<&PathBuf>) -> Result<String, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path.as_os_str() != "-" => {
            info!(path = %path.display(), "[calmeta:input:start] 파일 읽기 시작");
            let input = fs::read_to_string(path)?;
            info!(
                bytes = input.len(),
                "[calmeta:input:success] 파일 읽기 완료"
            );
            Ok(input)
        }
        _ => {
            info!("[calmeta:input:start] 표준 입력 읽기 시작");
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            info!(
                bytes = input.len(),
                "[calmeta:input:success] 표준 입력 읽기 완료"
            );
            Ok(input)
        }
    }
}

fn print_json(value: &(impl Serialize + ?Sized)) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn init_logging() {
    let filter = env::var("CALMETA_LOG_LEVEL")
        .or_else(|_| env::var("LOG_LEVEL"))
        .unwrap_or_else(|_| "warn".to_owned());
    let env_filter = match EnvFilter::try_new(&filter) {
        Ok(env_filter) => env_filter,
        Err(error_value) => {
            eprintln!(
                "[calmeta:logging:warn] Invalid log filter; falling back to warn: {error_value}"
            );
            EnvFilter::new("warn")
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(io::stderr)
        .without_time()
        .with_target(false)
        .init();
}
