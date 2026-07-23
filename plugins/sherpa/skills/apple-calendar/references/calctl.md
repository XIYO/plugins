# calctl 명령 참고

## 공통

```bash
calctl <command> [options]
calctl <command> ... --json
CALCTL_LOG_LEVEL=debug calctl <command> ...
```

로그 레벨은 `debug`, `info`, `warn`, `error`이며 기본값은 `warn`이다. 정상 데이터는 stdout, 로그와 오류는 stderr로 분리한다.

## 권한·목록

```bash
calctl doctor
calctl authorize
calctl sources [--json]
calctl calendars [--source iCloud] [--json]
```

`authorize`는 최신 전체 접근 API를 호출한다. 읽기·검색·수정이 필요하므로 쓰기 전용 권한으로는 부족하다.

## 캘린더 생성

```bash
calctl calendar-create <name> --source <source-name-or-id> [--color '#RRGGBB'] [--dry-run] [--json]
```

- 소스 지정은 필수다. 잘못된 계정에 평면 캘린더가 생기는 것을 방지한다.
- 같은 소스에 같은 이름이 있으면 실패한다.
- `--dry-run`은 저장하지 않고 해석 결과만 출력한다.

## 캘린더 수정·삭제

```bash
calctl calendar-edit <name-or-id> [--source <source>] \
  [--title <new-name>] [--color '#RRGGBB'] [--dry-run] [--json]

calctl calendar-delete <name-or-id> [--source <source>] --force [--json]
```

캘린더는 소스 간 이동할 수 없다. 이름·색만 수정한다. 캘린더 삭제는 그 안의 일정도 함께 제거하므로, 먼저 `events`로 내용을 조사하고 사용자 승인을 받은 경우에만 `--force`를 붙인다.

## 일정 추가

```bash
# 시간 일정
calctl add <title> --calendar <name-or-id> [--source <source>] \
  --start '2026-08-15T14:00' [--end '2026-08-15T15:00']

# 하루 종일 일정
calctl add <title> --calendar <name-or-id> [--source <source>] \
  --date 2026-08-15

# 반복
calctl add <title> --calendar <name-or-id> --source iCloud \
  --date 2026-08-14 --repeat monthly [--interval 1] \
  [--until 2033-07-14 | --count 12]
```

선택 옵션: `--notes`, `--location`, `--url`, `--dry-run`, `--json`. `--end`를 생략하면 시간 일정은 1시간, 하루 종일 일정은 시작일 하루다. 하루 종일 일정의 `--end`는 마지막으로 포함할 날짜이므로 하루짜리는 생략하거나 시작일과 같게 둔다. 날짜에 시간대가 없으면 현재 macOS 시간대로 해석한다.

macOS 26.5 실측상 EventKit은 하루 종일 일정의 종료일을 포함 날짜의 23:59:59로 정규화한다. `start + 1일`을 종료일로 넣으면 이틀 일정이 되므로 일반적인 exclusive-end 가정을 적용하지 않는다.

## 검색·상세

```bash
calctl events [--calendar <name-or-id>] [--source <source>] \
  [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--query <text>] [--limit 200] [--json]
calctl show <event-id> [--json]
```

- `--to`는 검색 종료 경계로 사용한다.
- 반복 일정은 지정 범위의 발생 건으로 펼쳐져 나온다.
- `event-id`는 특정 발생 건을, `calendar-item-id`는 계열 식별에 도움을 준다.
- `--query`는 제목과 메모를 함께 검색한다. 제목은 `요금 청구`처럼 짧게 두고, 메모 첫 줄에는 계약에 맞는 세 항목 요약을 둔다. 상세 구문과 필드는 `metadata-schema.md`를 따른다.

## 수정·이동

```bash
calctl edit <event-id> [--title <text>] [--notes <text> | --clear-notes] \
  [--url <absolute-url> | --clear-url] \
  [--calendar <name-or-id> --source <source>] [--span this|future] [--dry-run]

# 기존 반복 규칙의 종료만 변경
calctl edit <event-id> --until 2026-12-09 --span future
calctl edit <event-id> --count 5 --span future

calctl move <event-id> --calendar <name-or-id> [--source <source>] \
  [--span this|future] [--dry-run]
```

반복 일정은 `--span`을 생략하면 실패한다. 단일 일정은 기본 `this`다. `--until`과 `--count`는 기존 반복 주기와 상세 조건을 보존하고 종료 조건만 교체하며, 둘을 함께 쓸 수 없다. 기존 일정의 링크는 `--url`로 추가·교체하고 `--clear-url`로 제거한다.

## 삭제

```bash
calctl show <event-id>
calctl delete <event-id> [--span this|future] --force
```

삭제 전에 반드시 `show`로 대상을 확인하고 사용자 승인을 받는다. 반복 일정은 `--span`이 필수다.

## 오류 해석

- `not-determined`: `calctl authorize` 실행 후 macOS 권한 창에서 허용
- `denied`: 시스템 설정 > 개인정보 보호 및 보안 > 캘린더에서 허용
- `write-only`: `authorize`로 전체 접근 요청
- `ambiguous`: 같은 이름이 여러 소스에 있음. `--source` 또는 ID 지정
- `source not found`: `calctl sources`의 정확한 제목이나 ID 사용

EventKit이 같은 이름의 iCloud 소스를 둘 이상 돌려주더라도 실제 이벤트 캘린더가 있는 소스가 정확히 하나면 그 소스를 선택한다. 둘 이상에 캘린더가 있으면 임의 선택하지 않고 `ambiguous`로 실패한다.
