---
id: DOM-PIPE
title: Message Pipeline Requirements
status: draft
owner: maintainer
---
# Message Pipeline Requirements

## 목적과 범위

두 로컬 메시지 소스를 동일한 추출·최적화·내보내기 흐름으로 처리한다. 이 도메인은 LLM 호출 이전까지 책임지며 실제 일정 판단은 책임지지 않는다.

## 액터

- 소유자: 자신의 Mac 메시지 기록을 내보내고 비용을 검증한다.
- 분석기: 실명 없이 스레드별 CCT를 소비한다.
- 소스 CLI: 암호화·플랫폼별 메시지 표현을 읽기 전용으로 복원한다.

## 상태 모델

- `EXTRACTED`: 공통 모델로 파싱됨
- `OPTIMIZED`: 프로필 변환과 감사 코드가 확정됨
- `ALIASED`: 중앙 상태에 스레드/화자 별칭이 할당됨
- `EXPORTED`: 선택 형식이 표준 출력에 기록됨
- `FAILED`: 경계 오류와 원인이 상위로 전달됨

## 비즈니스 규칙

- BR-PIPE-001: 종료 시각은 배타적이며 모든 메시지는 UTC로 정규화한 뒤 출력 시간대로 변환한다.
- BR-PIPE-002: 자기 자신은 스레드마다 `A`, 상대는 최초 등장 순서대로 `B`부터 할당한다.
- BR-PIPE-003: `schedule` 프로필의 세션 간격 기본값은 30분이다.
- BR-PIPE-004: 첨부 전용 메시지는 종류 표식으로 보존하고 파일은 열거나 변환하지 않는다.
- BR-PIPE-005: 연속 중복은 같은 스레드·화자·최적화 본문이며 5분 이내일 때만 하나로 줄인다.

## 기능 요구사항

- FR-PIPE-EXTRACT-001: 시스템은 고정된 읽기 전용 명령으로 기간 내 KakaoTalk와 iMessage를 추출해야 한다.
- FR-PIPE-NORM-001: 시스템은 소스 메시지를 공통 ID, 스레드, 화자, UTC 시각, 종류, 본문, 첨부 메타데이터로 정규화해야 한다.
- FR-PIPE-OPT-001: 시스템은 `exact`와 `schedule` 프로필을 제공하고 각 변환 건수를 보고해야 한다.
- FR-PIPE-OPT-002: 앱별 출력 차이는 adapter 정규화에서 끝나야 하며 어휘 치환·대화 구조·출력 구조 최적화는 소스 독립 단계로 재사용해야 한다.
- FR-PIPE-OPT-003: 약한 관계 필드가 일반 연속 메시지에도 쓰이는 소스에서는 강한 관계 신호가 없으면 답장으로 추론해서는 안 된다.
- FR-PIPE-ALIAS-001: 시스템은 실제 이름과 모델용 별칭을 소유자 전용 SQLite에서 지속적으로 연결해야 한다.
- FR-PIPE-ALIAS-002: 시스템은 명시적인 단일 스레드 identity 요청에서만 방/화자 표시 이름을 반환하고 원본 source ID는 반환하지 않아야 한다.
- FR-PIPE-EXPORT-001: 시스템은 CCT를 기본값으로, TSV와 compact JSON을 비교·디버그 형식으로 출력해야 한다.
- FR-PIPE-BENCH-001: 시스템은 원문을 표시하지 않고 `o200k_base` 토큰 수와 스레드 분포를 집계해야 한다.
- FR-PIPE-BENCH-002: 시스템은 실명 없는 스레드 manifest를 제공하고 같은 별칭으로 단일 스레드만 결정적으로 내보낼 수 있어야 한다.
- FR-PIPE-CONTEXT-001: 시스템은 스레드별·전역 파생 분석 요약을 기간, 모델, 노력도와 함께 append-only 중앙 상태에 저장하고 최신 값을 조회할 수 있어야 한다.

## 유스케이스

- [UC-PIPE-001 기간 메시지 컨텍스트 내보내기](use-cases/UC-PIPE-001-export-thread-context.md)

## 인터페이스

CLI만 제공하며 외부 HTTP/UI 인터페이스는 없다. CCT의 캐노니컬 계약은 [CCT](../../../contracts/cct/CCT.md)다.

## 비기능 요구사항

- NFR-SEC-001: 소스 저장소와 파일을 변경하는 명령은 실행 경로에 존재해서는 안 된다.
- NFR-PRI-001: 로그와 집계에는 본문, 이름, 원본 ID, 연락처, 인증 재료가 포함되어서는 안 된다.
- NFR-PRI-002: 중앙 상태는 채팅 원문을 저장하지 않으며 분석 요약 저장은 명시적인 `context put` 입력에서만 허용해야 한다.
- NFR-REL-001: 잘못된 행은 조용히 누락하지 않고 소스와 행 번호를 포함한 비민감 오류로 실패해야 한다.
- NFR-PERF-001: 20,000개 메시지의 최적화·렌더링은 외부 추출 시간을 제외하고 단일 프로세스에서 처리 가능해야 한다.
- NFR-OPS-001: 로그 레벨은 `LOG_LEVEL`로 조절하고 기본값은 `warn`이어야 한다.

## 용어집

- CCT: Compact Conversation Transcript. 스레드·일자·세션·화자를 상속하는 줄 기반 모델 입력 형식.
- 중앙 상태: 실명 매핑, 정확 시각, 본문 해시, 변환 감사를 보관하는 로컬 SQLite.
- 세션: 같은 날짜에서 이전 메시지와의 간격이 설정값 이하인 연속 구간.

## 관련 문서

**설계** — [DESIGN-PIPE](../../design/pipeline/DESIGN.md)

**상위** — [ARCHITECTURE](../../../ARCHITECTURE.md) · [REQUIREMENTS](../../../REQUIREMENTS.md)
