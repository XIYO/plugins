---
name: apple-calendar
description: macOS Apple Calendar를 화면 조작 없이 EventKit CLI로 관리하고 일정 메모를 버전형 스키마로 생성·검증하는 스킬. iCloud 소스·캘린더 조회, 캘린더 생성, 일정 CRUD·검색·이동, 하루 종일·반복 일정, 통신·금융 메타데이터, `calctl`, `calmeta`, "애플 캘린더에 등록", "iCloud 캘린더", "일정 메모 검사" 요청에 사용한다.
---

# Apple Calendar

Calendar 앱 화면이나 AppleScript 전체 순회 대신 공개 EventKit API를 사용하는 `calctl`을 사용한다. 구조화된 일정 메모는 Rust 런타임 `calmeta`로 생성·검증한다.

## 실행 전

```bash
SHERPA_BIN="${SHERPA_INSTALL_ROOT:-$HOME/.local}/bin"
CALCTL="$SHERPA_BIN/calctl"
CALMETA="$SHERPA_BIN/calmeta"
test -x "$CALCTL" || CALCTL="$(command -v calctl || true)"
test -x "$CALMETA" || CALMETA="$(command -v calmeta || true)"
test -x "$CALCTL" && test -x "$CALMETA"
"$CALCTL" doctor
```

- 이 `SKILL.md`가 있는 디렉터리에서 `../..`를 Sherpa 플러그인 루트로 해석하고 `scripts/doctor.sh calendar`로 버전과 권한을 확인한다.
- 관리 런타임이 없거나 버전이 다르면 설치 내용을 알린 뒤 플러그인 루트에서 `scripts/install-runtime.sh calendar`를 실행한다. 기본 설치 경로는 `~/.local/bin`이다.
- 권한이 `not-determined`면 `calctl authorize`를 실행하고 사용자가 macOS 캘린더 전체 접근을 허용하게 한다.
- 권한이 `denied`면 시스템 설정에서 `calctl`의 캘린더 권한을 켜야 한다.
- 캘린더 이름은 계정마다 중복될 수 있으므로 쓰기 명령에는 가능하면 `--source iCloud`를 붙인다.
- 전체 CLI는 `references/calctl.md`, 메모 계약은 `references/metadata-schema.md`를 읽는다.

## 기본 흐름

1. `$CALCTL calendars --source iCloud`와 범위가 좁은 `$CALCTL events`로 대상과 중복을 확인한다.
2. 등록·수정·이동 전에 대상, 날짜, 반복 범위, 목적 캘린더를 한 번 요약한다.
3. 통신·금융 일정이면 `$CALMETA render`로 메모를 만들고 `$CALMETA validate`로 검증한다.
4. `$CALCTL add` 또는 `$CALCTL edit`에 검증된 메모와 URL을 넣는다.
5. 같은 범위를 다시 조회해 제목, 날짜, 반복 종료, 캘린더, URL을 확인한다.

## 분류와 제목

- 큰 분류는 별도 캘린더로 관리한다. Calendar에는 Reminders 같은 태그가 없으므로 `#카테고리`를 분류 기능처럼 쓰지 않는다.
- 일정이고 목적 캘린더가 명확하면 해당 캘린더, 애매하면 iCloud `베이스캠프`에 둔다.
- 할 일이거나 일정인지도 애매하면 Apple Reminders의 `베이스캠프`에서 관리한다.
- 제목은 `요금 청구`, `카드 결제`처럼 짧은 명사구로 쓴다. 공급자·결제수단·식별자를 제목에 길게 연결하지 않는다.
- 서비스 홈페이지는 메모에 중복하지 않고 Calendar 이벤트의 URL 필드에 둔다.

## 메타데이터

메모는 첫 줄 요약, 구역별 필드, 마지막 스키마 선언으로 구성한다.

```text
요금 청구일 · 예시모바일 · 예시카드 1234

서비스
• 제공자: 예시모바일
• 상품: 데이터 11GB
• 종류: 휴대전화

납부
• 청구일: 매월 9일
• 결제수단: 예시카드 · 끝 1234
• 청구서: billing@example.com

계정
• 로그인: 연결 계정

@schema: xiyo.calendar.telecom-billing@1
```

- 화면 메뉴, 푸터, 중복 안내, 이전 납부수단, 변경 이력, 반영 정책은 일정 메모에 쌓지 않는다.
- 청구서는 주소만 `청구서`에 기록한다. `이메일 상세` 같은 UI 문구나 채널 설명을 별도 필드로 만들지 않는다.
- 새 메모는 반드시 `calmeta render`로 만들고, 외부에서 받은 메모는 `calmeta validate`를 통과시킨다.
- 버전 규칙과 지원 필드는 `references/metadata-schema.md`가 정본이다.

## 안전 규칙

- 반복 일정 수정·이동은 `--span this|future`를 명시한다.
- 삭제는 `show`로 정확한 대상을 확인하고 사용자 확인을 받은 뒤 `delete ... --force`로 실행한다.
- 캘린더 삭제는 내부 일정도 함께 제거한다. 범위를 조사하고 사용자 확인을 받은 뒤 실행한다.
- 캘린더 이름만으로 하나를 확정할 수 없으면 소스나 캘린더 ID를 추가로 확인한다. 임의 선택하지 않는다.
- 로그에는 일정 본문, 이메일, 전화번호, 카드·계좌 번호를 남기지 않는다.

## 구현 경계

- `calctl`: EventKit 접근, 권한, 캘린더와 일정 CRUD를 담당하는 어댑터다.
- `calmeta`: EventKit과 독립적인 Rust 파서·검증기다. 메모 계약 외의 Calendar 상태를 변경하지 않는다.
- 스킬은 정책과 실행 순서를 담당한다. 파싱 규칙을 스킬 프롬프트로 흉내 내지 않고 `calmeta` 결과를 사용한다.
