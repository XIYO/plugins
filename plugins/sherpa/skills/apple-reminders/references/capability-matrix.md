# 미리 알림 자동화 능력 매트릭스

이 문서는 미리 알림 자동화 도구를 고르는 기준을 정리한다. macOS 업데이트로 사설 인터페이스의 동작이 달라질 수 있으므로 실제 변경 전에는 Sherpa의 `scripts/doctor.sh reminders`와 좁은 범위의 읽기 작업으로 다시 확인한다. RemCTL doctor 원문은 로컬 경로와 리스트 정보를 포함하므로 모델 출력으로 전달하지 않는다.

## 왜 창구가 셋으로 갈렸나 (역사)

- **2012** — 리마인더 앱 + AppleScript 스크립팅 딕셔너리(sdef) 출시. 이후 **AppleScript sdef는 동결**(신기능 백포트 안 함).
- **2017** — 애플이 Workflow 인수 → **Shortcuts**를 자동화 표준으로 채택.
- 이후 태그(2021)·섹션/컬럼뷰(2023, iOS17)·하위작업·그룹 = **전부 App Intents(=Shortcuts)로만 추가.** AppleScript·EventKit엔 안 열음.

결론: AppleScript는 죽지 않았지만 **레거시로 동결**, 신규 투자는 Shortcuts로. 조직 기능(태그/섹션/하위작업/그룹)은 리마인더 앱 **사설 스키마**에 저장되고 **App Intents / ReminderKit로만** 노출된다. 그래서 데이터 프레임워크(EventKit)로 깊이 파도 조직 기능엔 못 닿는 역설. **이 벽을 뚫는 게 RemCTL**(ReminderKit 사설 프레임워크 + SQLite 토큰맵을 조합해 동기화 안전하게 조직기능을 씀).

## 전체 매트릭스

| 기능 | AppleScript | EventKit | Shortcuts | **RemCTL** |
|------|:--:|:--:|:--:|:--:|
| 리스트 생성/리네임/삭제 | ✅ | ✅ | △/❌ | ✅ |
| 리스트 색 | ✅ | ✅ | – | ✅ |
| 리스트 이모지(emblem) | ✅ | ❌ | ❌ | ✅ |
| 항목 생성/수정/삭제 | ✅ | ✅ | 생성만 | ✅ |
| 항목 리스트간 이동 | ✅ | ✅ | ❌ | ✅(경계넘으면 clone-delete) |
| 마감/우선순위/메모/URL | ✅ | ✅ | ✅ | ✅ |
| 플래그(flagged) | ✅ | ❌ | ✅ | ✅ |
| 반복/위치/다중알람 | ❌ | ✅ | ✅/일부 | ✅ |
| 대량 배치 속도 | 느림 | 빠름 | 중간 | 빠름 |
| **태그** | ❌ | ❌ | 구형액션 | **✅** |
| **하위작업** | 읽기만 | ❌ | 신형액션 | **✅ (2단 한계)** |
| **섹션에 항목 넣기** | ❌ | ❌ | 신형액션 | **✅** |
| **섹션 생성/삭제** | ❌ | ❌ | ❌ | **✅** |
| **그룹(폴더) 생성/삭제** | ❌ | ❌ | ❌ | **✅** |
| 비-iCloud 계정(Outlook) | ✅ | ✅ | – | ❌ iCloud만 |

→ **RemCTL = iCloud 기반 조직 기능과 대량 정리의 우선 선택.** 비-iCloud 계정은 AppleScript를 사용한다. 상세·설치·제약은 `remctl.md`.

## 근거와 확인 방법

### AppleScript 한계 — sdef 원문
`sdef /System/Applications/Reminders.app`로 확인할 수 있는 공개 모델:
- 커스텀 명령: **`show` 하나뿐**(UI에 항목 띄우기). 나머지는 범용 Cocoa 표준(make/delete/move/get/set).
- 클래스: **`account` / `list` / `reminder` 셋뿐.** section·tag·group 클래스 **없음.**
- `list` 속성: `id, name, container, color, emblem`. 담는 요소: `reminder` 하나.
- `reminder` 속성: `name, id, container, creation date, modification date, body, completed, completion date, due date, allday due date, remind me date, priority, flagged`.
- `reminder.container` 는 `list` **또는 `reminder`** 가능 → 모델은 하위작업을 알지만 **container가 읽기전용**이라 만들 수 없음.
- 우선순위 인코딩: **0=없음, 1–4=높음, 5=중간, 6–9=낮음.**

대표적인 런타임 확인 결과:
```
make new section ... → "section is not defined"
every group        → "Expected class name" (group 클래스 없음)
이름에 "#태그" 넣어 생성 → 리터럴 텍스트로 회수(진짜 태그 안 됨)
```

### EventKit 한계 — 프레임워크 런타임 반사
`class_copyPropertyList`로 `EKReminder`와 `EKCalendarItem`의 공개 속성을 확인하면:
- 있음: `title, notes, calendar, completed, completionDate, dueDateComponents, startDateComponents, priority, alarms, recurrenceRules, URL, location, attachments, attendees, creationDate, lastModifiedDate`.
- **없음: `tags`, `flagged`, `section`, 공개 `subtasks`** (`parentID`는 내부 전용).
- `EKCalendar`(리스트): `color / CGColor` 있음(색 됨), **emblem/이모지 없음.**

→ EventKit은 시간·반복·위치·속도엔 강하나 **조직 기능·플래그·이모지엔 못 닿음.**

### Shortcuts
- **구형 "Add New Reminder"**: Priority·URL·**Tags**·Notes 지원, **섹션 없음.**
- **신형 "Create Reminder"**(iOS 18~, macOS 26 탑재): `Target List`·**`List Section`**·**`Subtasks`**·Due/All-Day 분리. → 섹션에 항목 넣기·하위작업 자동화 가능. 단 섹션 "생성"은 여전히 불가(앱에서 먼저 만들어야 함).
- iOS 26 신규 리마인더 액션은 "Show Quick Reminder" 하나 — 리스트/섹션/그룹 생성 액션 없음.
- 문서: Matthew Cassinelli(전 애플 Shortcuts 팀), 9to5Mac(2025-12), Apple support/125148.

### RemCTL — 조직 기능 경로
`github.com/viticci/remctl`. 읽기=SQLite 직접, 기본쓰기=EventKit, **조직기능쓰기=ReminderKit 사설(`--private`)**. 섹션 멤버십은 SQLite+CRDT 토큰맵을 다룬다. 사용 전에 확인할 핵심 제약:
- **iCloud 스토어만 읽는다** — Outlook/Exchange 계정 리스트는 `list not found`(그건 AppleScript로). 계정 순회로 확인: `tell application "Reminders" ... repeat with a in accounts`.
- **하위작업 2단 한계** — 하위작업에 또 하위작업 = `does not support subtasks on this macOS version`.
- **하위작업 순서** — 한 edit에 `--subtask` 여러 개 = 스크램블. 순차호출=순서보존. 앱은 최신을 위에 표시(역순 추가로 오름차순 표시).
- **부모(하위작업 有) 이동** — `--list`+`--section` 한 방 불가. 이동(clone-delete→새 id) 후 새 id에 섹션.
- **완료 항목도 이동·섹션배정 시 완료상태 보존.**
설치·명령·워크플로 → `remctl.md`.

### 원본 DB — 직접 쓰지 않는다
저장소: `~/Library/Group Containers/group.com.apple.reminders/Container_v1/Stores/` 에 `Data-local.sqlite` + `Data-<UUID>.sqlite`(계정/CloudKit 존별) 여러 개 = **NSPersistentCloudKitContainer**.
- **읽기**: 복사본으로 안전. 섹션·태그·하위작업까지 다 보임(`ZREMCDREMINDER`, 태그 `ZREMCDHASHTAGLABEL`, 섹션멤버십 `ZMEMBERSHIPSOFREMINDERSINSECTIONSASDATA`).
- **쓰기**: 동기화 상태와 CRDT 토큰맵, WAL을 함께 고려해야 하므로 직접 쓰기를 금지한다. 조직 기능 변경은 RemCTL의 명시적 어댑터를 사용하고 결과를 다시 읽어 검증한다.

## 도구 선택 요약
- **AppleScript** = 앱 원격조종(Apple Events). 즉석·느림·모든 계정. sdef 동결로 조직기능 없음.
- **EventKit** = 동기화 엔진 통한 데이터 접근. 코드·빠름. 조직기능·플래그·이모지 없음.
- **Shortcuts** = App Intents 신형. 태그·섹션배치·하위작업 일부(단축어 앱제작 필요, 생성 불가).
- **RemCTL** = 위 셋 + ReminderKit를 묶어 조직 기능을 안전하고 빠르게 제공한다. iCloud 기반 대량 정리와 조직 기능에 우선 사용한다.
