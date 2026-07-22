---
status: draft
owner: maintainer
---
# Software Requirements Specification

## 목적

macOS 로컬 메시지를 외부 변경 없이 읽어 보호된 로컬 원문 아카이브에 동기화하고, 새 메시지만 스레드 단위 LLM 분석에 적합한 최소 토큰 표현으로 변환하는 요구사항의 정본이다.

## 범위

범위에는 KakaoTalk·iMessage 읽기 전용 동기화, 원문·처리 상태 보관, 공통 메시지 정규화, 프로필별 최적화, 별칭 관리, 증분 CCT/TSV/JSON 내보내기, 토큰 집계와 세션 요약 연결이 포함된다. 메시지 전송, 메신저 읽음 처리, UI 자동화, 첨부 본문 해석과 LLM API 호출 자체는 포함하지 않는다.

## 도메인 목록

- [DOM-PIPE Pipeline Requirements](docs/requirements/pipeline/README.md)

## 상태 정의

- `draft`: 구현 및 검증 중
- `review`: 수용 기준과 구현을 대조 중
- `approved`: 수용 기준을 자동 테스트로 충족
- `superseded`: 새 문서로 대체
- `deprecated`: 사용 중단 예정
- `archived`: 역사 기록

## 추적 규칙

요구사항과 테스트는 `FR/BR/NFR/UC/AC/TEST` ID로 연결한다. 설계 문서는 구현 경로를 반복하지 않고 해당 요구사항 ID를 역참조한다.

## 관련 문서

**상위** — [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [TESTING](TESTING.md)

**도메인** — [Pipeline Requirements](docs/requirements/pipeline/README.md)
