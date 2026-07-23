---
id: ADR-0001
title: Rust core with read-only CLI adapters
status: accepted
date: 2026-07-22
deciders: [owner, maintainer]
supersedes: []
---
# ADR-0001 Rust core와 읽기 전용 CLI adapter

## 컨텍스트

KakaoTalk DB는 SQLCipher 키 파생과 변하는 사설 스키마가 필요하고, 최신 iMessage 본문은 `attributedBody`, 편집, 답장, 반응을 복원해야 한다. 두 문제를 동시에 Rust로 재구현하면 검증 범위와 데이터 손실 위험이 커진다.

## 결정

파이프라인, 모델, 최적화, 상태, 내보내기는 Rust로 구현한다. 첫 버전의 소스 경계는 이미 검증된 `kakaocli`와 공식 `imsg`를 셸 없이 직접 실행하는 adapter로 둔다. 허용 명령은 KakaoTalk의 고정 `query`, iMessage의 `chats/history/stats`로 제한한다.

## 대안

- 전부 Rust로 직접 읽기: 단일 바이너리지만 암호화와 Apple 내부 직렬화의 중복 구현 위험이 크다.
- Python 파이프라인: 실험은 빠르지만 배포·권한·타입 경계와 장기 유지보수가 불리하다.
- 각 CLI 출력 스크립트만 유지: 공통 계약, 감사, 테스트와 상태 관리가 분산된다.

## 결과와 영향

핵심 변환은 빠르고 테스트 가능한 단일 Rust 애플리케이션이 된다. 외부 CLI 버전 호환성 검사가 필요하며, 향후 native extractor는 같은 trait 뒤에 선택적으로 추가할 수 있다.

## 관련 문서

**설계** — [DESIGN-CTX](../design/context/DESIGN.md)

**상위** — [ARCHITECTURE](../../ARCHITECTURE.md)
