---
id: UC-PIPE-001
title: Export thread context
status: draft
owner: maintainer
acceptance_criteria: [AC-PIPE-001]
---
# UC-PIPE-001 기간 메시지 컨텍스트 내보내기

## 목표

소유자가 선택한 소스와 기간의 메시지를 읽기 전용으로 추출해 스레드별 별칭 CCT 또는 비교 형식으로 얻는다.

## 주 액터

소유자

## 사전조건

- 의존 CLI가 설치되고 해당 로컬 DB를 읽을 권한이 있다.
- KakaoTalk는 `kakaocli auth`가 사전에 성공했다.
- 시작 시각이 종료 시각보다 이르다.

## 트리거

소유자가 source, start, end, profile과 format을 지정해 `msgpipe export`를 실행한다.

## 기본 흐름

1. 시스템이 인자와 외부 CLI 버전을 검증한다.
2. 허용된 읽기 명령으로 종료 배타 기간의 행을 가져온다.
3. 행을 공통 메시지로 파싱하고 UTC 기준으로 정렬한다.
4. 선택 프로필로 최적화하며 변환 감사를 계산한다.
5. 보호된 SQLite 트랜잭션에서 별칭을 조회하거나 할당한다.
6. 선택 형식을 표준 출력에 쓰고 비민감 집계를 표준 오류 로그에 남긴다.

## 대안 흐름

### 의존 CLI 또는 권한 누락

1. 시스템은 실행하지 못한 경계와 복구 지침을 포함해 실패한다.
2. 부분 출력과 상태 커밋을 남기지 않는다.

### 지원하지 않는 소스 행

1. 시스템은 소스 종류와 행 번호만 포함한 오류를 기록한다.
2. 원문을 표시하지 않고 전체 실행을 실패시킨다.

### 집계만 요청

1. `benchmark`는 같은 파이프라인을 수행한다.
2. 본문 출력 없이 메시지/변환/토큰 분포만 반환한다.

## 사후조건

### 성공

원본 DB는 변경되지 않고, stdout에는 별칭 기반 파생 데이터만 있으며 중앙 상태에는 원문이 없다.

### 실패

원본 DB와 상태 DB 모두 부분 변경이 없고 실패 원인이 상위로 전달된다.

## 수용 기준

- AC-PIPE-001: 합성 KakaoTalk/iMessage fixture에서 같은 공통 메시지 의미와 결정적 별칭을 만들고, CCT 왕복·프로필 변환·권한·비노출 테스트를 모두 통과한다.

## 관련 문서

**요구사항 README** — [Message Pipeline Requirements](../README.md)

**설계** — [DESIGN-PIPE](../../../design/pipeline/DESIGN.md)

**계약** — [CCT](../../../../contracts/cct/CCT.md)
