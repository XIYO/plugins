---
name: x-message-pipeline
description: macOS의 KakaoTalk·iMessage 로컬 기록을 msgpipe로 읽기 전용 추출하고, 공통 정규화·채팅 표현 치환·대화 구조 최적화 후 CCT로 스레드별 내보내며 o200k_base 토큰을 벤치마크하고 중앙 별칭·분석 맥락을 관리한다. "카톡/문자 토큰 줄여줘", "대화 익스포트", "스레드별 분석", "CCT", "메시지 옵티마이저", KakaoTalk와 iMessage 통합 분석 준비 요청에 사용한다.
---

# 메시지 파이프라인

KakaoTalk와 iMessage의 서로 다른 출력은 소스 어댑터에서 끝낸다. 이후 단계는 같은 정규화 모델과 옵티마이저를 사용하고, 모델에는 이름·원본 ID 대신 안정적인 별칭과 CCT를 전달한다.

## 실행 위치

- 이 `SKILL.md`에서 두 디렉터리 위를 플러그인 루트로 해석한다. marketplace 설치본은 캐시로 복사되므로 원래 clone 경로나 저장소 밖 파일을 가정하지 않는다.
- 설치된 `msgpipe`가 있으면 우선 사용한다.
- `msgpipe`가 없으면 플러그인 루트의 설치 스크립트를 실행한다.

  ```bash
  bash <plugin-root>/scripts/install-runtime.sh
  ```

- 설치하지 않고 개발·진단할 때만 `cargo run --manifest-path <plugin-root>/Cargo.toml -- <command>`를 사용한다.
- 앱을 수정하거나 포맷을 진단할 때만 플러그인 루트의 `ARCHITECTURE.md`와 `contracts/cct/CCT.md`를 읽는다. 구현을 스킬 안에 복제하지 않는다.

## 불변 경계

- 원본 DB와 소스 CLI는 읽기 전용으로만 다룬다. 인증·권한·CLI 설치 문제는 각각 `x-kakaotalk`, `x-imessage`를 함께 사용한다.
- 사용자가 분석을 명시하기 전에는 추출 크기와 토큰 비용만 측정한다. 벤치마크는 본문을 출력하지 않는다.
- 메시지 평문 덤프를 파일이나 로그에 남기지 않는다. 로그에는 작업명·건수·소요 시간·오류 종류만 기록한다.
- 날짜 범위는 `start <= timestamp < end`다. 날짜만 주어지면 기본 `Asia/Seoul` 자정으로 해석한다.
- 첨부 파일은 메타데이터 표식만 유지한다. 사용자의 별도 요청 없이 파일을 열거나 변환하지 않는다.
- 모델 입력에 identity map을 넣지 않는다. 일정 후보가 나온 뒤 필요한 결과에 한해서만 로컬에서 이름을 해석한다.

## 구조

```text
source CLI
  -> source adapter
  -> NormalizedMessage
  -> shared lexical replacer
  -> shared conversation optimizer
  -> stable alias/state
  -> format exporter
```

- 앱별 JSON 필드, 메시지 타입, 첨부·답장 신호 해석은 소스 어댑터 책임이다.
- `ㅋ`, `ㅋㅋㅋㅋ`, 단독 이모지·기호, 짧은 긍정·부정·질문, URL, 반복 글자 같은 표현 치환은 공통 lexical replacer 책임이다.
- 시간상 인접한 중복 메시지와 같은 교차 메시지 판단은 공통 conversation optimizer 책임이다.
- 시간·화자 상속, 세션 구분, escape는 exporter 책임이다. 앱별 옵티마이저를 따로 만들지 않는다.
- KakaoTalk의 일반 텍스트 `type=1`에도 mention/linkify/bot 메타데이터가 있을 수 있으므로 `attachment` 필드가 비어 있지 않다는 이유만으로 첨부로 바꾸지 않는다.
- iMessage의 `reply_to_guid`·`reply_to_text`는 SMS 연속 관계에도 나타날 수 있다. `thread_originator_guid` 같은 강한 신호 없이 답장 관계를 추론하지 않는다.

## 준비와 비용 측정

- 별도 통화나 모델 단가가 없으면 "토큰 비용"은 `o200k_base` 토큰 수를 뜻한다. 금액을 추정하려면 사용자가 지정한 모델과 최신 입력 단가를 별도로 확인한다.
- 상대 기간에서 "지난달"은 지난 달력월 전체, "이번 달"은 이번 달 1일부터 현재 시각까지로 해석한다. 날짜 인자에는 현재 날짜의 다음 날을 exclusive `--end`로 사용한다.

1. 본문을 읽지 않는 진단을 실행한다.

   ```bash
   msgpipe doctor kakao
   msgpipe doctor imessage
   ```

2. 같은 반개구간으로 소스별 비용을 측정한다. 기본 `schedule` + `cct`를 사용한다.

   ```bash
   msgpipe benchmark kakao --start 2026-06-01 --end 2026-07-23
   msgpipe benchmark imessage --start 2026-06-01 --end 2026-07-23
   ```

3. 결과의 `thread_manifest`에서 스레드별 메시지 수·기간·토큰을 확인한다. 본문이 필요한 경우에도 전체가 아니라 안정 별칭 하나만 표준 출력으로 내보낸다.

   ```bash
   msgpipe export kakao --start 2026-06-01 --end 2026-07-23 --thread K001
   msgpipe export imessage --start 2026-06-01 --end 2026-07-23 --thread I001
   ```

- 모델 입력은 CCT를 기본으로 한다. JSON은 제어면·디버깅에만 사용한다.
- `schedule`은 날짜와 30분 세션 단위로 시간·화자를 상속한다. 원문에 가까운 검토가 필요할 때만 `--profile exact`를 사용한다.
- 스레드가 목표 컨텍스트보다 크면 날짜 또는 세션 경계에서 나누고, 직전 파트의 파생 요약만 다음 파트에 전달한다. 같은 state를 사용해 별칭을 유지한다.

## 분석 실행

사용자가 분석을 명시한 뒤에만 수행한다.

1. `msgpipe cct-spec`의 범례와 `msgpipe context get global`의 최신 중앙 맥락을 읽는다. 중앙 맥락이 없으면 빈 상태로 시작한다.
2. 각 분석 작업에는 범례, 중앙 맥락, 단일 스레드 CCT만 넣는다. 기본 분석 모델은 `gpt-5.6-terra`, reasoning effort는 `medium`이다.
3. 스레드별로 새 분석 컨텍스트를 사용한다. 병렬화하더라도 원문을 스레드 사이에 공유하지 않는다.
4. 결과는 JSON보다 다음처럼 짧은 키-값 형식으로 받는다.

   ```text
   candidate: yes|no
   when: <일시 또는 미정>
   action: <일정·할 일>
   evidence: <짧은 근거>
   confidence: high|medium|low
   ```

5. 원문이 아닌 파생 요약만 append-only context에 저장한다.

   ```bash
   printf '%s' '<thread summary>' | msgpipe context put thread \
     --thread K001 --start 2026-06-01 --end 2026-07-23
   printf '%s' '<global summary>' | msgpipe context put global \
     --start 2026-06-01 --end 2026-07-23
   ```

6. 일정 후보가 확인된 결과만 로컬 identity map으로 해석한다.

   ```bash
   msgpipe identities K001
   ```

이름 해석은 최종 사용자 출력용이며 분석 모델 입력용이 아니다. context에는 원문 대화를 복사하지 말고 결정·미결 항목·관계 맥락만 간결하게 남긴다.
