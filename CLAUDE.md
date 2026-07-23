# Repository guide

이 저장소는 Codex와 Claude Code가 설치할 수 있는 XIYO 공개 플러그인 marketplace다. 소비자용 정식 제품은 `sherpa@xiyo` 하나이며 Calendar·Reminders·Messages 전문 스킬과 런타임을 내부에 함께 둔다. 플러그인 설치기는 디렉터리를 캐시로 복사하므로 저장소 밖 파일이나 절대 경로에 의존하지 않는다.

`CLAUDE.md`가 정본이며 `AGENTS.md`는 이 파일을 가리키는 심볼릭 링크다. 두 파일을 따로 복제해 관리하지 않는다.

## 정본과 구조

- `.agents/plugins/marketplace.json`: Codex 카탈로그 정본
- `.claude-plugin/marketplace.json`: Claude Code 카탈로그 정본
- `catalog-policy.json`: 정식 플러그인과 호환 플러그인 구분
- `plugins/<name>/.codex-plugin/plugin.json`: Codex 플러그인 매니페스트
- `plugins/<name>/.claude-plugin/plugin.json`: Claude Code 플러그인 매니페스트
- `plugins/<name>/skills/<name>/SKILL.md`: 에이전트 실행 규칙
- `plugins/<name>/scripts/install-runtime.sh`: 캐시 내부에서도 동작하는 런타임 설치기
- `scripts/check-all.sh`: 공개 전 저장소 전체 품질 게이트

Sherpa는 `skills/sherpa`, `skills/apple-calendar`, `skills/apple-reminders`, `skills/message-pipeline`을 모두 포함한다. 플러그인끼리 의존하지 않는다. 설치본은 항상 자기완결형이어야 하며 개인 `x-*` 스킬, 저장소 밖 상대경로, 작성자 컴퓨터의 절대경로에 의존하지 않는다.

`apple-calendar`와 `message-pipeline` 독립 패키지는 마이그레이션 기간의 호환 경로다. 정본 런타임 소스와 달라지지 않도록 `scripts/check-legacy-sync.py`로 검사하고, 신규 기능의 소비자 진입점은 Sherpa만 사용한다.

## 소비자 문서

- 루트 `README.md`는 영어 진입점, `README.ko.md`는 한국어 진입점이다. 설치 ID, 단계, 플랫폼, 권한, 선행 조건, 첫 사용, 검증, 데이터 경계를 같은 변경에서 갱신한다.
- 공개 설치 명령은 GitHub 원격 `XIYO/plugins`를 사용한다. `$PWD` 예시는 로컬 개발 절에만 둔다.
- 각 플러그인 README는 결과 중심 설명 → 설치 → 선행 조건 → 첫 사용 → 검증 → 데이터 경계 → 제한 사항 → 개발 순서로 쓴다.
- 구현 용어는 사용자가 얻는 결과를 먼저 설명한 뒤 소개한다.
- 스킬 폴더에는 에이전트 실행에 필요한 지식만 둔다. 사용자용 설치 문서나 변경 이력을 `SKILL.md`에 넣지 않는다.

## 데이터 불변 규칙

- KakaoTalk와 iMessage 소스 데이터베이스는 읽기 전용으로 다룬다. 읽음 처리, 감시, 첨부 변환 명령을 코드 경로에 추가하지 않는다.
- KakaoTalk 텍스트 전송은 Sherpa의 별도 승인 경계에서만 허용한다. 정확한 채팅방 단일 해석, 본문 결합 미리보기, 짧은 만료 토큰, 사용자 확인, 일회성 전송을 모두 거쳐야 한다. iMessage 전송은 지원하지 않는다.
- 원문 채팅은 msgpipe의 소유자 전용 로컬 SQLite에만 저장한다. 동일 원본 메시지는 멱등 upsert하고, 수정된 메시지는 기존 분석 연결을 끊어 다시 pending으로 만든다.
- 분석 완료와 세션 요약, 상위 rollup과 입력 watermark 연결은 각각 같은 트랜잭션에서 처리한다.
- 모델 입력은 스레드와 화자를 별칭으로 치환한다. 실명 매핑은 소유자 전용 로컬 SQLite에만 둔다.
- 로그, 테스트 fixture, 이슈, Git 저장소에는 메시지 본문, 일정 메모, 이름, 전화번호, 이메일, 원본 식별자, 키, 상태 DB, 로컬 절대경로를 남기지 않는다.

## 버전과 릴리스

- 단일 Rust 앱 플러그인은 plugin manifest와 Cargo package의 기본 SemVer를 함께 갱신한다.
- Sherpa 플러그인 버전과 내부 런타임 버전은 독립적이다. `runtime-versions.json`이 `calmeta`, `calctl`, `msgpipe`, RemCTL 고정 버전의 정본이다.
- Codex 로컬 재설치용 `+codex.<cachebuster>`는 기본 버전을 바꾸지 않는다.
- 사용자에게 보이는 변경은 플러그인별 `CHANGELOG.md`의 `Unreleased`에 기록한다.
- 공개 태그는 `<plugin-name>-v<version>` 형식을 사용한다.
- 일정 메모 같은 데이터 계약은 앱 버전과 분리한다. 호환성 규칙이 PATCH를 해석하지 않으면 `MAJOR.MINOR`만 저장한다.

## 로깅과 오류

- 접두사는 `[layer:module:action]` 형식을 쓴다.
- 외부 명령, 데이터베이스, Calendar 권한 같은 경계에서 시작·성공·실패를 기록한다.
- 기본 레벨은 `warn`; `LOG_LEVEL`로 조절한다.
- 실패를 잡았다면 원본 오류를 보존해 상위로 전달하거나 의미 있는 복구를 수행한다. 빈 catch나 오류 삼키기를 금지한다.
- 로그에 개인 원문이나 인증 정보를 넣지 않는다.

## 품질 게이트

저장소 루트에서 다음을 실행한다.

```bash
bash scripts/check-all.sh
```

릴리스 전에는 추가로 각 스킬과 플러그인을 공식 validator로 검사한다.

```bash
python3 ~/.codex/skills/.system/skill-creator/scripts/quick_validate.py plugins/<name>/skills/<name>
python3 ~/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py plugins/<name>
claude plugin validate .
```

검증 스크립트는 임시 빌드 디렉터리를 사용하고 종료 시 제거한다. 플러그인 안에 `target/`을 만든 채 설치하거나 배포하지 않는다. 실데이터 회귀 검증은 집계 수치와 해시만 출력하며 원문은 터미널에도 표시하지 않는다.
