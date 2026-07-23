---
id: UC-CTX-001
title: Sync and export pending conversation context
status: draft
owner: maintainer
acceptance_criteria: [AC-CTX-001]
---
# UC-CTX-001 원문 동기화와 pending 대화 분석

## 목표

소유자가 선택한 소스·기간의 원문을 로컬 상태에 한 번 동기화하고, 아직 요약되지 않은 메시지만 별칭 CCT로 분석기에 전달한 뒤 세션 요약을 커밋한다.

## 사전조건

- 의존 CLI가 설치되고 로컬 원본 DB 읽기 권한이 있다.
- KakaoTalk는 `kakaocli auth`가 성공했다.
- 시작 시각이 종료 시각보다 이르다.

## 기본 흐름

1. 소유자가 `sherpa context sync`에 source, start, end를 지정한다.
2. 시스템이 허용된 읽기 명령으로 행을 가져와 전부 파싱·검증한다.
3. 별칭, 최적화 전 원문과 sync 실행을 보호 SQLite에 멱등 커밋한다.
4. 소유자가 `status`·`benchmark`로 본문 없는 범위와 비용을 확인한다.
5. 분석기가 `pending --thread`로 한 대화의 미분석 CCT만 받는다.
6. stdout 성공 후 시스템이 해당 메시지의 마지막 제시 시점을 기록한다.
7. 분석기가 각 CCT 세션의 파생 요약을 `summary put session`으로 제출한다.
8. 시스템이 요약, 메시지 수·집합 해시와 분석 연결을 하나의 트랜잭션으로 커밋한다.
9. 분석기가 `summary inputs thread`로 미반영 세션 요약과 watermark를 받고, 기존 대화 rollup과 합쳐 `summary put thread --through-context-id`로 저장한다.
10. 시스템이 conversation rollup과 watermark 이하 세션 요약의 연결을 원자적으로 커밋한다.
11. 분석기가 별칭별 최신 미반영 대화 rollup을 `summary inputs global`로 받아 기존 전역 rollup에 합치고 같은 방식으로 저장한다.
12. 다음 분석은 최신 대화·전역 rollup과 새 pending 메시지만 사용한다.

## 대안 흐름

### 의존 CLI·권한·파싱 실패

부분 원문이나 sync 실행을 남기지 않고 비민감 오류로 실패한다.

### 분석 또는 요약 커밋 실패

분석 연결을 만들지 않고 메시지를 pending으로 유지한다.

### 상위 rollup 전 중단

저장된 세션·대화 요약은 미반영 상태로 남고 다음 `summary inputs`에서 다시 출력된다. 출력 뒤 새로 생성된 context ID는 이전 watermark 커밋에 포함하지 않는다.

### 메시지 수정·지연 수집

원문 upsert가 변경을 감지하고 기존 분석 연결·제시 시점을 제거해 다시 pending으로 만든다.

### 집계만 요청

`benchmark`는 소스 CLI를 호출하지 않고 아카이브만 읽어 본문 없는 토큰 분포를 반환한다.

## 사후조건

### 성공

원본 메신저 DB는 변경되지 않는다. sherpa context 상태 DB에는 원문과 처리 상태가 있고, 모델 출력에는 별칭 CCT만 있으며 다음 분석은 과거 원문을 재첨부하지 않는다.

### 실패

원본 DB는 변경되지 않고, 트랜잭션 경계 전 상태가 유지된다.

## 수용 기준

- AC-CTX-001: 합성 fixture에서 원문 round-trip, 멱등 sync, 변경 시 pending 복귀, 제시 전 요약 거부, 요약/분석 연결 원자성, CCT와 별칭 결정성, 파일 권한을 검증한다.

## 관련 문서

**요구사항** — [Context Engine Requirements](../README.md)

**설계** — [DESIGN-CTX](../../../design/context/DESIGN.md)

**계약** — [CCT](../../../../contracts/cct/CCT.md)
