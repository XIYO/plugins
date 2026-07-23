# XIYO Plugins

[English](README.md)

개인 컨텍스트를 수집하고, 확인한 약속과 할 일만 계획으로 연결하는 로컬 우선 macOS 비서를 하나만 설치합니다.

> **현재 단계:** Planner는 Preview, Context는 로컬 메시지 리더와 메일 연결 상태에 따라 달라지는 Experimental 기능입니다.

## Sherpa 설치

### Codex

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add sherpa@xiyo
```

### Claude Code

```bash
claude plugin marketplace add XIYO/plugins
claude plugin install sherpa@xiyo
```

설치 후 새 작업이나 세션을 시작합니다.

## 하나의 제품, 두 개의 도메인

| 도메인 | 입력 또는 목적지 | 역할 |
| --- | --- | --- |
| Context | KakaoTalk, iMessage, 연결된 메일 | 수집·검색·검토·요약과 PlanningCandidate 추출 |
| Planner | Apple Calendar, Apple Reminders | 확인한 약속과 할 일을 Event 또는 Task로 기록 |

Sherpa는 `ContextItem -> PlanningCandidate -> Event | Task` 흐름을 조정합니다. 애플리케이션과 외부 도구는 제품 도메인이 아니라 어댑터입니다.

공개 실행 인터페이스는 두 가지뿐입니다.

```text
sherpa context ...
sherpa planner ...
```

## 첫 사용

```text
셰르파, Context와 Planner 준비 상태만 확인해줘. 내 데이터는 변경하거나 출력하지 마.
```

플러그인 루트에서 수동 설치·진단:

```bash
bash scripts/install-runtime.sh context
bash scripts/install-runtime.sh planner
bash scripts/doctor.sh all
```

### 선행 조건

- macOS 14 이상이 필요하며 전체 조합은 macOS 26.x에서 검증합니다.
- 관리 런타임 설치에는 Rust와 Xcode Command Line Tools가 필요합니다.
- Calendar·Reminders·전체 디스크 접근·접근성 권한은 필요한 어댑터만 요청합니다.
- 선택형 `kakaocli`와 `imsg`는 자동 설치하지 않습니다.
- 메일 수집에는 호스트에 연결된 메일 앱이 필요합니다.

## 데이터 경계

- 대화 원본 데이터베이스는 읽기 전용입니다.
- 선택한 Context는 명시적으로 비울 때까지 소유자 전용 로컬 SQLite에 보관합니다.
- Context에서 발견한 내용은 사용자 확인 없이 Planner에 기록하지 않습니다.
- Planner 변경은 Apple Calendar 또는 Reminders에서 다시 읽어 검증합니다.
- 카카오톡 텍스트 전송은 정확한 대상·본문 미리보기·짧은 승인 토큰·명시적 확인이 모두 필요합니다.
- iMessage·이메일 전송, 첨부, 반응, 일괄 전송은 지원하지 않습니다.
- 에이전트에 선택해 반환한 내용만 설정된 모델 컨텍스트에 들어갑니다.

개인 데이터 접근을 허용하기 전에 [Sherpa 전체 안내](plugins/sherpa/README.md)를 확인합니다.

## 업데이트와 제거

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

제거 전에 Context 상태 위치를 확인하고 필요하면 비웁니다.

```bash
~/.local/bin/sherpa context state-path
~/.local/bin/sherpa context purge --force
codex plugin remove sherpa@xiyo
```

## 저장소 구조

- `.agents/plugins/marketplace.json`: Codex 마켓플레이스 카탈로그
- `.claude-plugin/marketplace.json`: Claude Code 마켓플레이스 카탈로그
- `plugins/sherpa/`: 유일한 자기완결형 제품이자 정본
- `catalog-policy.json`: 공개 제품 정책
- `scripts/`: 카탈로그·버전·저장소 검사

## 기여와 지원

Pull Request 전에 [기여 안내](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md)를 읽습니다. 취약점은 [보안 정책](https://github.com/XIYO/.github/blob/main/SECURITY.md)에 따라 비공개로 제보합니다.

## 라이선스

[MIT](LICENSE) © 2026 XIYO
