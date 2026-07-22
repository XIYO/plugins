---
status: draft
owner: maintainer
---
# Test Strategy

## 범위

소스 파서, 읽기 전용 명령 구성, 정규화/최적화, 별칭 영속성, CCT 이스케이프 왕복, 토큰 집계와 CLI 오류 경계를 검증한다.

## 테스트 레벨

- 단위: 각 변환 규칙과 CCT codec
- 통합: 가짜 CLI fixture에서 공통 모델 및 SQLite 상태까지
- CLI: stdout/stderr 분리, 종료 코드, 콘텐츠 비노출
- 실데이터 회귀: 메시지 수·변환 수·토큰 수·해시만 비교

## 환경

Rust `1.97.1`, macOS, SQLite bundled 빌드를 기준으로 한다. 실데이터 테스트는 로컬에서만 실행하고 fixture로 커밋하지 않는다.

## 진입/종료 기준

진입 조건은 의존 CLI의 버전과 읽기 권한 확인이다. 종료 조건은 `fmt`, 경고를 오류로 처리한 `clippy`, 전체 테스트 통과 및 실데이터 출력에 본문·이름이 없음을 확인하는 것이다.

## 추적성 방식

- `TEST-UNIT-PIPE-001` -> `AC-PIPE-001`: CCT/별칭/변환 단위 테스트
- `TEST-INT-PIPE-001` -> `AC-PIPE-001`: fixture 기반 양쪽 소스 통합 테스트
- `TEST-SEC-PIPE-001` -> `NFR-PRI-001`~`002`, `NFR-SEC-001`: 상태 파일 권한, 원문 비저장과 파생 맥락 scope 검증
- `TEST-PERF-PIPE-001` -> `NFR-PERF-001`: `o200k_base` 집계와 기준 형식 대비 절감률

## 리포팅

CI와 로컬 명령은 테스트 이름, 건수, 시간만 출력한다. 실패 fixture는 합성 데이터만 사용하며 실데이터 원문을 스냅샷으로 남기지 않는다.

## 관련 문서

**상위** — [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [REQUIREMENTS](REQUIREMENTS.md)

**설계** — [DESIGN-PIPE](docs/design/pipeline/DESIGN.md)
