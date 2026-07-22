# msgpipe

`msgpipe`는 macOS에 동기화된 KakaoTalk와 iMessage 기록을 읽기 전용으로 추출하고, 대화 의미를 유지하면서 LLM 입력 토큰을 줄이는 Rust CLI다. 채팅 원문은 메모리에서만 처리하고, 모델용 출력에는 `K001`·`I001` 같은 스레드 별칭과 `A`·`B` 같은 화자 별칭을 사용한다.

현재 목표는 실제 일정 분석을 실행하는 것이 아니라, 두 메시지 소스를 동일한 모델로 정규화하고 검증 가능한 CCT 형식으로 내보내는 것이다.

## 상태

v0.1 CLI, CCT 계약, 읽기 전용 어댑터, 공통 옵티마이저, 보호 상태 저장소를 구현했다. 실제 데이터 회귀 검증은 본문을 출력하거나 저장하지 않고 집계값만 사용한다.

## 설계 원칙

- KakaoTalk는 검증된 `kakaocli` SQLCipher 리더를, iMessage는 공식 `imsg` 리더를 어댑터로 사용한다.
- 앱별 JSON·메시지 타입·관계 신호는 adapter가 공통 모델로 바꾸고, 단어 치환과 대화 구조 최적화는 소스와 무관한 공통 엔진으로 한 번만 구현한다.
- `exact` 프로필은 메시지와 분 단위 시각을 보존한다.
- `schedule` 프로필은 무의미한 반응 제거, 확인 반응 축약, URL 축약, 반복 축약, 근접 중복 제거와 30분 세션 시각을 사용한다.
- 첨부 전용 메시지는 삭제하지 않고 `@image`·`@file` 같은 표식으로 남긴다. 파일을 열거나 변환하는 작업은 별도 승인된 2단계다.
- JSON은 로컬 파서 경계와 디버그 내보내기에만 사용한다. 모델 입력 기본값은 [CCT 계약](contracts/cct/CCT.md)이다.

## 예정 CLI

```bash
msgpipe doctor kakao
msgpipe doctor imessage
msgpipe export kakao --start 2026-06-01 --end 2026-07-23 --profile schedule
msgpipe export imessage --start 2026-06-01 --end 2026-07-23 --profile schedule
msgpipe benchmark kakao --start 2026-06-01 --end 2026-07-23
msgpipe export kakao --start 2026-06-01 --end 2026-07-23 --thread K001
msgpipe identities K001
msgpipe context put thread --thread K001 --start 2026-06-01 --end 2026-07-23 < summary.md
msgpipe context get global
```

`benchmark`의 `thread_manifest`는 별칭별 메시지 수·기간·CCT 토큰만 제공한다. 이 목록으로 분석 큐를 만든 뒤 `export --thread <alias>`로 한 스레드씩 전달한다. `export`는 표준 출력으로만 원문 파생 데이터를 내보내며 로그는 표준 오류로 분리한다.

분석 후보를 실제 사람/방으로 되돌릴 때만 `identities <thread-alias>`를 명시적으로 호출한다. 이 출력은 방 표시 이름과 화자 별칭 매핑을 포함하지만 원본 DB 식별자는 포함하지 않는다.

`context put`은 나중의 스레드 분석기가 만든 파생 요약을 append-only로 중앙 SQLite에 저장한다. 기본 분석 메타데이터는 `gpt-5.6-terra`와 `medium`이며, `context get/list`로 최신 요약 또는 본문 없는 이력을 읽는다. 이 명령은 모델을 호출하지 않는다.

## 관련 문서

**상위** — [ARCHITECTURE](ARCHITECTURE.md) · [REQUIREMENTS](REQUIREMENTS.md) · [TESTING](TESTING.md)

**요구사항/설계** — [Pipeline Requirements](docs/requirements/pipeline/README.md) · [DESIGN-PIPE](docs/design/pipeline/DESIGN.md)

**결정** — [ADR-0001 Rust core와 CLI adapter](docs/adr/0001-rust-core-cli-adapters.md) · [ADR-0002 CCT 세션 형식](docs/adr/0002-cct-session-format.md) · [ADR-0003 최적화 단계 경계](docs/adr/0003-source-normalization-common-optimization.md)
