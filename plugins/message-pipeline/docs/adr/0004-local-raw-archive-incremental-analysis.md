---
id: ADR-0004
title: Local raw archive and incremental analysis state
status: accepted
date: 2026-07-22
---
# ADR-0004: Local raw archive and incremental analysis state

## 상황

원문을 매 실행의 메모리에서만 처리하면 같은 기간을 외부 소스에서 반복 추출하고 과거 대화를 모델에 다시 전달해야 한다. 해시와 파생 요약만으로는 메시지 수정·지연 동기화를 재분석하거나, 어느 메시지까지 CCT로 제시하고 요약했는지 증명할 수 없다.

## 결정

최적화 전 `NormalizedMessage`를 msgpipe의 소유자 전용 SQLite에 저장한다. 동일 메시지는 `(source, source_message_id)`로 upsert하고, 내용이나 구조 메타데이터가 바뀌면 기존 분석 연결을 제거한다.

각 아카이브 행은 마지막 수집·CCT 제시·분석 시점과 `analysis_context_id`를 가진다. `pending`은 요약에 연결되지 않은 행만 준비한다. `context put session`은 실제로 제시된 pending 행 집합의 해시와 세션 요약을 append-only로 저장하고, 같은 트랜잭션에서 해당 행을 분석 완료로 연결한다.

요약 scope는 세 계층이다. `session`은 원문 coverage를 증명하는 불변 요약, `thread`는 이전 스레드 rollup과 새 세션 요약을 합친 누적 요약, `global`은 스레드 간 결정·관계 변화·미결 항목의 누적 요약이다. 각 session/thread 행은 상위 rollup에 포함됐는지 기록한다. `context inputs`가 아직 미반영된 요약과 watermark를 내보내고, 상위 rollup 저장과 watermark 이하 입력 연결을 한 트랜잭션으로 처리한다. 다음 분석에는 최신 `thread`·`global` rollup과 새 pending만 전달한다.

## 결과

- 과거 원문 대신 최신 스레드/전역 누적 요약과 새 메시지만 모델에 전달할 수 있다.
- 분석 실패 시 연결이 생성되지 않아 같은 pending 입력을 재시도할 수 있다.
- session 저장 뒤 rollup 전에 중단돼도 미반영 요약을 다시 조회할 수 있다.
- 수정·백필 메시지는 자동으로 pending으로 돌아온다.
- 상태 DB와 그 백업은 원문·이름·원본 ID를 포함하는 고감도 자산이 된다.

SQLite 애플리케이션 암호화는 이번 결정에 포함하지 않는다. 상태 디렉터리 `0700`, 파일 `0600`, FileVault가 활성화된 소유자 장치를 기본 보안 경계로 사용하며 원문을 로그·Git·보조 덤프에 복제하지 않는다.

## 대안

- 원문 비저장: 개인정보 표면은 작지만 증분 분석과 수정 감지가 불가능해 기각한다.
- 최적화 결과만 저장: 치환 규칙 변경 시 원문에서 재생성할 수 없고 손실을 복구할 수 없어 기각한다.
- 매 실행 원본 DB 재조회: 소스 부하와 처리 비용이 반복되고 분석 커서를 신뢰성 있게 유지할 수 없어 기각한다.
- SQLCipher로 상태 DB 암호화: 보호 수준은 높지만 배포 의존성과 키 관리가 추가된다. 별도 보안 요구가 생기면 후속 ADR로 검토한다.
