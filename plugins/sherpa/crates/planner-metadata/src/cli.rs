use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    path::PathBuf,
};

use crate::{SchemaRef, build, definition, definitions, parse, parse_and_validate, render};
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::info;

#[derive(Debug, Parser)]
#[command(
    name = "sherpa planner metadata",
    about = "Planner Event 메타데이터를 파싱하고 검증합니다"
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

pub fn run_from(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::try_parse_from(
        std::iter::once(OsString::from("sherpa planner metadata")).chain(arguments),
    )?;
    match cli.command {
        Command::Parse { input } => {
            let input = read_input(input.as_ref())?;
            info!(
                bytes = input.len(),
                "[planner:metadata:parse:start] 메모 파싱 시작"
            );
            let document = parse(&input)?;
            info!(schema = %document.schema.canonical(), "[planner:metadata:parse:success] 메모 파싱 완료");
            print_json(&document)?;
        }
        Command::Validate { input, json } => {
            let input = read_input(input.as_ref())?;
            info!(
                bytes = input.len(),
                "[planner:metadata:validate:start] 메모 검증 시작"
            );
            let document = parse_and_validate(&input)?;
            info!(schema = %document.schema.canonical(), "[planner:metadata:validate:success] 메모 검증 완료");
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
                "[planner:metadata:render:start] 메모 생성 시작"
            );
            let document = build(&schema, &summary, &fields)?;
            let output = render(&document)?;
            info!(schema = %document.schema.canonical(), "[planner:metadata:render:success] 메모 생성 완료");
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
            info!(path = %path.display(), "[planner:metadata:input:start] 파일 읽기 시작");
            let input = fs::read_to_string(path)?;
            info!(
                bytes = input.len(),
                "[planner:metadata:input:success] 파일 읽기 완료"
            );
            Ok(input)
        }
        _ => {
            info!("[planner:metadata:input:start] 표준 입력 읽기 시작");
            let mut input = String::new();
            io::stdin().read_to_string(&mut input)?;
            info!(
                bytes = input.len(),
                "[planner:metadata:input:success] 표준 입력 읽기 완료"
            );
            Ok(input)
        }
    }
}

fn print_json(value: &(impl Serialize + ?Sized)) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
