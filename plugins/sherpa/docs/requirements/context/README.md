---
id: DOM-CTX
title: Context Engine Requirements
status: draft
owner: maintainer
---
# Context Engine Requirements

## 목적과 범위

KakaoTalk·iMessage 대화와 연결된 메일에서 요청 범위의 개인 컨텍스트를 수집하고, 안전하고 재개 가능한 분석 상태를 관리한다. 로컬 엔진은 대화 원문을 보호된 상태에 보관하며 메일은 연결된 제공자 앱에서 최소 범위로 읽는다.

## 액터

- 소유자: 자신의 Mac 메시지를 동기화하고 분석 범위·비용·처리 상태를 확인한다.
- 분석기: 실명 없는 pending CCT와 최신 대화·전역 rollup을 소비하고 세션 요약과 새 rollup을 제출한다.
- 소스 CLI: 플랫폼별 메시지 표현을 읽기 전용으로 복원한다.

## 상태 모델

- `SYNCED`: 최적화 전 원문이 로컬 아카이브에 멱등 저장됨
- `PENDING`: 아직 세션 요약에 연결되지 않았거나 수정·지연 수집됨
- `PRESENTED`: pending CCT 생성과 stdout flush가 성공함
- `ANALYZED`: 파생 요약과 메시지 coverage가 같은 트랜잭션으로 연결됨
- `FAILED`: 경계 오류와 원인이 상위로 전달되고 이전 상태가 유지됨

## 비즈니스 규칙

- BR-CTX-001: 종료 시각은 배타적이며 모든 메시지는 UTC로 정규화한다.
- BR-CTX-002: 자기 자신은 대화마다 `A`, 상대는 최초 등장 순서대로 `B`부터 할당한다.
- BR-CTX-003: `schedule` 프로필의 세션 간격 기본값은 30분이다.
- BR-CTX-004: 첨부 전용 메시지는 종류 표식으로 보존하고 파일은 열거나 변환하지 않는다.
- BR-CTX-005: 연속 중복은 같은 대화·화자·최적화 본문이며 5분 이내일 때만 줄인다.
- BR-CTX-006: 분석 완료는 실제 제시된 pending 메시지를 덮는 요약 저장과 원자적으로 처리한다.
- BR-CTX-007: 원문이나 구조 메타데이터가 바뀐 메시지는 기존 분석 연결을 제거하고 pending으로 되돌린다.

## 기능 요구사항

- FR-CTX-EXTRACT-001: 고정된 읽기 전용 명령으로 기간 내 KakaoTalk와 iMessage를 추출해야 한다.
- FR-CTX-MAIL-001: 연결된 메일 제공자에서 날짜·발신자·제목·라벨 조건으로 필요한 범위만 읽어야 한다.
- FR-CTX-ARCHIVE-001: 최적화 전 본문, ID, 대화, 화자, UTC 시각, 종류와 첨부 메타데이터를 `(source, source_message_id)`로 멱등 upsert해야 한다.
- FR-CTX-ARCHIVE-002: insert/update/unchanged 건수와 마지막 수집·제시·분석 시점을 본문 없이 조회해야 한다.
- FR-CTX-ARCHIVE-003: 수정·지연 수집된 메시지를 다시 pending으로 만들어야 한다.
- FR-CTX-ARCHIVE-004: 전체 보관소 경로를 명시적으로 조회하고, 사용자 확인을 뜻하는 `--force`가 있을 때만 DB와 SQLite sidecar를 삭제해야 한다.
- FR-CTX-NORM-001: 소스 행을 공통 메시지로 정규화해야 한다.
- FR-CTX-OPT-001: `exact`와 `schedule` 프로필 및 변환 건수를 제공해야 한다.
- FR-CTX-OPT-002: 앱 차이는 adapter에서 끝내고 어휘·구조·출력 최적화는 공유해야 한다.
- FR-CTX-OPT-003: 강한 관계 신호 없이 답장 관계를 추론해서는 안 된다.
- FR-CTX-ALIAS-001: 실제 이름과 모델 별칭을 소유자 전용 SQLite에서 지속적으로 연결해야 한다.
- FR-CTX-ALIAS-002: 명시적 identity 요청에만 표시 이름을 반환하고 원본 source ID는 반환하지 않아야 한다.
- FR-CTX-EXPORT-001: CCT를 기본값으로, TSV와 compact JSON을 비교 형식으로 출력해야 한다.
- FR-CTX-EXPORT-002: `pending`은 분석 요약에 연결되지 않은 아카이브 메시지만 출력해야 한다.
- FR-CTX-BENCH-001: 아카이브에서 원문을 표시하지 않고 `o200k_base` 토큰과 대화 분포를 집계해야 한다.
- FR-CTX-SUMMARY-001: 세션·대화·전역 요약을 기간, 모델, 노력도와 함께 append-only로 저장해야 한다.
- FR-CTX-SUMMARY-002: 세션 요약은 제시된 pending 메시지 수와 집합 해시를 기록하고 그 메시지만 분석 완료로 연결해야 한다.
- FR-CTX-SUMMARY-003: 대화·전역 요약은 누적 rollup으로 저장하고 메시지 분석 상태를 변경하지 않아야 한다.
- FR-CTX-SUMMARY-004: 상위 rollup에 아직 포함되지 않은 요약과 단조 증가 watermark를 조회하고, rollup 저장과 watermark 이하 입력 연결을 원자적으로 처리해야 한다.
- FR-CTX-CANDIDATE-001: 약속·일정·할 일 가능성은 Planner 쓰기 전 `PlanningCandidate`로 제시해야 한다.
- FR-CTX-REPLY-001: KakaoTalk 답장은 정확히 하나의 대화, 본문 해시, 짧은 만료 시각에 승인 토큰을 결합해야 한다.
- FR-CTX-REPLY-002: 승인된 동일 본문만 한 번 전송하고 변경·만료·재사용을 거부해야 한다.

## 유스케이스

- [UC-CTX-001 기간 메시지 컨텍스트 내보내기](use-cases/UC-CTX-001-export-conversation-context.md)

## 인터페이스

CLI만 제공하며 외부 HTTP/UI는 없다. CCT 계약은 [CCT](../../../contracts/cct/CCT.md)다.

## 비기능 요구사항

- NFR-SEC-001: 소스 저장소와 파일을 변경하는 명령은 실행 경로에 없어야 한다.
- NFR-SEC-002: 새 상태 디렉터리·DB 권한은 각각 `0700`·`0600`이어야 한다. 기존 공유 디렉터리 권한을 바꾸지 말고 안전하지 않으면 거부해야 한다.
- NFR-PRI-001: 로그와 집계에는 본문, 이름, 원본 ID, 연락처, 인증 재료나 상태 DB 경로가 없어야 한다.
- NFR-PRI-002: 원문은 사용자 로컬 상태 DB에만 저장하며 Git·로그·보조 덤프에 복제해서는 안 된다.
- NFR-PRI-003: 원문은 자동 만료하지 않으며 플러그인 제거와 분리된 명시적 purge 수명주기를 제공해야 한다.
- NFR-REL-001: 잘못된 행은 조용히 누락하지 않고 비민감 오류로 실패해야 한다.
- NFR-REL-002: 분석 요약 저장 실패 시 메시지는 pending 상태를 유지해야 한다.
- NFR-REL-003: 세션 또는 대화 요약 저장 뒤 상위 rollup 전에 중단돼도 미반영 요약을 잃지 않아야 한다.
- NFR-PERF-001: 20,000개 메시지의 최적화·렌더링은 외부 추출 시간을 제외하고 단일 프로세스에서 처리 가능해야 한다.
- NFR-OPS-001: 로그 레벨은 `LOG_LEVEL`로 조절하고 기본값은 `warn`이어야 한다.
- NFR-OPS-002: 공개 명령은 `sherpa context` 이름 공간을 사용해야 한다.

## 용어집

- CCT: 대화·일자·세션·화자를 상속하는 줄 기반 모델 입력 형식.
- 로컬 상태: 원문, 실명 매핑, 정확 시각, 변환 감사와 분석 요약을 보관하는 소유자 전용 SQLite.
- pending: 아직 유효한 세션 요약에 연결되지 않은 아카이브 메시지.
- 제시: sherpa context가 선택 메시지의 CCT를 stdout에 성공적으로 기록한 상태. 메신저 읽음과 무관하다.
- 세션: 같은 날짜에서 이전 표시 메시지와의 간격이 설정값 이하인 연속 구간.

## 관련 문서

**설계** — [DESIGN-CTX](../../design/context/DESIGN.md)

**상위** — [ARCHITECTURE](../../../ARCHITECTURE.md) · [REQUIREMENTS](../../../REQUIREMENTS.md)
