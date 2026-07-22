---
status: draft
owner: maintainer
---
# Software Architecture Description

## 요구사항 개요

시스템은 `UC-PIPE-001`의 기간 기반 내보내기를 수행한다. 외부 CLI의 이질적인 JSON을 공통 메시지로 변환한 뒤, 원문 비저장과 별칭 분리를 지키면서 토큰 효율적인 표현을 만든다.

## 품질 목표

1. 원본 메시지 저장소를 절대 변경하지 않는 안전성
2. 동일 입력·설정에 대해 동일 결과를 내는 결정성
3. 모델 입력과 실명·원본 식별자 매핑의 분리
4. 변환별 손실과 토큰 절감량을 집계로 검증하는 추적성

## 제약

- macOS 로컬 환경을 대상으로 한다.
- KakaoTalk SQLCipher 키 파생과 iMessage `attributedBody` 복원은 초기 버전에서 재구현하지 않는다.
- 평문 메시지는 프로세스 메모리와 명시적인 표준 출력만 통과한다.
- 기본 시간대는 `Asia/Seoul`, 범위의 종료 시각은 배타적이다.

## 컨텍스트

```text
KakaoTalk DB -> kakaocli --read-only --+
                                       +-> NormalizedMessage -> Optimizer -> CCT/TSV/JSON stdout
Messages chat.db -> imsg read commands-+                         |
                                                                 +-> aggregate benchmark
                                                                 +-> protected SQLite alias/audit state
```

## 솔루션 전략

추출기는 허용된 외부 명령만 인자 배열로 실행한다. 파서는 소스 JSON을 즉시 공통 모델로 바꾸고, 옵티마이저는 프로필별 순수 함수로 동작한다. 상태 저장소는 본문 대신 SHA-256과 변환 코드만 기록하고 별칭을 재사용한다.

## 빌딩 블록

- `extract`: 외부 CLI 실행, 앱별 JSON·메시지 타입·첨부/관계 신호 파싱, 읽기 전용 명령 allowlist
- `model`: 소스 독립 메시지·첨부 메타데이터
- `optimizer::replacer`: 앱과 무관한 채팅 어휘·반응·반복·URL·첨부 표식 치환
- `optimizer::structure`: 메시지 간 중복과 강한 관계 신호를 이용한 대화 구조 최적화
- `state`: SQLite 별칭·정확 시각·본문 해시·append-only 파생 분석 맥락 저장
- `export`: CCT3/CCT2 세션·필드 상속, TSV, compact JSON 렌더링
- `benchmark`: `o200k_base` 기반 형식별 토큰 집계

## 런타임

추출과 파싱이 성공한 뒤에만 상태 트랜잭션을 연다. 별칭 할당과 감사 인덱스 기록은 하나의 트랜잭션으로 커밋한다. 렌더링 실패 시 원본 저장소에는 영향이 없고, 상태 트랜잭션은 롤백한다.

## 배포

단일 Rust 바이너리로 빌드한다. `kakaocli`와 `imsg`는 별도 검증·설치된 실행 파일이며 경로를 명시하거나 `PATH`에서 찾는다.

## 횡단 관심사

- 모든 외부 명령·DB 경계는 시작/성공/실패를 구조화 로그로 남긴다.
- 로그에는 본문, 표시 이름, 원본 ID, 연락처, 키, 토큰을 기록하지 않는다.
- 상태 디렉터리는 `0700`, SQLite 파일은 `0600`으로 강제한다. `analysis_context`에는 원문이 아니라 분석기가 명시적으로 제출한 파생 요약만 저장한다.
- 모델 분석기는 별도 단계이며 각 스레드 CCT와 중앙 요약만 받는다.

## 위험

- 카카오톡 DB 스키마 변경은 고정 SELECT 파서를 깨뜨릴 수 있다.
- 반응 축약은 일반 동의와 일정 확정을 혼동할 수 있어 변환 감사를 유지한다.
- 첨부파일 안의 일정 정보는 기본 추출에서 보이지 않으므로 표식을 남기고 별도 2단계로 다룬다.

## 관련 문서

**요구사항** — [REQUIREMENTS](REQUIREMENTS.md) · [Pipeline Requirements](docs/requirements/pipeline/README.md)

**설계** — [DESIGN-PIPE](docs/design/pipeline/DESIGN.md)

**ADR** — [ADR-0001](docs/adr/0001-rust-core-cli-adapters.md) · [ADR-0002](docs/adr/0002-cct-session-format.md) · [ADR-0003](docs/adr/0003-source-normalization-common-optimization.md)
