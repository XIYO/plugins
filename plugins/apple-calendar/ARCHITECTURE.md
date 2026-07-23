# Architecture

## 경계

```text
apple-calendar skill
├── 정책: 분류, 제목, 안전 확인, 실행 순서
├── calctl: EventKit 권한과 Calendar 상태 변경
└── calmeta: 순수 Rust 메타데이터 파싱·검증·렌더링
```

`calmeta`는 Calendar 저장소에 접근하지 않는 결정론적 엔진이다. 따라서 스키마 회귀 테스트는 iCloud 계정이나 macOS 권한 없이 실행할 수 있다. `calctl`만 EventKit과 TCC 권한 경계를 가진다.

## 버전 경계

- 플러그인, `calmeta`: 함께 배포하는 SemVer `MAJOR.MINOR.PATCH`
- `calctl`: EventKit 어댑터의 독립 SemVer. Swift의 `calctlVersion`과 `Info.plist`의 `CFBundleShortVersionString`은 반드시 일치한다.
- 일정 메모 계약: 호환성만 표현하는 `MAJOR.MINOR`

플러그인 릴리스는 번들한 `calctl` 버전과의 호환성을 검증한다. 메모에는 변경 이력이 없으므로 PATCH를 저장하지 않는다. 데이터 해석이 깨지는 변경은 MAJOR, 선택 필드 추가는 MINOR다.
