---
status: draft
owner: maintainer
---
# Software Architecture Description

## 요구사항 개요

시스템은 KakaoTalk·iMessage 원본 저장소를 읽기 전용으로 동기화하고, 별도의 보호 SQLite에 최적화 전 원문과 증분 분석 상태를 보관한다. 모델에는 미분석 메시지만 별칭 기반 CCT로 전달하고, 성공한 세션 요약을 다음 분석의 맥락으로 재사용한다.

## 품질 목표

1. 원본 메시지 저장소를 절대 변경하지 않는 안전성
2. 동일 소스 메시지의 멱등 보관과 수정·지연 수집 감지
3. 분석 실패 시 메시지를 잃지 않는 트랜잭션 경계
4. 과거 원문 재첨부를 피하는 스레드·세션 단위 증분성
5. 모델 입력과 실명·원본 식별자 매핑의 분리

## 제약

- macOS 로컬 환경을 대상으로 한다.
- KakaoTalk SQLCipher 키 파생과 iMessage `attributedBody` 복원은 검증된 외부 리더에 위임한다.
- msgpipe 아카이브는 원문을 포함하는 일반 SQLite다. 디렉터리 `0700`, 파일 `0600`과 FileVault를 보안 경계로 삼는다.
- 기본 시간대는 `Asia/Seoul`, 범위 종료는 배타적이다.

## 컨텍스트

```text
KakaoTalk DB -> kakaocli read-only --+
                                      +-> sync -> protected raw SQLite
Messages chat.db -> imsg read-only --+                    |
                                                          +-> status / benchmark
                                                          +-> pending -> Optimizer -> CCT -> model
                                                                                         |
                                      prior summaries <- analysis_context <- summary commit
```

## 솔루션 전략

`sync`만 외부 CLI를 호출한다. 파서는 소스 JSON을 공통 `NormalizedMessage`로 만든 뒤 원문 본문, ID, 이름, 정확 시각과 첨부 메타데이터를 `(source, source_message_id)`로 upsert한다. 변경된 행은 기존 분석 연결과 마지막 제시 시점을 제거한다.

`pending`은 `analysis_context_id IS NULL`인 아카이브 행만 불러와 공통 옵티마이저와 CCT exporter에 전달한다. stdout 쓰기가 성공한 뒤 해당 행의 `last_presented_at_utc`를 기록한다. `context put session`은 제시된 pending 행의 집합 해시와 파생 요약을 저장하고 같은 트랜잭션에서 분석 연결을 설정한다. `context inputs`는 아직 상위 요약에 연결되지 않은 session/thread 요약을 CTX로 내보낸다. `context put thread`와 `context put global`은 각각 스레드·전체 누적 rollup을 append-only로 저장하고 CTX watermark 이하 입력을 원자적으로 연결하되 메시지 상태는 변경하지 않는다.

## 빌딩 블록

- `extract`: 허용된 외부 CLI 명령 실행과 앱별 JSON·타입·첨부 신호 파싱
- `archive`: 원문 upsert, 아카이브 조회, last-presented/analysis coverage, 상태 집계
- `model`: 소스 독립 메시지·첨부 메타데이터
- `optimizer::replacer`: 채팅 어휘·반응·반복·URL·첨부 표식 치환
- `optimizer::structure`: 메시지 간 중복과 강한 관계 신호를 이용한 구조 최적화
- `state`: SQLite 스키마 마이그레이션, 별칭·감사·세션/스레드/전역 요약 트랜잭션
- `export`: CCT3/CCT2 세션·필드 상속, TSV, compact JSON
- `benchmark`: 아카이브 입력의 `o200k_base` 토큰 집계

## 상태 모델

- `source_thread`, `speaker`: 원본 식별자·표시 이름과 안정 별칭
- `archived_message`: 최적화 전 본문·첨부 메타데이터·해시·수집/제시/분석 시각·요약 연결
- `source_sync`: 소스·범위별 동기화 실행과 insert/update/unchanged 건수
- `message_audit`: 프로필별 변환 코드와 보존 여부
- `analysis_context`: append-only 세션/스레드/전역 요약, 모델, effort, 세션 coverage와 상위 rollup 연결/watermark

## 트랜잭션 경계

- 동기화: 모든 소스 출력의 파싱·검증 완료 후 별칭, 감사, 원문 upsert, sync 기록을 하나의 `BEGIN IMMEDIATE`로 커밋한다.
- CCT 제시: stdout 쓰기 성공 후 해당 아카이브 행의 제시 시점을 커밋한다.
- 분석 완료: 세션 요약 insert와 제시된 pending 메시지 연결을 같은 트랜잭션에서 처리한다. 요약 저장이 실패하면 pending 상태를 유지한다.
- 요약 rollup: thread/global 요약 insert와 CTX watermark 이하 입력 요약 연결을 같은 트랜잭션에서 처리한다. 새로 생긴 더 큰 ID는 다음 rollup 입력으로 남긴다.

## 횡단 관심사

- 모든 외부 명령·DB 경계는 시작/성공/실패를 구조화 로그로 남긴다.
- 로그에는 본문, 표시 이름, 원본 ID, 연락처, 키, 토큰이나 DB 경로를 기록하지 않는다.
- 모델 분석기는 각 스레드의 최신 누적 요약, 최신 중앙 누적 요약과 새 pending CCT만 받는다.
- `last_presented_at_utc`는 파이프라인 출력 기록이며 메신저 읽음 상태로 해석하지 않는다.

## 위험

- 상태 DB의 복사·백업에는 원문과 실명 매핑이 포함된다.
- 카카오톡·Messages 스키마 변경은 외부 리더나 어댑터 파서를 깨뜨릴 수 있다.
- 세션 범위를 잘못 커밋하면 여러 세션이 한 요약에 연결될 수 있으므로 CCT의 다음 `S`를 exclusive end로 사용한다.
- 첨부파일 안의 일정 정보는 기본 추출에서 보이지 않으므로 표식만 남긴다.

## 관련 문서

**요구사항** — [REQUIREMENTS](REQUIREMENTS.md) · [Pipeline Requirements](docs/requirements/pipeline/README.md)

**설계** — [DESIGN-PIPE](docs/design/pipeline/DESIGN.md)

**계약** — [CCT](contracts/cct/CCT.md) · [CTX](contracts/context/CTX.md)

**ADR** — [ADR-0001](docs/adr/0001-rust-core-cli-adapters.md) · [ADR-0002](docs/adr/0002-cct-session-format.md) · [ADR-0003](docs/adr/0003-source-normalization-common-optimization.md) · [ADR-0004](docs/adr/0004-local-raw-archive-incremental-analysis.md)
