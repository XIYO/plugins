# XIYO Plugins

[English](README.md)

Codex와 Claude Code가 macOS의 개인 작업을 검토 가능한 명령줄 도구로 다루게 하는 로컬 우선 플러그인 모음입니다.

이 저장소는 공개 마켓플레이스입니다. 각 플러그인은 에이전트 스킬, 런타임 소스, 데이터 계약, 설치기, 검증 절차를 함께 담습니다. 설치본은 개인 스킬 저장소나 작성자 컴퓨터의 절대경로에 의존하지 않습니다.

> **현재 단계:** Preview. macOS에서 사용할 수 있지만 아직 로컬 런타임을 소스에서 빌드하며, 안정 버전 전까지 인터페이스가 달라질 수 있습니다.

## 플러그인 선택

| 하고 싶은 일 | 설치 ID | 런타임 | 데이터 접근 | 단계 |
| --- | --- | --- | --- | --- |
| 화면 자동화 없이 Apple Calendar 관리 | `apple-calendar@xiyo` | `calctl`, `calmeta` | EventKit을 통한 캘린더 읽기·쓰기 | Preview |
| 새 KakaoTalk·iMessage 내용만 증분 분석 | `message-pipeline@xiyo` | `msgpipe` | 원본 DB 읽기 전용, 보호된 로컬 보관소 쓰기 | Experimental |

`message-pipeline`은 KakaoTalk와 iMessage 소스 리더를 별도로 준비해야 하므로 Experimental로 표시합니다. 공통 원문 보관소, 증분 상태, CCT 출력은 번들된 `msgpipe` 애플리케이션이 담당합니다.

## GitHub에서 설치

### Codex

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add apple-calendar@xiyo
codex plugin add message-pipeline@xiyo
codex plugin list
```

### Claude Code

```bash
claude plugin marketplace add XIYO/plugins
claude plugin install apple-calendar@xiyo
claude plugin install message-pipeline@xiyo
```

새 스킬을 불러오려면 설치 후 새 Codex 작업 또는 Claude Code 세션을 시작합니다.

## 선행 조건

두 플러그인은 현재 macOS만 지원합니다.

- **Apple Calendar:** Rust 도구 모음과 Xcode Command Line Tools(`swiftc`, `codesign`)가 필요합니다. 첫 사용 시 macOS 표준 권한 창에서 캘린더 접근을 허용합니다.
- **Message Pipeline:** Rust 도구 모음과 읽기 전용 소스 리더가 필요합니다. KakaoTalk는 `kakaocli`, iMessage는 `imsg`를 사용하며 소스 리더에 전체 디스크 접근 권한이 필요할 수 있습니다.
- 설치된 런타임을 찾을 수 있도록 `~/.local/bin`과 `~/.cargo/bin`을 `PATH`에 포함합니다.

런타임이 없으면 번들 스킬이 플러그인 내부 설치기를 실행할 수 있습니다. 설치기는 임시 디렉터리에서 빌드하므로 마켓플레이스 캐시에 빌드 결과를 남기지 않습니다.

## 첫 사용

새 작업이나 세션에서 다음처럼 요청할 수 있습니다.

```text
내 Apple Calendar 접근 상태와 사용 가능한 iCloud 캘린더를 확인해줘. 아무것도 변경하지 마.
```

```text
iMessage 소스 리더가 준비됐는지만 확인해줘. 메시지 내용은 출력하거나 분석하지 마.
```

런타임을 직접 확인하려면 다음 명령을 사용합니다.

```bash
calctl doctor
calmeta spec
msgpipe doctor imessage
msgpipe doctor kakao
```

설치 성공과 데이터 권한 허용은 별개입니다. `doctor` 명령은 캘린더나 메시지 내용을 출력하지 않고 실행 파일과 macOS 권한 상태를 알려줍니다.

## 데이터 경계

- 런타임은 원본 데이터를 스스로 업로드하지 않습니다.
- 에이전트에 반환한 내용은 호스트 애플리케이션에 설정된 모델 제공자가 처리할 수 있습니다. 먼저 읽기 전용 진단을 요청하고, 분석이나 변경 전에는 선택 범위를 확인합니다.
- Apple Calendar 쓰기는 EventKit 권한이 필요합니다. 스킬은 수정 전에 대상을 조회하고 반복 일정 변경 범위를 명시합니다.
- Message Pipeline은 KakaoTalk 또는 Messages 원본 DB를 수정하지 않습니다. 동기화한 원문은 소유자 전용 로컬 SQLite에 저장합니다. 애플리케이션 수준 암호화는 하지 않으므로 FileVault 사용을 권장합니다.
- 메시지 분석은 미분석 내용만 스레드·화자 별칭과 함께 내보내지만, 선택된 메시지 본문은 설정된 모델 컨텍스트에 들어갑니다.
- 로그와 버그 제보에는 메시지 본문, 일정 메모, 이름, 연락처, 원본 식별자, 인증 정보, 로컬 DB 경로를 포함하지 않습니다.

전체 권한·저장 모델은 각 플러그인 README를 확인합니다.

- [Apple Calendar](plugins/apple-calendar/README.md)
- [Message Pipeline](plugins/message-pipeline/README.md)

## 업데이트와 제거

Codex:

```bash
codex plugin marketplace upgrade xiyo
codex plugin add apple-calendar@xiyo
codex plugin remove apple-calendar@xiyo
```

Claude Code:

```bash
claude plugin marketplace update xiyo
claude plugin update apple-calendar@xiyo
claude plugin uninstall apple-calendar@xiyo
```

`apple-calendar`를 `message-pipeline`으로 바꾸면 같은 흐름을 사용할 수 있습니다. 플러그인을 제거해도 런타임 실행 파일이나 로컬 애플리케이션 데이터가 자동 삭제되지는 않으므로, 삭제 전 해당 플러그인 README를 확인합니다.

## 저장소 구조

- `.agents/plugins/marketplace.json`: Codex 마켓플레이스 카탈로그
- `.claude-plugin/marketplace.json`: Claude Code 마켓플레이스 카탈로그
- `plugins/<name>/`: 자기완결형 플러그인, 스킬, 런타임, 설치기, 계약, 테스트
- `scripts/`: 저장소 전체 일관성 검사

마켓플레이스 매니페스트가 기계 판독 카탈로그의 정본입니다. 위 표는 소비자용 설명이며 CI에서 매니페스트와 일치하는지 검사합니다.

## 기여와 지원

Pull Request를 열기 전에 [기여 안내](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md)를 읽습니다. 버그와 기능 요청은 해당 저장소의 이슈 양식을 사용합니다. 취약점은 [보안 정책](https://github.com/XIYO/.github/blob/main/SECURITY.md)에 따라 비공개로 제보합니다.

## 라이선스

[MIT](LICENSE) © 2026 XIYO
