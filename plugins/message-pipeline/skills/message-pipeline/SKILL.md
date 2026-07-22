---
name: message-pipeline
description: 하나의 설치형 스킬로 macOS의 KakaoTalk·iMessage 기록을 읽기 전용 동기화하고 원문을 소유자 전용 로컬 SQLite에 멱등 보관한다. msgpipe로 미분석 메시지만 공통 정규화·채팅 표현 치환·대화 구조 최적화 후 스레드·세션별 CCT로 내보내며, 마지막 수집·제시·분석 시점과 세션 요약·스레드/전역 누적 맥락을 관리한다. "카톡/문자 읽어줘", "대화 동기화", "새 메시지만 분석", "언제까지 봤지", "카톡/문자 토큰 줄여줘", "스레드별 일정 분석", "CCT", KakaoTalk와 iMessage 통합 분석 요청에 사용한다.
---

# 메시지 파이프라인

이 스킬 하나가 KakaoTalk와 iMessage를 모두 지원한다. 앱 차이는 소스 어댑터에서 끝내고, 원문 아카이브 이후에는 같은 옵티마이저·CCT·증분 분석 절차를 사용한다.

- KakaoTalk 인증·권한·`kakaocli` 복구가 필요하면 [KakaoTalk 소스](references/kakaotalk.md)를 읽는다.
- iMessage 권한·`imsg` 복구가 필요하면 [iMessage 소스](references/imessage.md)를 읽는다.
- 두 소스를 함께 다룰 때만 두 문서를 모두 읽는다.

## 실행 위치

- 이 파일에서 두 디렉터리 위를 플러그인 루트로 해석한다. marketplace 캐시 밖 경로를 가정하지 않는다.
- `msgpipe`가 없으면 플러그인 루트에서 설치한다.

  ```bash
  bash <plugin-root>/scripts/install-runtime.sh
  ```

- 구현을 수정할 때만 플러그인 루트의 `ARCHITECTURE.md`와 `contracts/cct/CCT.md`를 읽는다.

## 저장과 개인정보 경계

- KakaoTalk·Messages 원본 DB와 소스 CLI는 계속 읽기 전용이다. `sync`가 쓰는 대상은 별도의 msgpipe 로컬 SQLite뿐이다.
- `sync`는 최적화·축약 전 본문, 정확 시각, 원본 ID, 이름, 첨부 메타데이터를 공통 원문 모델로 보관한다. 동일 `(source, source_message_id)`는 멱등 upsert한다.
- 로컬 상태 디렉터리는 `0700`, SQLite는 `0600`으로 강제한다. SQLite 자체는 애플리케이션 수준에서 암호화하지 않으므로 FileVault가 켜진 소유자 장치에만 둔다. 이 DB의 복사·백업에도 원문이 포함된다.
- 메시지 원문, 이름, 연락처, 원본 ID, 인증 재료를 로그에 남기지 않는다. SQLite 외에 평문 덤프 파일을 만들지 않는다.
- `status.last_presented_at_utc`는 msgpipe가 CCT를 마지막으로 준비해 표준 출력에 제시한 시각이지, KakaoTalk·Messages의 읽음 상태가 아니다.
- 모델에는 실명·원본 ID 대신 안정적인 스레드·화자 별칭만 전달한다. 최종 결과에 필요할 때만 `identities`로 이름을 해석한다.
- 첨부 파일은 메타데이터 표식만 보관한다. 별도 요청 없이 파일을 열거나 변환하지 않는다.

## 데이터 흐름

```text
read-only source CLI
  -> source adapter
  -> protected raw archive
  -> pending-only query
  -> shared lexical replacer
  -> shared conversation optimizer
  -> per-thread CCT
  -> immutable session summary commit
  -> cumulative thread rollup
  -> cumulative global rollup
```

- `ㅋ`, `ㅋㅋㅋㅋ`, 단독 이모지·기호, 짧은 긍정·부정·질문, URL, 반복 글자는 공통 lexical replacer가 처리한다.
- 근접 중복 같은 교차 메시지 판단은 공통 conversation optimizer가 처리한다.
- 시간·화자 상속과 30분 세션은 exporter가 처리한다. 앱별 옵티마이저를 만들지 않는다.
- 수정되거나 뒤늦게 동기화된 메시지는 분석 연결을 해제해 자동으로 다시 pending 상태로 만든다.

## 동기화와 비용 측정

- 범위는 `start <= timestamp < end`다. 날짜만 주어지면 `Asia/Seoul` 자정으로 해석한다.
- "지난달"은 지난 달력월 전체, "이번 달"은 1일부터 오늘을 포함하는 월 누계다. 오늘까지는 다음 날을 exclusive `--end`로 쓴다.
- 별도 모델 단가가 없으면 "토큰 비용"은 `o200k_base` 토큰 수를 뜻한다.

1. 소스 리더를 확인한다.

   ```bash
   msgpipe doctor kakao
   msgpipe doctor imessage
   ```

2. 요청 범위를 로컬 아카이브에 먼저 동기화한다. 이 단계는 원문을 stdout에 출력하지 않는다.

   ```bash
   msgpipe sync kakao --start 2026-06-01 --end 2026-07-23
   msgpipe sync imessage --start 2026-06-01 --end 2026-07-23
   ```

3. 원문 없이 스레드별 보관·pending·마지막 처리 시점을 확인한다.

   ```bash
   msgpipe status kakao
   msgpipe status imessage
   ```

4. 아카이브에서 비용을 측정한다. `benchmark`는 소스 DB를 다시 읽지 않고 본문도 출력하지 않는다.

   ```bash
   msgpipe benchmark kakao --start 2026-06-01 --end 2026-07-23
   msgpipe benchmark imessage --start 2026-06-01 --end 2026-07-23
   ```

## 증분 분석

사용자가 분석을 명시한 뒤에만 다음을 수행한다.

1. `msgpipe context get global`과 `msgpipe context get thread --thread <alias>`로 최신 누적 요약을 읽는다. 없는 context는 빈 상태로 취급한다. 과거 session 원문이나 session 요약 전체를 다시 붙이지 않는다.
2. `status`에서 `pending_messages > 0`인 별칭만 고른다.
3. 스레드 하나의 미분석 원문만 CCT로 준비한다.

   ```bash
   msgpipe pending kakao --start 2026-06-01 --end 2026-07-23 --thread K001
   msgpipe pending imessage --start 2026-06-01 --end 2026-07-23 --thread I001
   ```

4. 모델 입력에는 CCT 범례, 최신 전역 rollup, 해당 스레드의 최신 rollup, 이번 pending CCT만 넣는다. 기본 모델은 `gpt-5.6-terra`, reasoning effort는 `medium`이다. 스레드마다 새 분석 컨텍스트를 사용한다.
5. 세션별 결과는 다음처럼 짧게 만든다.

   ```text
   summary: <세션 핵심>
   important: <결정·약속·변화>
   open: <미결 항목>
   schedule: <일정 후보 또는 없음>
   ```

6. 분석이 성공한 뒤 각 CCT `S` 세션을 별도로 저장한다. `--start`는 해당 `S`, `--end`는 다음 `S`의 시각이며 마지막 세션은 요청 범위의 끝이다. 이 커밋은 그 구간에서 실제로 CCT로 제시된 pending 메시지만 분석 완료로 연결한다.

   ```bash
   printf '%s' '<session summary>' | msgpipe context put session \
     --thread K001 --start 2026-07-10T09:00:00+09:00 --end 2026-07-10T11:30:00+09:00
   ```

   schedule 최적화 결과에 메시지 행이 하나도 없으면 모델을 호출하지 않는다. 대신 실제 `pending` 요청 범위를 `정보성 내용 없음` 같은 짧은 세션 요약으로 커밋하고 `--model msgpipe-optimizer --reasoning-effort none`을 기록해 무의미 반응이 계속 pending으로 남지 않게 한다.

7. 한 스레드의 세션 커밋이 끝나면 미반영 세션 요약을 CTX로 읽는다. 출력이 비어 있으면 thread rollup 작업은 없다. 기존 thread rollup과 CTX의 세션 요약들만 합쳐 새 누적 thread rollup을 만들고, 헤더의 `through`를 저장 명령에 그대로 쓴다. 과거 원문은 사용하지 않는다.

   ```bash
   msgpipe context inputs thread --thread K001
   printf '%s' '<cumulative thread summary>' | msgpipe context put thread \
     --thread K001 --through-context-id 42 \
     --start 2026-06-01 --end 2026-07-23
   ```

8. 모든 스레드가 끝난 뒤 `context inputs global`을 읽는다. 출력이 비어 있으면 global rollup 작업은 없다. 기존 global rollup과 CTX에 나온 별칭별 최신 thread rollup만 합쳐 관계 변화·결정·미결 항목의 새 중앙 rollup을 저장한다. 중간 thread 버전, 원문이나 전체 session 이력을 다시 넣지 않는다.

   ```bash
   msgpipe context inputs global
   printf '%s' '<global summary>' | msgpipe context put global \
     --through-context-id 51 --start 2026-06-01 --end 2026-07-23
   ```

`context put session`이 성공하기 전에는 메시지가 pending에서 빠지지 않는다. 분석이 실패하면 같은 `pending`을 다시 만들 수 있다. `thread`·`global`은 교체가 아니라 append-only 버전이며 `context get`은 최신 버전을 반환한다. rollup 전에 중단된 session/thread 요약은 연결되지 않은 채 다음 `context inputs`에 다시 나타난다. `through` 출력 뒤 생긴 더 큰 ID도 다음 실행에 남는다. 이미 session 요약에 연결된 과거 원문은 다음 모델 입력에 재첨부하지 않고, 수정·추가되어 pending으로 돌아온 메시지만 다시 분석한다.

일정 후보를 사용자에게 보여줄 때만 다음처럼 이름을 복원한다.

```bash
msgpipe identities K001
```
