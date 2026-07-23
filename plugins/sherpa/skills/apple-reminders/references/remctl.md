# RemCTL — 미리 알림 구조 관리

`github.com/viticci/remctl` (MIT, Federico Viticci/MacStories). AppleScript·EventKit가 못 하는 조직기능을 **동기화 안전하게** 처리하는 오픈소스 CLI. AI 에이전트용으로 설계됨(자체 SKILL.md 포함).

## 아키텍처
3계층으로 각 작업에 맞는 경로를 씀:
- **읽기** → 로컬 iCloud **SQLite 직접**(수십 ms, 섹션·태그·하위작업·첨부까지 다 보임)
- **기본 쓰기** → **EventKit**(항목 CRUD·반복·마감 — 표준 동기화)
- **조직 기능 쓰기** → **ReminderKit 사설 프레임워크**(`--private`) + 섹션 멤버십은 SQLite+CRDT 토큰맵. remindd 동기화 상태머신과 안 부딪히게 조율.

## 설치
```bash
bash <sherpa-plugin-root>/scripts/install-runtime.sh reminders
# 기본 설치 경로는 ~/.local/bin/remctl
bash <sherpa-plugin-root>/scripts/doctor.sh reminders  # 민감 경로를 숨긴 상태 점검
```
**요구사항**: `swiftc`(EventKit·권한 헬퍼), `clang`(remctl-private=ReminderKit), python3. 권한: **Reminders(EventKit) + 자동화 + Full Disk Access(SQLite 읽기)**. 번들 doctor는 성공·실패·경고 개수만 보고하고 DB 경로와 리스트 정보는 숨긴다. FDA 없으면 `--via-eventkit` 폴백(제한적: 섹션·태그·id 없음).

## 핵심 명령
```bash
R="$(command -v remctl || printf '%s' "$HOME/.local/bin/remctl")"
# 읽기
$R lists                          # 트리(그룹·섹션·개수)
$R lists --json                   # 구조(id·counts·children·objectUUID)
$R show "리스트"                   # 미완료만(기본)
$R show "리스트" --completed       # 완료 포함
$R show "리스트" --completed --json
$R info <id>                      # 단건 상세(List·Section·Status·Deep link)
$R sections                       # 전체 섹션
$R subtasks <id>                  # 하위작업
$R stats                          # Total/Active/Completed/Lists/Sections
$R export -l "리스트" --format json|csv
# 항목
$R add "제목" -l "리스트" [-d "2026-12-28 10:00"] [-p high] [-f]
$R add "제목" -l "리스트" --private --section "섹션" --subtask "a" -t "Food"
$R edit <id> --title "새 제목"
$R edit <id> --list "리스트"                 # 이동(경계 넘으면 clone-delete→새 id 반환)
$R edit <id> -d 2026-12-28    # 날짜만=하루종일 / "…HH:MM"=시간지정 / clear=제거
$R edit <id> --private --section "섹션"       # 섹션 배정
$R edit <id> --private --subtask "스텝"       # 하위작업 추가
$R edit <id> --private -t "Food,Travel"       # 태그 추가(--set-tags/--clear-tags/--remove-tag)
$R edit <id> --private --flagged              # 진짜 플래그
$R done <id> / $R undone <id> / $R flag <id> / $R delete <id> --force
# 구조
$R list-create "이름" [--color …]
$R list-rename … / $R list-delete --list-id <n> --force
$R section-create "이름" -l "리스트" --private
$R section-rename / $R section-delete
$R group-create / $R group-edit / $R group-delete --group-id <n> --private --force
```

## 운영 시 주의점

### 1. iCloud 스토어만 읽는다
RemCTL은 iCloud SQLite만 읽음. **Outlook/Exchange 등 다른 계정 리스트는 `Error: list not found`.** 계정 확인은 AppleScript로:
```bash
osascript -e 'tell application "Reminders"
  repeat with a in accounts
    log (name of a); repeat with l in lists of a
      log "  " & (name of l)
    end repeat
  end repeat
end tell'
```
비-iCloud 계정 작업은 AppleScript/EventKit으로 처리.

### 2. 하위작업은 2단까지
하위작업(#B, 부모 #A의 자식)에 또 하위작업 추가 → `ReminderKit reminder change item does not support subtasks on this macOS version`. **설계: 부모 1개 + 평면 하위작업.** 3단 그룹핑이 필요하면 부모를 여러 개 두고 각자 하위작업.

### 3. 하위작업 순서 = 순차추가 + 역순
- `edit <id> --private --subtask a --subtask b …` **한 번에 여러 개 = 순서 스크램블**.
- **하나씩 순차 호출**하면 추가 순서대로 저장됨.
- 앱은 **나중에 추가한 걸 위에** 표시 → 화면에서 1→N으로 보이려면 **N→1 역순으로 추가**.
- 번호 프리픽스 제거 등 제목 변경은 `edit <id> --title "…"` → **위치 유지**(순서 안 깨짐).

### 4. 부모(하위작업 有) 이동은 2단계
`edit <id> --list X --private --section Y` 한 방 → `Error: moving a parent reminder with subtasks cannot currently be combined with other edits`. 해결:
```bash
newid=$($R edit <id> --list "대상 리스트" --json | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")
$R edit "$newid" --private --section "대상 섹션"
```
이동 JSON: `{"id":새, "oldId":원, "subtasksMoved":N, "method":"clone-delete"}`. 하위작업도 함께 옮겨짐.

### 5. 이미 그 리스트 안이면 이동 없이 섹션만
`edit <id> --private --section X` — 부모여도 OK(이동 아님이라 4번 제약 없음). **완료 항목도 완료상태 유지.**

### 6. 삭제
- `list-delete`: name 또는 `--list-id` + `--force`.
- `group-delete`: **자식 리스트를 먼저 top-level로 올린 뒤 그룹만 삭제**(자식·항목 보존). 자식까지 없애려면 **자식 리스트를 먼저 `list-delete`** 하고 빈 그룹을 지운다.
- id 네임스페이스: 리스트 id와 리마인더 id가 숫자로 겹칠 수 있음 → 삭제는 `--list-id`/`--group-id`로 명시.

## 대규모 정리 워크플로
대량으로 "옛 리스트/그룹 → 새 체계 섹션"으로 옮길 때:
1. **감사**: `lists`, `stats`, 계정 순회(AppleScript)로 iCloud/타계정·그룹·섹션·개수 파악.
2. **수집**: 옮길 항목 `show <list> --completed --json` → `id\t제목` 뽑기.
3. **분류 설계**: id→섹션 매핑을 짜고, **섹션 먼저 생성**(`section-create … --private`).
4. **1개 검증**: 한 항목 `edit <id> --list X --private --section Y` → `info`로 List·Section·Status 확인.
5. **일괄 이동**: 평면 항목은 한 방(`--list --private --section`), **부모(하위작업 有)는 실패** → 걸러서 2단계(함정4)로.
6. **원본 비었나 확인**: `lists --json` counts=0.
7. **삭제**: 자식 리스트 → 그룹 순.
8. **검증**: `show --completed --json`으로 섹션별 개수 집계.

bash 스크립트는 **파일로 써서 `bash file` 실행**(zsh는 `${!arr[@]}` 연관배열 안 됨 → 함수+positional args로).

## 이동·복원·중복 주의사항

대량 이동·정리 시 이걸 모르면 데이터가 중복·오해로 꼬인다.

### clone-delete 이동의 실체
`edit <id> --list X` 로 리스트를 옮기면(계정/컨테이너 경계, **또는 하위작업 있는 부모**) **clone-delete**가 일어난다: 새 항목 복제(새 numericId) + **원본은 최근 삭제됨(trash)으로**. 하위작업도 함께 복제(`subtasksMoved`). 반환 JSON: `{"id":새UUID,"numericId":새숫자,"oldId":원,"subtasksMoved":N,"method":"clone-delete","originalDeleted":true}`.

### ★최악의 실수 — trash 원본을 "복원"하지 마라
clone-delete 후 최근 삭제됨에는 **옮긴 원본과 하위작업**이 남을 수 있다. 이를 별개의 삭제 이력으로 보고 재생성하면 이미 대상 목록에 있는 복제본과 중복된다. 복원을 검토하기 전에 이동 결과와 수정 시각을 대조하고, 해당 작업에서 생긴 원본이면 건드리지 않는다.

### id vs numericId 필드 불일치
- `add --json` → `numericId`(명령용 숫자) + `id`(UUID). done/edit엔 **numericId** 써야 함(UUID면 실패).
- `show`/`lists --json` → **`id`가 곧 숫자 id**(numericId 키 없음).
- 캡처는 항상 `x.get('numericId') or x.get('id')`.

### 최근 삭제됨(trash): 읽기 O, 비우기 X
SQLite `ZREMCDREMINDER.ZMARKEDFORDELETION=1`. DB 복사본으로 **읽기 가능**(제목·수정일로 오늘/예전 구분). **프로그램으로 영구삭제(비우기)는 불가** — RemCTL delete는 trash 항목에 실패, raw 삭제는 CloudKit 손상. → **앱에서 "영구 삭제"** 또는 30일 자동 purge. 대량 이동은 trash를 크게 부풀리니 미리 사용자에게 고지.

### 완료 항목은 앱에서 숨김
`show`는 미완료만 표시; 완료 포함은 `--completed`. 사용자는 완료가 안 보여 "지웠냐"고 오해함 → `stats`(Total/Completed)로 보존 증명.

### 중복 제거(dedup)
소스 리스트가 겹치면(복사본 리스트 등) 합칠 때 전부 2배. `show --completed --json` → 제목 정규화(공백·기호·"이케아"·"할인"·"구매" 제거)로 그룹핑 → 그룹당 1개(**활성 우선, 없으면 최소 id**) 남기고 나머지 삭제. 삭제도 trash행이니 마지막에 앱 비우기.

### 부모 delete 후 검증
부모를 `delete`하면 하위작업도 함께 사라짐(그 하위작업 id 재삭제는 실패=이미 없음). 단 카운트가 기대만큼 안 줄 수 있으니 **삭제 후 반드시 중복 재집계로 검증**.

### zsh 함정 (Bash 툴은 zsh 실행)
`for x in $VAR`는 zsh에서 **단어분리 안 됨**(통째 1회 실행), `${!arr[@]}` 연관배열도 bash 전용. → 반복·연관배열 쓰는 정리 스크립트는 **파일로 써서 `bash file`** 로 실행.

## 안전 원칙
- RemCTL은 **raw SQLite 쓰기를 안 함**(읽기만) — 쓰기는 EventKit/ReminderKit 경유라 동기화 안전.
- `--private`는 사설 ReminderKit → **OS 업데이트로 깨질 수 있음**(개인 로컬용은 무방, doctor로 재확인).
- 대량 삭제/이동 전 **원본 counts 확인**, 후 **중복 재집계**(비가역).
- 복원(un-delete)은 RemCTL에 없음 → **앱에서 Recover(30일 내)** 또는 **재생성**(단 trash 원본과 중복 안 되게 위 함정 준수).
