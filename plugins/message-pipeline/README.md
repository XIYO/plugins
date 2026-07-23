# msgpipe

`msgpipe`는 macOS에 동기화된 KakaoTalk와 iMessage 기록을 읽기 전용으로 가져와 소유자 전용 로컬 SQLite에 원문을 보관하고, 아직 요약되지 않은 메시지만 토큰 절약형 CCT로 준비하는 Rust CLI다.

> **단계:** Experimental · **플랫폼:** macOS · **원본 접근:** 읽기 전용

## 설치

[XIYO 플러그인 마켓플레이스](../../README.ko.md)를 추가하고 `message-pipeline@xiyo`를 설치한다.

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add message-pipeline@xiyo
```

설치한 스킬을 불러오려면 새 Codex 작업을 시작한다. 첫 사용 시 `msgpipe`가 없으면 번들 스킬이 플러그인 내부 설치기를 실행할 수 있다.

## 선행 조건

- macOS와 Rust 도구 모음
- KakaoTalk용 읽기 전용 `kakaocli` 또는 iMessage용 읽기 전용 `imsg`
- 소스 리더가 요구하는 macOS 전체 디스크 접근 권한
- `~/.cargo/bin`을 포함한 `PATH`

소스 리더는 자동 설치하지 않는다. 현재 KakaoTalk·iMessage 연동에는 별도 준비 과정이 있으므로 이 플러그인은 Experimental이다.

## 첫 사용

원문을 내보내기 전에 읽기 전용 진단부터 실행한다.

```text
iMessage와 KakaoTalk 소스 리더 설치 상태만 확인해줘. 메시지 내용은 출력하거나 분석하지 마.
```

수동 확인:

```bash
msgpipe doctor imessage
msgpipe doctor kakao
```

동기화는 원문을 표준 출력에 표시하지 않는다. 분석을 명시적으로 요청한 경우에만 미분석 메시지를 CCT로 준비해 모델 컨텍스트에 넣는다.

## 데이터 경계

- KakaoTalk와 Messages 원본 DB는 수정하지 않는다.
- 동기화한 원문은 소유자 전용 로컬 SQLite에 저장하며, 파일 자체는 애플리케이션 수준에서 암호화하지 않는다. FileVault가 켜진 소유자 기기에서만 사용한다.
- 런타임은 데이터를 스스로 업로드하지 않지만, 분석하도록 선택한 CCT 본문은 Codex 또는 Claude Code에 설정된 모델 제공자가 처리할 수 있다.
- 로그와 버그 제보에는 메시지 본문, 이름, 연락처, 원본 식별자, 인증 재료, 로컬 DB 경로를 포함하지 않는다.

## 상태

v0.2는 읽기 전용 소스 어댑터, 원문 아카이브, 멱등 동기화, 공통 옵티마이저, CCT, 토큰 벤치마크와 증분 분석 상태를 구현한다.

## 설계 원칙

- KakaoTalk는 검증된 `kakaocli` SQLCipher 리더를, iMessage는 `imsg` 리더를 사용한다. 원본 DB를 수정하지 않는다.
- `sync`만 외부 리더를 호출한다. `export`, `pending`, `benchmark`는 로컬 아카이브만 읽는다.
- 원문, 정확 시각, 원본 식별자, 이름과 첨부 메타데이터는 별칭·분석 상태와 함께 SQLite에 저장한다.
- 동일 메시지는 `(source, source_message_id)`로 upsert한다. 본문이나 메타데이터가 바뀌면 분석 연결과 마지막 제시 시점을 지워 다시 pending으로 만든다.
- `pending`은 세션 요약에 연결되지 않은 메시지만 내보낸다. `context put session`이 성공해야 해당 범위가 분석 완료된다.
- `schedule` 프로필은 무의미 반응 제거, 확인 반응·URL·반복 축약, 근접 중복 제거와 30분 세션화를 수행한다.
- 모델 입력은 스레드 `K001`·`I001`, 화자 `A`·`B` 같은 별칭과 [CCT](contracts/cct/CCT.md)를 기본으로 한다.

## 증분 처리

```bash
msgpipe doctor kakao
msgpipe doctor imessage

msgpipe sync kakao --start 2026-06-01 --end 2026-07-23
msgpipe sync imessage --start 2026-06-01 --end 2026-07-23

msgpipe status kakao
msgpipe benchmark kakao --start 2026-06-01 --end 2026-07-23
msgpipe pending kakao --start 2026-06-01 --end 2026-07-23 --thread K001

printf '%s' '<session summary>' | msgpipe context put session \
  --thread K001 \
  --start 2026-07-10T09:00:00+09:00 \
  --end 2026-07-10T11:30:00+09:00

msgpipe context inputs thread --thread K001
printf '%s' '<cumulative thread summary>' | msgpipe context put thread \
  --thread K001 --through-context-id 42 \
  --start 2026-06-01 --end 2026-07-23

msgpipe context inputs global
printf '%s' '<cumulative global summary>' | msgpipe context put global \
  --through-context-id 51 --start 2026-06-01 --end 2026-07-23

msgpipe context get thread --thread K001
msgpipe context get global
msgpipe identities K001
```

`status`는 원문 없이 스레드별 보관 건수, pending 건수, 첫·마지막 메시지 시각, 마지막 수집·CCT 제시·분석 시각을 출력한다. `last_presented_at_utc`는 msgpipe 출력 시각이며 메신저의 읽음 상태가 아니다.

`context put session`은 stdin의 파생 요약과 해당 기간에서 실제 CCT로 제시된 pending 메시지의 분석 완료 표시를 하나의 트랜잭션으로 저장한다. 실패하면 메시지는 pending으로 남으므로 다시 분석할 수 있다. 수정·지연 동기화된 메시지도 자동으로 pending으로 돌아온다.

`context inputs`는 아직 상위 rollup에 포함되지 않은 요약을 [CTX](contracts/context/CTX.md)로 출력한다. 헤더의 `through`를 `context put thread|global --through-context-id`에 넘기면 요약 저장과 입력 연결이 원자적으로 처리된다. 작업이 중단돼도 미반영 요약은 다음 `context inputs`에 다시 나타난다.

`context put thread`는 기존 스레드 누적 요약과 새 세션 요약을 합친 최신 rollup을 저장한다. `context put global`은 스레드별 변화·결정·미결 항목을 합친 중앙 rollup을 저장한다. 둘 다 메시지 분석 상태를 변경하지 않으므로 다음 분석에는 최신 thread/global rollup과 새 pending 원문만 첨부하면 된다.

## 로컬 데이터 보호

기본 상태 경로는 운영체제의 XIYO/msgpipe 로컬 앱 데이터 디렉터리 아래 `state.sqlite3`다. 디렉터리 권한은 `0700`, 파일은 `0600`으로 매 실행 교정한다. SQLite는 애플리케이션 수준에서 암호화하지 않으므로 FileVault가 활성화된 소유자 장치에만 두며, 복사와 백업에도 채팅 원문이 포함된다고 취급한다.

로그에는 메시지 본문, 표시 이름, 연락처, 원본 ID, 인증 재료나 상태 DB 경로를 남기지 않는다. 실제 데이터 회귀 검증도 집계와 해시만 출력한다.

## 관련 문서

**상위** — [ARCHITECTURE](ARCHITECTURE.md) · [REQUIREMENTS](REQUIREMENTS.md) · [TESTING](TESTING.md)

**요구사항/설계** — [Pipeline Requirements](docs/requirements/pipeline/README.md) · [DESIGN-PIPE](docs/design/pipeline/DESIGN.md) · [CTX](contracts/context/CTX.md)

**결정** — [ADR-0001](docs/adr/0001-rust-core-cli-adapters.md) · [ADR-0002](docs/adr/0002-cct-session-format.md) · [ADR-0003](docs/adr/0003-source-normalization-common-optimization.md) · [ADR-0004](docs/adr/0004-local-raw-archive-incremental-analysis.md)

**릴리스** — [CHANGELOG](CHANGELOG.md)
