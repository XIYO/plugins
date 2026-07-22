---
id: DESIGN-PIPE
title: Message Pipeline Design
status: draft
owner: maintainer
---
# Message Pipeline Design

## 요구사항 범위

`FR-PIPE-EXTRACT-001`, `FR-PIPE-NORM-001`, `FR-PIPE-OPT-001`~`003`, `FR-PIPE-ALIAS-001`~`002`, `FR-PIPE-EXPORT-001`, `FR-PIPE-BENCH-001`~`002`, `FR-PIPE-CONTEXT-001`을 구현한다.

## 구성요소

- `extract::kakao`: epoch 범위를 값으로만 삽입한 고정 CTE/SELECT를 `kakaocli query`에 전달한다.
- `extract::imessage`: `imsg chats`로 기간 후보를 찾고 각 chat에 `history --start --end --attachments --json`을 실행한다.
- 각 `extract` adapter: 앱 고유 JSON, 메시지 type, 첨부 키와 강한 관계 신호를 공통 구조로 만든다. 일반 메타 필드 존재만으로 첨부·답장을 추론하지 않는다.
- `optimizer::replacer`: NFC, 채팅 반응, 무의미 기호, 반복, URL과 첨부 표식을 메시지 단위 순수 함수로 변환한다.
- `optimizer::structure`: 정규화된 메시지 사이의 연속 중복과 향후 답장/템플릿 관계를 판단한다. 앱 이름이나 원본 JSON 키를 참조하지 않는다.
- `export`: 세션화와 시각·화자 상속처럼 선택 출력 형식에만 필요한 압축을 수행한다.
- `state`: 별칭 할당과 원문 없는 메시지 인덱스를 단일 SQLite 트랜잭션에 기록한다.
- `export`: 별칭이 적용된 메시지를 CCT/TSV/JSON으로 렌더링한다.
- `benchmark`: 같은 결과를 `o200k_base`로 세고 분포만 직렬화한다.

## 데이터 모델

| 엔터티 | 핵심 필드 | 비고 |
|---|---|---|
| `NormalizedMessage` | source, message_id, thread, author, timestamp_utc, kind, content, attachments | 메모리 전용 원문 |
| `OptimizedMessage` | normalized metadata, rendered_content, transforms | 내보내기 직전 메모리 전용 |
| `threads` | source, source_thread_id, display_name, alias | 로컬 보호 DB |
| `speakers` | thread_id, source_author_id, display_name, alias, is_self | 자기 자신은 A |
| `message_audit` | source_message_id, timestamp_utc, content_sha256, action_codes | 본문 저장 금지 |
| `analysis_context` | scope, scope_key, period, model, reasoning_effort, summary | 명시적으로 제출된 파생 요약, append-only |

## 런타임 시퀀스와 트랜잭션 경계

1. 외부 프로세스 stdout을 메모리에서 파싱한다.
2. 전 행 검증과 최적화를 완료한다.
3. `BEGIN IMMEDIATE` 이후 별칭과 감사 행을 upsert한다.
4. commit 뒤 모델 형식을 렌더링한다. 렌더러는 DB를 쓰지 않는다.

외부 출력이 불완전하거나 파싱이 실패하면 상태 트랜잭션을 시작하지 않는다. 동일 원본 메시지는 `(source, source_message_id)`로 멱등 upsert한다.

## 실패 처리

- 외부 명령의 비정상 종료는 stderr 본문을 그대로 로그에 남기지 않고 종료 코드와 오류 분류만 보존한다.
- JSON/NDJSON 스키마 오류는 행 번호와 누락 필드명으로 실패한다.
- SQLite 오류는 롤백 후 원본 오류 체인을 상위로 전달한다.
- stdout 쓰기 실패는 렌더 단계 실패로 반환한다.

## 보안

- 실행 가능한 하위 명령을 코드 상수로 제한한다.
- 셸을 통하지 않고 `Command` 인자 배열을 사용한다.
- 상태 디렉터리/파일 권한을 매 실행 검증·교정한다.
- 첨부 경로와 연락처 핸들은 공통 모델에서 폐기하고 종류·MIME·크기만 남긴다.

## 관측성

`[extract:<source>:start|success|failure]`, `[state:sqlite:*]`, `[pipeline:optimize:*]`, `[export:<format>:*]` 경계를 기록한다. 필드는 source, row_count, duration_ms, profile, transform_count만 허용한다.

## 마이그레이션 / 롤백

SQLite `PRAGMA user_version`으로 단방향 스키마 마이그레이션을 수행한다. 새 CCT 버전은 헤더 버전을 올리고 기존 파서를 유지한다. 애플리케이션 롤백 시 상태 DB를 삭제할 필요가 없도록 additive migration을 우선한다.

## 테스트 전략

합성 fixture의 소스별 의미 동등성, 모든 최적화 분기, CCT escaping/상속 왕복, 별칭 재실행 결정성, 상태 파일 모드와 로그 redaction을 검증한다. 실데이터 테스트는 집계 기준만 비교한다.

## 관련 문서

**요구사항 README** — [Message Pipeline Requirements](../../requirements/pipeline/README.md)

**유스케이스** — [UC-PIPE-001 기간 메시지 컨텍스트 내보내기](../../requirements/pipeline/use-cases/UC-PIPE-001-export-thread-context.md)

**ADR** — [ADR-0001 Rust core와 CLI adapter](../../adr/0001-rust-core-cli-adapters.md) · [ADR-0002 CCT 세션 형식](../../adr/0002-cct-session-format.md) · [ADR-0003 최적화 단계 경계](../../adr/0003-source-normalization-common-optimization.md)

**상위** — [ARCHITECTURE](../../../ARCHITECTURE.md)
