---
id: DESIGN-PIPE
title: Message Pipeline Design
status: draft
owner: maintainer
---
# Message Pipeline Design

## 요구사항 범위

읽기 전용 소스 동기화, 원문 보관, 공통 최적화, 별칭, CCT/토큰 집계, pending 조회와 세션 요약 커밋을 구현한다.

## 구성요소

- `extract::kakao`: epoch 범위를 넣은 고정 CTE/SELECT만 `kakaocli query`에 전달한다.
- `extract::imessage`: `imsg chats`와 각 chat의 `history --start --end --attachments --json`만 실행한다.
- `archive`: 최적화 전 `NormalizedMessage`를 멱등 upsert하고 변경된 메시지를 pending으로 되돌린다.
- `optimizer::replacer`: NFC, 채팅 반응, 무의미 기호, 반복, URL과 첨부 표식을 순수 함수로 변환한다.
- `optimizer::structure`: 정규화된 메시지 사이의 연속 중복을 판단한다.
- `state`: 원문 아카이브, 별칭, 변환 감사, 제시·분석 상태와 append-only 요약을 트랜잭션으로 조정한다.
- `export`: 세션화와 시각·화자 상속을 적용해 CCT/TSV/JSON으로 렌더링한다.
- `benchmark`: 아카이브에서 같은 범위를 불러와 `o200k_base` 분포만 직렬화한다.

## 데이터 모델

| 엔터티 | 핵심 필드 | 비고 |
|---|---|---|
| `NormalizedMessage` | source, message_id, thread, author, timestamp_utc, kind, content, attachments | 최적화 전 원문 모델 |
| `source_thread` | source, source_thread_id, display_name, alias | 모델에는 alias만 전달 |
| `speaker` | thread_id, source_author_id, display_name, alias, is_self | 자기 자신은 A |
| `archived_message` | 원문, attachments JSON, hash, ingested/presented/analyzed 시각, context ID | 사용자 로컬 SQLite |
| `source_sync` | source, period, insert/update/unchanged count, completed_at | 원문 없는 실행 기록 |
| `message_audit` | source_message_id, profile, hash, kept, transform_codes | 변환 추적 |
| `analysis_context` | scope, period, model, effort, summary, coverage, rollup link/watermark | append-only 세션/스레드/전역 요약 |

## 동기화

1. 외부 프로세스 stdout을 메모리에서 전부 파싱·검증한다.
2. exact profile로 별칭과 원문 해시를 준비한다.
3. `BEGIN IMMEDIATE`에서 별칭·감사·원문·sync 실행을 함께 upsert한다.
4. 기존 행과 본문·시각·화자·종류·첨부가 달라지면 `last_presented_at_utc`, `analyzed_at_utc`, `analysis_context_id`를 `NULL`로 만든다.

소스 출력이 불완전하거나 파싱이 실패하면 상태 트랜잭션을 시작하지 않는다.

## 증분 분석

1. `pending`은 선택 범위의 `analysis_context_id IS NULL` 행만 읽는다.
2. 공통 옵티마이저와 exporter가 CCT를 만든다.
3. stdout flush 성공 후 입력 행의 `last_presented_at_utc`를 기록한다.
4. 분석기는 CCT의 각 `S` 구간을 요약한다.
5. `context put session`은 제시된 pending 행의 정렬된 `(message_id, content_sha256)` 집합 해시와 요약을 저장하고 분석 연결을 한 트랜잭션에서 설정한다.
6. `context inputs thread`는 미반영 세션 요약을 모두 CTX로 내보낸다. `context inputs global`은 별칭별 최신 미반영 thread rollup만 내보내 중간 누적 버전을 생략한다.
7. 분석기는 기존 스레드 rollup과 CTX 세션 입력으로 새 `thread` rollup을, 기존 전역 rollup과 CTX thread 입력으로 새 `global` rollup을 만든다.
8. `context put thread|global --through-context-id`는 rollup insert와 watermark 이하 입력 연결을 원자적으로 처리한다. 출력 뒤 생긴 더 큰 ID는 다음 실행에 남는다.
9. 다음 분석은 최신 전역/스레드 rollup과 새 pending만 사용한다.

## 실패 처리

- 외부 명령의 비정상 종료는 stderr 본문을 로그에 남기지 않고 종료 코드만 보존한다.
- JSON/NDJSON 오류는 행 번호와 누락 필드명으로 실패한다.
- SQLite 오류는 롤백 후 오류 체인을 상위로 전달한다.
- stdout 쓰기 실패 시 제시 시점을 기록하지 않는다.
- 요약 저장이나 coverage 검증 실패 시 메시지를 pending으로 유지한다.

## 보안

- 실행 가능한 소스 하위 명령을 코드 상수로 제한하고 셸을 사용하지 않는다.
- 원문은 상태 SQLite에만 저장하며 로그·Git·별도 덤프에 기록하지 않는다.
- 상태 디렉터리/파일 권한을 `0700`/`0600`으로 교정한다.
- SQLite 자체는 암호화되지 않으므로 FileVault와 소유자 계정 경계를 요구한다.

## 테스트 전략

합성 fixture로 소스별 파싱, 원문 round-trip, 동기화 멱등성, 수정 시 pending 복귀, 제시 전 요약 거부, 세션 요약의 원자적 분석 연결, CCT escaping, 별칭 결정성과 파일 모드를 검증한다. 실데이터 테스트는 집계만 출력한다.

## 관련 문서

**요구사항** — [Message Pipeline Requirements](../../requirements/pipeline/README.md)

**유스케이스** — [UC-PIPE-001](../../requirements/pipeline/use-cases/UC-PIPE-001-export-thread-context.md)

**계약** — [CCT](../../../contracts/cct/CCT.md) · [CTX](../../../contracts/context/CTX.md)

**ADR** — [ADR-0001](../../adr/0001-rust-core-cli-adapters.md) · [ADR-0002](../../adr/0002-cct-session-format.md) · [ADR-0003](../../adr/0003-source-normalization-common-optimization.md) · [ADR-0004](../../adr/0004-local-raw-archive-incremental-analysis.md)

**상위** — [ARCHITECTURE](../../../ARCHITECTURE.md)
