#![forbid(unsafe_code)]

pub mod cli;

use std::{collections::HashSet, error::Error, fmt};

use serde::Serialize;

pub const SCHEMA_PREFIX: &str = "xiyo.calendar.";
const SCHEMA_MARKER: &str = "@schema: ";
const SUMMARY_SEPARATOR: &str = " · ";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaRef {
    pub name: String,
    pub major: u64,
    pub minor: u64,
}

impl SchemaRef {
    pub fn parse(value: &str) -> Result<Self, MetadataError> {
        let (name, version) = value
            .split_once('@')
            .ok_or_else(|| MetadataError::new("스키마는 <이름>@<버전> 형식이어야 합니다"))?;

        if !valid_schema_name(name) {
            return Err(MetadataError::new(format!(
                "올바르지 않은 스키마 이름입니다: {name}"
            )));
        }

        let parts = version.split('.').collect::<Vec<_>>();
        if parts.is_empty() || parts.len() > 2 {
            return Err(MetadataError::new(
                "스키마 버전은 MAJOR 또는 MAJOR.MINOR만 허용합니다",
            ));
        }

        let major = parse_version_component(parts[0], "MAJOR")?;
        if major == 0 {
            return Err(MetadataError::new(
                "스키마 MAJOR 버전은 1 이상이어야 합니다",
            ));
        }
        let minor = if parts.len() == 2 {
            parse_version_component(parts[1], "MINOR")?
        } else {
            0
        };

        Ok(Self {
            name: name.to_owned(),
            major,
            minor,
        })
    }

    pub fn canonical(&self) -> String {
        if self.minor == 0 {
            format!("{}@{}", self.name, self.major)
        } else {
            format!("{}@{}.{}", self.name, self.major, self.minor)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataDocument {
    pub schema: SchemaRef,
    pub summary: Vec<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FieldRule {
    pub name: &'static str,
    pub required: bool,
    pub introduced_minor: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SectionRule {
    pub name: &'static str,
    pub required: bool,
    pub fields: &'static [FieldRule],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SchemaDefinition {
    pub name: &'static str,
    pub major: u64,
    pub latest_minor: u64,
    pub summary_parts: usize,
    pub sections: &'static [SectionRule],
}

const TELECOM_SERVICE_FIELDS: &[FieldRule] = &[
    FieldRule {
        name: "제공자",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "상품",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "종류",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "식별자",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "속도",
        required: false,
        introduced_minor: 0,
    },
];

const TELECOM_CONTRACT_FIELDS: &[FieldRule] = &[
    FieldRule {
        name: "시작일",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "종료일",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "할인기간",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "약정기간",
        required: false,
        introduced_minor: 0,
    },
    FieldRule {
        name: "결합",
        required: false,
        introduced_minor: 0,
    },
];

const TELECOM_BILLING_FIELDS: &[FieldRule] = &[
    FieldRule {
        name: "청구일",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "결제수단",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "청구서",
        required: false,
        introduced_minor: 0,
    },
];

const TELECOM_ACCOUNT_FIELDS: &[FieldRule] = &[FieldRule {
    name: "로그인",
    required: false,
    introduced_minor: 0,
}];

const TELECOM_SECTIONS: &[SectionRule] = &[
    SectionRule {
        name: "서비스",
        required: true,
        fields: TELECOM_SERVICE_FIELDS,
    },
    SectionRule {
        name: "계약",
        required: false,
        fields: TELECOM_CONTRACT_FIELDS,
    },
    SectionRule {
        name: "납부",
        required: true,
        fields: TELECOM_BILLING_FIELDS,
    },
    SectionRule {
        name: "계정",
        required: false,
        fields: TELECOM_ACCOUNT_FIELDS,
    },
];

const CARD_FIELDS: &[FieldRule] = &[
    FieldRule {
        name: "발급사",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "상품",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "끝번호",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "유효기간",
        required: false,
        introduced_minor: 0,
    },
];

const CARD_PAYMENT_FIELDS: &[FieldRule] = &[
    FieldRule {
        name: "결제일",
        required: true,
        introduced_minor: 0,
    },
    FieldRule {
        name: "결제계좌",
        required: false,
        introduced_minor: 0,
    },
];

const CARD_STATEMENT_FIELDS: &[FieldRule] = &[FieldRule {
    name: "수신처",
    required: false,
    introduced_minor: 0,
}];

const CARD_SECTIONS: &[SectionRule] = &[
    SectionRule {
        name: "카드",
        required: true,
        fields: CARD_FIELDS,
    },
    SectionRule {
        name: "결제",
        required: true,
        fields: CARD_PAYMENT_FIELDS,
    },
    SectionRule {
        name: "명세서",
        required: false,
        fields: CARD_STATEMENT_FIELDS,
    },
];

pub const SCHEMAS: &[SchemaDefinition] = &[
    SchemaDefinition {
        name: "xiyo.calendar.telecom-billing",
        major: 1,
        latest_minor: 0,
        summary_parts: 3,
        sections: TELECOM_SECTIONS,
    },
    SchemaDefinition {
        name: "xiyo.calendar.card-payment",
        major: 1,
        latest_minor: 0,
        summary_parts: 3,
        sections: CARD_SECTIONS,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataError {
    pub line: Option<usize>,
    pub message: String,
}

impl MetadataError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: message.into(),
        }
    }

    fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            message: message.into(),
        }
    }
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(formatter, "{line}행: {}", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl Error for MetadataError {}

pub fn parse(input: &str) -> Result<MetadataDocument, MetadataError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    if lines.is_empty() || lines.iter().all(|line| line.trim().is_empty()) {
        return Err(MetadataError::new("메타데이터 메모가 비어 있습니다"));
    }

    let first_non_empty = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .expect("빈 입력은 앞에서 거부됩니다");
    if first_non_empty != 0 {
        return Err(MetadataError::at(1, "요약 앞에 빈 줄을 둘 수 없습니다"));
    }

    let last_non_empty = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .expect("빈 입력은 앞에서 거부됩니다");
    let marker_line = lines[last_non_empty].trim();
    let marker_value = marker_line.strip_prefix(SCHEMA_MARKER).ok_or_else(|| {
        MetadataError::at(
            last_non_empty + 1,
            "마지막 줄에 @schema: <이름>@<버전>을 선언해야 합니다",
        )
    })?;
    let schema = SchemaRef::parse(marker_value)
        .map_err(|error| MetadataError::at(last_non_empty + 1, error.message))?;

    for (index, line) in lines[..last_non_empty].iter().enumerate() {
        if line.trim().starts_with(SCHEMA_MARKER) {
            return Err(MetadataError::at(
                index + 1,
                "@schema 선언은 마지막에 한 번만 사용할 수 있습니다",
            ));
        }
    }

    let summary = parse_summary(lines[0], 1)?;
    let mut sections = Vec::<Section>::new();
    let mut current_section: Option<Section> = None;
    let mut section_names = HashSet::<String>::new();

    for (index, raw_line) in lines[1..last_non_empty].iter().enumerate() {
        let line_number = index + 2;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(field_text) = line.strip_prefix("• ") {
            let section = current_section.as_mut().ok_or_else(|| {
                MetadataError::at(line_number, "필드는 구역 제목 다음에 와야 합니다")
            })?;
            let (name, value) = field_text.split_once(": ").ok_or_else(|| {
                MetadataError::at(line_number, "필드는 • 필드: 값 형식이어야 합니다")
            })?;
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return Err(MetadataError::at(
                    line_number,
                    "필드명과 값은 비어 있을 수 없습니다",
                ));
            }
            if section.fields.iter().any(|field| field.name == name) {
                return Err(MetadataError::at(
                    line_number,
                    format!("중복 필드입니다: {}.{name}", section.name),
                ));
            }
            section.fields.push(Field {
                name: name.to_owned(),
                value: value.to_owned(),
            });
            continue;
        }

        if line.starts_with('•') {
            return Err(MetadataError::at(
                line_number,
                "불릿 뒤에 공백을 두고 • 필드: 값 형식을 사용해야 합니다",
            ));
        }
        if line.contains(": ") {
            return Err(MetadataError::at(
                line_number,
                "필드 앞에는 • 기호가 필요합니다",
            ));
        }

        if let Some(section) = current_section.take() {
            if section.fields.is_empty() {
                return Err(MetadataError::at(
                    line_number - 1,
                    format!("구역에 필드가 없습니다: {}", section.name),
                ));
            }
            sections.push(section);
        }

        if !section_names.insert(line.to_owned()) {
            return Err(MetadataError::at(
                line_number,
                format!("중복 구역입니다: {line}"),
            ));
        }
        current_section = Some(Section {
            name: line.to_owned(),
            fields: Vec::new(),
        });
    }

    if let Some(section) = current_section {
        if section.fields.is_empty() {
            return Err(MetadataError::new(format!(
                "구역에 필드가 없습니다: {}",
                section.name
            )));
        }
        sections.push(section);
    }

    if sections.is_empty() {
        return Err(MetadataError::new(
            "하나 이상의 메타데이터 구역이 필요합니다",
        ));
    }

    Ok(MetadataDocument {
        schema,
        summary,
        sections,
    })
}

pub fn validate(document: &MetadataDocument) -> Result<(), MetadataError> {
    let definition = schema_definition(&document.schema)?;

    if document.schema.minor > definition.latest_minor {
        return Err(MetadataError::new(format!(
            "지원하지 않는 MINOR 버전입니다: {} (최대 {}.{})",
            document.schema.canonical(),
            definition.major,
            definition.latest_minor
        )));
    }

    if document.summary.len() != definition.summary_parts {
        return Err(MetadataError::new(format!(
            "요약은 ' · '로 구분한 {}개 항목이어야 합니다",
            definition.summary_parts
        )));
    }

    for section in &document.sections {
        let section_rule = definition
            .sections
            .iter()
            .find(|rule| rule.name == section.name)
            .ok_or_else(|| {
                MetadataError::new(format!("허용되지 않은 구역입니다: {}", section.name))
            })?;

        for field in &section.fields {
            let field_rule = section_rule
                .fields
                .iter()
                .find(|rule| rule.name == field.name)
                .ok_or_else(|| {
                    MetadataError::new(format!(
                        "허용되지 않은 필드입니다: {}.{}",
                        section.name, field.name
                    ))
                })?;
            if field_rule.introduced_minor > document.schema.minor {
                return Err(MetadataError::new(format!(
                    "{}.{} 필드는 {}.{}부터 사용할 수 있습니다",
                    section.name, field.name, definition.major, field_rule.introduced_minor
                )));
            }
        }

        for required in section_rule.fields.iter().filter(|field| field.required) {
            if !section
                .fields
                .iter()
                .any(|field| field.name == required.name)
            {
                return Err(MetadataError::new(format!(
                    "필수 필드가 없습니다: {}.{}",
                    section.name, required.name
                )));
            }
        }
    }

    for required in definition
        .sections
        .iter()
        .filter(|section| section.required)
    {
        if !document
            .sections
            .iter()
            .any(|section| section.name == required.name)
        {
            return Err(MetadataError::new(format!(
                "필수 구역이 없습니다: {}",
                required.name
            )));
        }
    }

    Ok(())
}

pub fn parse_and_validate(input: &str) -> Result<MetadataDocument, MetadataError> {
    let document = parse(input)?;
    validate(&document)?;
    Ok(document)
}

pub fn render(document: &MetadataDocument) -> Result<String, MetadataError> {
    validate(document)?;
    let definition = schema_definition(&document.schema)?;
    let mut output = String::new();
    output.push_str(&document.summary.join(SUMMARY_SEPARATOR));

    for section_rule in definition.sections {
        let Some(section) = document
            .sections
            .iter()
            .find(|section| section.name == section_rule.name)
        else {
            continue;
        };
        output.push_str("\n\n");
        output.push_str(section_rule.name);
        for field_rule in section_rule.fields {
            let Some(field) = section
                .fields
                .iter()
                .find(|field| field.name == field_rule.name)
            else {
                continue;
            };
            output.push_str("\n• ");
            output.push_str(field_rule.name);
            output.push_str(": ");
            output.push_str(&field.value);
        }
    }

    output.push_str("\n\n");
    output.push_str(SCHEMA_MARKER);
    output.push_str(&document.schema.canonical());
    Ok(output)
}

pub fn build(
    schema: &str,
    summary: &str,
    assignments: &[String],
) -> Result<MetadataDocument, MetadataError> {
    let schema = SchemaRef::parse(schema)?;
    let definition = schema_definition(&schema)?;
    let summary = parse_summary(summary, 1)?;
    let mut sections = Vec::<Section>::new();

    for assignment in assignments {
        let (path, value) = assignment
            .split_once('=')
            .ok_or_else(|| MetadataError::new("--field는 구역.필드=값 형식이어야 합니다"))?;
        let (section_name, field_name) = path
            .split_once('.')
            .ok_or_else(|| MetadataError::new("--field는 구역.필드=값 형식이어야 합니다"))?;
        let section_name = section_name.trim();
        let field_name = field_name.trim();
        let value = value.trim();
        if section_name.is_empty() || field_name.is_empty() || value.is_empty() {
            return Err(MetadataError::new(
                "--field의 구역, 필드명, 값은 비어 있을 수 없습니다",
            ));
        }

        let section = if let Some(index) = sections
            .iter()
            .position(|section| section.name == section_name)
        {
            &mut sections[index]
        } else {
            sections.push(Section {
                name: section_name.to_owned(),
                fields: Vec::new(),
            });
            sections.last_mut().expect("방금 구역을 추가했습니다")
        };
        if section.fields.iter().any(|field| field.name == field_name) {
            return Err(MetadataError::new(format!(
                "중복 필드입니다: {section_name}.{field_name}"
            )));
        }
        section.fields.push(Field {
            name: field_name.to_owned(),
            value: value.to_owned(),
        });
    }

    let document = MetadataDocument {
        schema,
        summary,
        sections,
    };
    validate(&document)?;

    let section_order = definition
        .sections
        .iter()
        .map(|section| section.name)
        .collect::<Vec<_>>();
    let mut document = document;
    document.sections.sort_by_key(|section| {
        section_order
            .iter()
            .position(|name| *name == section.name)
            .unwrap_or(usize::MAX)
    });
    Ok(document)
}

pub fn definitions() -> &'static [SchemaDefinition] {
    SCHEMAS
}

pub fn definition(schema: &SchemaRef) -> Result<&'static SchemaDefinition, MetadataError> {
    schema_definition(schema)
}

fn schema_definition(schema: &SchemaRef) -> Result<&'static SchemaDefinition, MetadataError> {
    SCHEMAS
        .iter()
        .find(|definition| definition.name == schema.name && definition.major == schema.major)
        .ok_or_else(|| {
            MetadataError::new(format!(
                "지원하지 않는 스키마입니다: {}",
                schema.canonical()
            ))
        })
}

fn parse_summary(value: &str, line: usize) -> Result<Vec<String>, MetadataError> {
    let parts = value
        .split(SUMMARY_SEPARATOR)
        .map(str::trim)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.iter().any(String::is_empty) {
        return Err(MetadataError::at(line, "요약 항목은 비어 있을 수 없습니다"));
    }
    Ok(parts)
}

fn valid_schema_name(value: &str) -> bool {
    value.starts_with(SCHEMA_PREFIX)
        && value
            .split('.')
            .all(|part| !part.is_empty() && valid_name_part(part))
}

fn valid_name_part(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn parse_version_component(value: &str, label: &str) -> Result<u64, MetadataError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.chars().all(|character| character.is_ascii_digit())
    {
        return Err(MetadataError::new(format!(
            "올바르지 않은 {label} 버전입니다: {value}"
        )));
    }
    value
        .parse::<u64>()
        .map_err(|_| MetadataError::new(format!("{label} 버전이 너무 큽니다: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TELECOM_NOTE: &str = "요금 청구일 · 예시모바일 · 예시카드 1234\n\n서비스\n• 제공자: 예시모바일\n• 상품: 데이터 11GB\n• 종류: 휴대전화\n• 식별자: 010-00**-00**\n\n계약\n• 시작일: 2026.07.13\n• 할인기간: 2026.07.13 ~ 2027.02.12\n\n납부\n• 청구일: 매월 9일\n• 결제수단: 예시카드 · 끝 1234\n• 청구서: billing@example.com\n\n계정\n• 로그인: 연결 계정\n\n@schema: xiyo.calendar.telecom-billing@1";

    #[test]
    fn parses_and_renders_canonical_telecom_note() {
        let document = parse_and_validate(TELECOM_NOTE).expect("유효한 통신 메모");
        assert_eq!(document.schema.major, 1);
        assert_eq!(document.schema.minor, 0);
        assert_eq!(document.summary.len(), 3);
        assert_eq!(render(&document).expect("렌더링"), TELECOM_NOTE);
    }

    #[test]
    fn accepts_crlf_and_normalizes_to_lf() {
        let crlf = TELECOM_NOTE.replace('\n', "\r\n");
        let document = parse_and_validate(&crlf).expect("CRLF 메모");
        assert!(!render(&document).expect("렌더링").contains('\r'));
    }

    #[test]
    fn rejects_patch_versions() {
        let invalid = TELECOM_NOTE.replace("@1", "@1.0.1");
        let error = parse(&invalid).expect_err("PATCH 버전 거부");
        assert!(error.message.contains("MAJOR.MINOR"));
    }

    #[test]
    fn rejects_duplicate_fields() {
        let invalid = TELECOM_NOTE.replace(
            "• 제공자: 예시모바일",
            "• 제공자: 예시모바일\n• 제공자: 다른 통신사",
        );
        let error = parse(&invalid).expect_err("중복 필드 거부");
        assert!(error.message.contains("중복 필드"));
    }

    #[test]
    fn rejects_unknown_fields() {
        let invalid = TELECOM_NOTE.replace(
            "• 상품: 데이터 11GB",
            "• 상품: 데이터 11GB\n• 화면문구: 이메일(상세)",
        );
        let error = parse_and_validate(&invalid).expect_err("알 수 없는 필드 거부");
        assert!(error.message.contains("허용되지 않은 필드"));
    }

    #[test]
    fn rejects_missing_required_fields() {
        let invalid = TELECOM_NOTE.replace("• 결제수단: 예시카드 · 끝 1234\n", "");
        let error = parse_and_validate(&invalid).expect_err("필수 필드 거부");
        assert!(error.message.contains("납부.결제수단"));
    }

    #[test]
    fn builds_card_metadata_in_schema_order() {
        let fields = vec![
            "결제.결제일=매월 14일".to_owned(),
            "카드.끝번호=1234".to_owned(),
            "카드.상품=예시 생활카드".to_owned(),
            "카드.발급사=예시카드".to_owned(),
            "명세서.수신처=billing@example.com".to_owned(),
        ];
        let document = build(
            "xiyo.calendar.card-payment@1",
            "카드 대금 결제일 · 예시 생활카드 · 예시카드 1234",
            &fields,
        )
        .expect("카드 메타데이터 생성");
        let rendered = render(&document).expect("렌더링");
        assert!(rendered.find("\n\n카드\n").unwrap() < rendered.find("\n\n결제\n").unwrap());
        assert!(rendered.ends_with("@schema: xiyo.calendar.card-payment@1"));
    }

    #[test]
    fn rejects_unknown_major_and_future_minor_versions() {
        let unknown_major = TELECOM_NOTE.replace(
            "xiyo.calendar.telecom-billing@1",
            "xiyo.calendar.telecom-billing@2",
        );
        assert!(
            parse_and_validate(&unknown_major)
                .expect_err("알 수 없는 MAJOR 거부")
                .message
                .contains("지원하지 않는 스키마")
        );

        let future_minor = TELECOM_NOTE.replace(
            "xiyo.calendar.telecom-billing@1",
            "xiyo.calendar.telecom-billing@1.1",
        );
        assert!(
            parse_and_validate(&future_minor)
                .expect_err("미지원 MINOR 거부")
                .message
                .contains("지원하지 않는 MINOR")
        );
    }

    #[test]
    fn skill_reference_covers_every_registered_schema_field() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let reference_path = [
            manifest_dir.join("skills/planner/references/event-metadata.md"),
            manifest_dir.join("../../skills/planner/references/event-metadata.md"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .expect("Planner 이벤트 메타데이터 스키마 문서 경로");
        let reference = std::fs::read_to_string(reference_path)
            .expect("Planner 이벤트 메타데이터 스키마 문서 읽기");

        for schema in SCHEMAS {
            let schema_heading = format!("{}@{}", schema.name, schema.major);
            assert!(
                reference.contains(&schema_heading),
                "스킬 문서에 스키마가 없습니다: {schema_heading}"
            );
            for section in schema.sections {
                assert!(
                    reference.contains(section.name),
                    "스킬 문서에 구역이 없습니다: {}.{}",
                    schema.name,
                    section.name
                );
                for field in section.fields {
                    assert!(
                        reference.contains(field.name),
                        "스킬 문서에 필드가 없습니다: {}.{}.{}",
                        schema.name,
                        section.name,
                        field.name
                    );
                }
            }
        }
    }
}
