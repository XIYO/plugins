---
id: ADR-0002
title: CCT session format for model input
status: accepted
date: 2026-07-22
deciders: [owner, maintainer]
supersedes: []
---
# ADR-0002 모델 입력용 CCT 세션 형식

## 컨텍스트

동일 데이터의 compact JSON, TSV, TOON과 상속형 대화 포맷을 `o200k_base`로 비교했다. JSON·TOON은 스레드/필드명이 반복되어 불리했고, 스레드·일자·화자 상속형 CCT가 가장 작았다. 30분 세션은 분 단위 행 형식보다 추가 절감되면서 일정의 기준 날짜와 대략적인 대화 시각을 유지했다.

## 결정

모델 입력 기본 형식은 버전이 있는 CCT3로 한다. 스레드 `T`, 날짜 `D`, 세션 시작 `S`, 화자/본문 행을 사용하고 빈 화자는 직전 화자를 상속한다. `schedule` 기본 세션 간격은 30분이며 `exact`는 CCT2 분 단위 행을 사용한다. 원문, 실제 이름, 정확한 시각과 분석 상태는 소유자 전용 로컬 SQLite에 남긴다.

## 대안

- compact JSON array: 범용 파싱은 쉽지만 구두점과 반복 필드 토큰이 크다.
- TSV: 단순하지만 스레드·시각·화자를 매 행 반복한다.
- TOON: 표 형식에는 유리하지만 시간순 다중 스레드에서 반복 필드를 제거하지 못했다.
- 60분 이상 세션: 조금 더 작지만 시간 해석 오차가 커진다.

## 결과와 영향

분석 프롬프트는 CCT 상속/이스케이프 규칙을 한 번 알아야 한다. 세션 안의 정확한 분은 모델에서 생략되므로 후보 결과는 중앙 상태의 원본 시각과 다시 결합해야 한다. 첨부 표식은 보존해 별도 2단계 필요성을 알린다.

## 관련 문서

**설계** — [DESIGN-PIPE](../design/pipeline/DESIGN.md)

**계약** — [CCT](../../contracts/cct/CCT.md)

**상위** — [ARCHITECTURE](../../ARCHITECTURE.md)
