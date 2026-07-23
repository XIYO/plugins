# XIYO Plugins

[English](README.md)

캘린더·미리 알림·KakaoTalk·iMessage 작업을 총괄하는 로컬 우선 macOS 비서를 하나만 설치합니다.

> **현재 단계:** Preview. Calendar·Reminders 연동은 Preview이고, KakaoTalk·iMessage 기능은 선택형 외부 리더와 로컬 원문 보관소에 의존하므로 Experimental입니다. 안정 버전 전까지 인터페이스와 설정 방식이 달라질 수 있습니다.

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

설치 후 새 Codex 작업 또는 Claude Code 세션을 시작하면 번들된 모든 스킬을 불러옵니다.

## 설치는 하나, 내부 기능은 전문 모듈

| Sherpa가 총괄하는 기능 | 내부 스킬 | 런타임 | 접근 경계 |
| --- | --- | --- | --- |
| Apple Calendar | `apple-calendar` | `calctl`, `calmeta` | 권한 승인 후 EventKit 읽기·쓰기 |
| Apple Reminders | `apple-reminders` | RemCTL 1.5.1 | iCloud 읽기, EventKit/ReminderKit 쓰기 |
| KakaoTalk·iMessage | `message-pipeline` | `msgpipe` | 원본 저장소 읽기 전용, 보호된 로컬 보관소 쓰기 |
| 통합 브리핑·등록 | `sherpa` | 위 전문 모듈 | 범위가 좁은 라우팅과 결과 통합 |

플러그인은 설치·제품 단위입니다. 내부 스킬은 분리해 각 데이터 소스의 권한, 검증, 삭제 안전 규칙을 그대로 유지합니다.

## 첫 사용

먼저 읽기 전용 진단을 요청합니다.

```text
셰르파, 캘린더·미리 알림·메시지 분석 준비 상태만 확인해줘. 내 데이터는 변경하거나 출력하지 마.
```

Sherpa는 특정 기능을 처음 사용할 때 필요한 런타임만 설치합니다. 번들 설치기의 기본 위치는 `~/.local/bin`이며 `SHERPA_INSTALL_ROOT`로 바꿀 수 있습니다.

플러그인 루트에서 직접 설치·진단하려면 다음을 사용합니다.

```bash
bash scripts/install-runtime.sh calendar
bash scripts/install-runtime.sh reminders
bash scripts/install-runtime.sh messages
bash scripts/doctor.sh all
```

### 선행 조건

- macOS 14 이상이 필요합니다. Calendar는 Rust와 Xcode Command Line Tools, Reminders는 Python 3와 Xcode의 Swift·Clang 도구, Message Pipeline은 Rust가 필요합니다. Sherpa 전체 조합은 macOS 26.x에서 검증합니다.
- Calendar와 Reminders 권한은 각 기능을 처음 사용할 때 해당 어댑터가 요청합니다.
- RemCTL은 번들 설치기가 고정·검증한 1.5.1 소스 커밋에서 가져옵니다. upstream 설치는 임시 위치에서 수행하고 필요한 구성요소만 복사하며, 일반명 `rctl`·`reminders` 별칭은 만들지 않습니다.
- `kakaocli`와 `imsg`는 선택형 외부 리더이며 자동 설치하지 않습니다.
- CLI를 직접 쓰려면 `~/.local/bin`을 `PATH`에 포함합니다.

## 데이터 경계

- Calendar 변경은 EventKit으로 실행하고 변경 후 다시 읽어 검증합니다.
- Reminders 변경은 RemCTL로 실행하며 Sherpa가 Reminders 데이터베이스에 직접 쓰지 않습니다.
- Message Pipeline은 KakaoTalk·Messages 원본 데이터베이스를 수정하지 않고 메시지를 발송하지 않습니다.
- 동기화한 메시지 원문은 사용자가 명시적으로 비울 때까지 소유자 전용 로컬 SQLite에 저장합니다. 애플리케이션 수준 암호화는 하지 않으므로 FileVault 사용을 권장합니다.
- 사용자가 선택해 에이전트에 반환한 내용만 설정된 모델 컨텍스트에 들어갑니다.
- 로그와 버그 제보에는 메시지 본문, 일정·미리 알림 메모, 이름, 연락처, 인증 정보, 원본 식별자, 로컬 데이터베이스 경로를 넣지 않습니다.

개인 데이터 접근을 허용하기 전에 [Sherpa 전체 안내](plugins/sherpa/README.md)를 확인합니다.

## 업데이트와 제거

Git 마켓플레이스를 갱신하고 현재 스냅샷을 다시 설치합니다.

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

Sherpa를 제거하기 전에 민감한 메시지 보관소 위치를 확인하고, 필요하면 명시적으로 비웁니다.

```bash
~/.local/bin/msgpipe state-path
~/.local/bin/msgpipe purge --force
codex plugin remove sherpa@xiyo
```

`purge --force`는 출력된 보관소 경로를 확인한 뒤에만 실행합니다. msgpipe 원문 보관소와 분석 이력을 모두 삭제합니다. 플러그인을 제거해도 관리 런타임, 메시지 보관소, 이후 `remctl onboard`가 만든 설정은 자동으로 지워지지 않습니다. 자세한 범위는 [Sherpa 제거 안내](plugins/sherpa/README.md#update-and-removal)를 확인합니다.

## 호환 플러그인

기존 설치를 갑자기 깨뜨리지 않도록 `apple-calendar@xiyo`와 `message-pipeline@xiyo`는 한시적으로 유지합니다. 신규 사용자는 `sherpa@xiyo`만 설치합니다.

Sherpa와 호환 플러그인을 함께 켜면 스킬 트리거가 겹칠 수 있습니다. 새 작업에서 Sherpa를 검증한 뒤 기존 설치를 제거합니다.

## 저장소 구조

- `.agents/plugins/marketplace.json`: Codex 마켓플레이스 카탈로그
- `.claude-plugin/marketplace.json`: Claude Code 마켓플레이스 카탈로그
- `plugins/sherpa/`: 정식 자기완결형 플러그인과 내부 전문 스킬
- `plugins/apple-calendar/`, `plugins/message-pipeline/`: 마이그레이션 기간 호환 패키지
- `catalog-policy.json`: 정식·호환 패키지 정책
- `scripts/`: 카탈로그·버전·소스 동기화·저장소 검사

마켓플레이스 매니페스트가 기계 판독 카탈로그의 정본입니다. CI는 순서, 구성, 번들 스킬, 런타임 고정 버전, 문서 링크, 호환 소스 동기화를 검사합니다.

## 기여와 지원

Pull Request를 열기 전에 [기여 안내](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md)를 읽습니다. 버그와 기능 요청은 이슈 양식을 사용합니다. 취약점은 [보안 정책](https://github.com/XIYO/.github/blob/main/SECURITY.md)에 따라 비공개로 제보합니다.

## 라이선스

[MIT](LICENSE) © 2026 XIYO
