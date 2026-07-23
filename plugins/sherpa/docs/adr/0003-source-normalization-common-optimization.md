---
id: ADR-0003
title: Source normalization and common optimization boundaries
status: accepted
date: 2026-07-22
deciders: [owner, maintainer]
supersedes: []
---
# ADR-0003 소스 정규화와 공통 최적화 경계

## 컨텍스트

KakaoTalk는 일반 텍스트의 `attachment` 열에도 멘션·링크·봇 메타데이터를 넣는다. iMessage의 `reply_to_guid/reply_to_text`도 실제 인라인 답장 외 연속 SMS association에 쓰인다. 앱별 원본 필드를 공통 최적화기가 직접 해석하면 동일 규칙이 중복되고 약한 신호를 잘못 일반화한다.

## 결정

파이프라인을 네 책임으로 나눈다.

1. source adapter가 앱별 JSON, type, 첨부와 강한 관계 신호를 공통 메시지로 정규화한다.
2. lexical replacer가 `ㅋ/ㅠ`, 긍정·부정·질문 반응, 반복, URL 같은 앱 독립 채팅 표현을 치환한다.
3. conversation optimizer가 연속 중복, 검증된 답장 관계, 반복 템플릿 같은 메시지 간 구조를 판단한다.
4. exporter가 CCT 세션·필드 상속처럼 출력 형식에만 해당하는 압축을 담당한다.

소스별 최적화 엔진을 따로 만들지 않는다. 앱 고유 신호는 adapter가 의미 있는 공통 값으로 바꿨을 때만 공통 엔진에 전달한다.

## 대안

- 앱마다 완전한 optimizer 구현: 빠르게 시작할 수 있지만 규칙과 감사 코드가 갈라져 결과 비교가 어렵다.
- 하나의 optimizer에서 원본 JSON 키 분기: 파일 수는 적지만 소스 계약과 의미 변환이 결합된다.
- exporter가 원본을 직접 압축: 토큰은 줄일 수 있으나 다른 형식과 분석 단계에서 재사용할 수 없다.

## 결과와 영향

새 메시지 앱은 adapter만 추가하면 공통 리플레이서·구조 최적화·exporter를 재사용한다. 앱 스키마 변경은 adapter fixture에서 잡힌다. 템플릿 축약과 답장 연결은 공통 모델에 충분히 강한 의미가 추가된 뒤 conversation optimizer에만 구현한다.

## 관련 문서

**설계** — [DESIGN-CTX](../design/context/DESIGN.md)

**상위** — [ARCHITECTURE](../../ARCHITECTURE.md)
