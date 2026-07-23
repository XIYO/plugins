# KakaoTalk 컨텍스트 소스

카카오 공식 API가 제공하지 않는 개인 대화 내역 조회를 Mac에 이미 캐시된 로컬 DB에서 수행한다. 읽기는 UI 자동화 없이 가능하지만, DB는 SQLCipher로 암호화되어 일반 `sqlite3`로 열리지 않는다.

## 안전 원칙

- 기본 동작은 읽기 전용이다. `login`, `harvest`, `inspect`, `sync --webhook`을 실행하지 않는다.
- 사용자가 현재 대화에서 특정 답장 전송을 요청한 경우에만 `sherpa context reply`의 승인 흐름을 사용한다. `kakaocli send`를 직접 실행하지 않는다.
- 카카오 계정 비밀번호를 입력·저장하지 않는다. 기존 로컬 DB 읽기에는 계정 비밀번호가 필요 없다.
- 파생 DB 키, IOPlatformUUID, 카카오 userId를 명령 인자·로그·답변에 노출하지 않는다. 특히 `--key`를 셸 인자로 넘기는 우회법을 사용하지 않는다.
- `status`와 `auth` 자체가 UUID·userId·DB 해시명을 출력하므로 원문을 모델 대화나 로그에 그대로 노출하지 않고 실행 단계에서 마스킹한다.
- 메시지 전체를 무차별 덤프하지 않는다. 채팅방·검색어·기간·건수를 좁혀 필요한 내용만 읽고, 사용자에게는 요약과 액션만 반환한다.
- CLI 출력이 Codex·Claude 입력으로 들어가면 메시지 내용이 해당 모델 제공자에게 전달될 수 있음을 구분한다. 로컬 DB 접근 자체와 모델 전송은 별개의 개인정보 경계다.
- 읽기 중에도 원본 DB와 `-wal`·`-shm`을 복사·수정·체크포인트하지 않는다. 항상 SQLCipher의 `SQLITE_OPEN_READONLY` 경로를 사용한다.
- 자체 래퍼를 만들면 `LOG_LEVEL`로 레벨을 조절하고, 로그에는 작업명·건수·소요 시간만 남긴다. 메시지 본문·참여자 이름·키는 기록하지 않는다.

## 승인형 텍스트 답장

답장은 공식 카카오 API가 아니라 KakaoTalk Mac 화면 자동화로 전송된다. 앱이 전면으로 이동하거나 대상 대화가 열리면서 읽음 상태가 바뀔 수 있음을 미리 알린다. 텍스트 한 건만 지원하며 iMessage, 첨부, 반응, 일괄 전송은 거부한다.

1. 답장 대상의 실제 표시 이름을 `identities` 또는 좁은 `kakaocli chats --json` 조회로 확인한다. 추측한 이름을 사용하지 않는다.
2. 답장 문구를 작성하되 전송하지 않는다.
3. Context의 Rust 승인 서비스로 미리보기를 만든다. 본문은 명령 인자가 아니라 표준 입력으로 전달한다.

   ```bash
   printf '%s' "$REPLY_TEXT" | sherpa context reply prepare \
     --via kakaotalk --conversation "<정확한 표시 이름>"
   ```

4. 출력된 채팅방, 본문, 만료 시각, 확인 필요 여부를 그대로 사용자에게 보여준다. `prepare`는 채팅 목록만 읽고 전송 UI를 열지 않는다.
5. 사용자가 그 미리보기를 명시적으로 승인할 때까지 멈춘다. 과거의 일반적인 “답장 가능하게 해라” 요청은 개별 메시지 승인이 아니다.
6. 승인 토큰이 유효한 동안 같은 본문을 표준 입력으로 다시 전달한다.

   ```bash
   printf '%s' "$REPLY_TEXT" | sherpa context reply confirm --token "<preview token>"
   ```

7. `status=dispatched`일 때만 UI 전송 명령이 성공했다고 보고한다. 이는 상대방 단말의 전달·열람 보장이 아니다.
8. 사용자가 취소하면 토큰을 즉시 제거한다.

   ```bash
   sherpa context reply cancel --token "<preview token>"
   ```

승인 서비스는 정확히 일치하는 표시 이름이 하나여도 그 문자열을 포함하는 다른 채팅방이 있으면 거부한다. 미리보기는 기본 15분 뒤 만료되고 본문이 한 글자라도 달라지면 전송되지 않으며 성공한 토큰은 다시 쓸 수 없다. 실패가 실제 전송 전인지 확실하지 않으면 자동 재시도하지 말고 사용자에게 불확실성을 알린 뒤 새 미리보기와 승인을 받는다.

## 진단 순서

1. 앱과 버전을 확인한다.

   ```bash
   mdls -name kMDItemCFBundleIdentifier -name kMDItemVersion /Applications/KakaoTalk.app
   ```

2. 컨테이너 접근과 DB 존재만 확인한다. DB는 보통 아래 경로의 78자리 16진수 파일이며 같은 이름의 `-wal`, `-shm`이 있다.

   ```text
   ~/Library/Containers/com.kakao.KakaoTalkMac/Data/Library/Application Support/com.kakao.KakaoTalkMac/
   ```

3. `file is not a database`는 손상 판정이 아니다. 파일 첫 16바이트가 `SQLite format 3`이 아니고 일반 `sqlite3`가 실패하는 것은 SQLCipher DB의 정상 신호다.
4. `Operation not permitted` 또는 컨테이너 접근 거부면 실행 주체인 Terminal·iTerm·IDE·Codex 호스트에 macOS **전체 디스크 접근**을 사용자가 직접 허용해야 한다. 읽기에는 **접근성** 권한이 필요 없다.

## 읽기 도구 선택

현재 공개 구현은 [silver-flight-group/kakaocli](https://github.com/silver-flight-group/kakaocli)다. SQLCipher 키를 IOPlatformUUID와 로컬 plist의 계정 식별자로 파생하고 DB를 읽기 전용으로 열어 JSON을 출력한다.

설치 전 반드시 최신 릴리스와 열린 이슈·PR을 다시 확인한다. 2026-07-22 기준 Homebrew가 설치하는 `v0.6.0`에는 다음 미병합 문제가 있다.

- 최신 계정의 userId가 수억 단위이면 10초 단일 스레드 SHA-512 역탐색이 끝나지 않는다. 수정안: [PR #20](https://github.com/silver-flight-group/kakaocli/pull/20).
- SwiftPM 결과물이 SQLCipher가 아닌 시스템 SQLite에 링크되어 `PRAGMA key`가 무효가 될 수 있다. 수정안: [PR #18](https://github.com/silver-flight-group/kakaocli/pull/18).
- `AlertKakaoIDsList` 항목은 로그인 계정 userId가 아닐 수 있다. 후보가 DB 파일명과 모두 불일치하면 같은 후보를 반복하지 말고 병렬 복구가 포함된 버전으로 전환한다.

따라서 `brew install silver-flight-group/tap/kakaocli`를 맹목적으로 실행하지 않는다. 사용자가 설치를 요청하면 최신 upstream에 위 수정이 병합됐는지 확인하고, 미병합이면 검토된 읽기 전용 포크를 고정 커밋으로 빌드한다. 설치 후 링크를 검증한다.

2026-07-22에 검증한 미병합 패치 조합은 다음과 같다. 이후에는 먼저 upstream 병합 여부를 다시 확인하고, 여전히 미병합일 때만 이 고정 커밋을 사용한다.

```bash
git fetch origin pull/18/head:pr18 pull/20/head:pr20
git switch --detach 66fee27bf1f1e872765cf6ac3ab3753afdd2d6c4
git cherry-pick 84e5569 6bec974
swift test
swift build -c release
```

- `66fee27`: 병렬 SHA-512 userId 복구, 검증 가능한 JSON 캐시, 환경 변수 override
- `84e5569`: `ioreg` stdout를 먼저 비워 UUID 조회 교착 방지
- `6bec974`: SQLCipher include·link·rpath 명시
- `swift test` 8개 통과와 `otool -L`의 `libsqlcipher.dylib` 링크를 둘 다 확인해야 한다.

```bash
otool -L "$(command -v kakaocli)" | rg 'libsqlcipher|libsqlite3'
kakaocli status
kakaocli auth
```

`auth` 성공 전에 메시지 조회로 넘어가지 않는다. `file is not a database`가 나오면 키만 의심하지 말고 실제 링크 대상이 `libsqlcipher`인지 먼저 확인한다.

PR #20은 `~/.kakaocli/userid.json`에 userId·UUID·계정 해시를 캐시한다. 2026-07-22 테스트에서는 기본 권한이 디렉터리 `0755`, 파일 `0644`로 생성됐으므로 인증 직후 소유자 전용으로 제한한다. 캐시 내용을 출력하지 않는다.

```bash
chmod 700 ~/.kakaocli
chmod 600 ~/.kakaocli/userid.json
```

## 조회

검증된 바이너리에서만 다음 읽기 명령을 사용한다.

```bash
kakaocli chats --limit 20 --json
kakaocli messages --chat "채팅방 일부 이름" --since 1d --limit 100 --json
kakaocli search "검색어" --json
kakaocli schema
```

- 채팅방 이름과 검색어는 셸 문자열로 조립하지 말고 프로세스 인자 배열로 전달한다.
- `query`가 필요하면 사전 검토한 단일 `SELECT`, `WITH`, 읽기 전용 `PRAGMA`만 허용한다.
- 출력은 즉시 필요한 필드만 파싱하고 평문 파일로 저장하지 않는다.

## 대량 추출과 토큰 최적화

여러 채팅방이나 긴 기간을 모델 분석용으로 준비할 때는 이 스킬의 `sherpa context`를 사용한다. 이 문서는 인증·권한·읽기 도구 복구를 설명하고, `sherpa context sync kakaotalk`는 고정된 읽기 전용 쿼리로 원문을 로컬 아카이브에 멱등 동기화한다. 이후 공통 정규화·치환·CCT 내보내기는 아카이브만 읽는다.

- 요청 범위를 `sherpa context sync kakaotalk`로 한 번 동기화한 뒤 `sherpa context benchmark kakaotalk`로 본문 없는 토큰 통계를 확인한다. 분석이 승인된 뒤 `sherpa context pending kakao --thread K001 ...`처럼 미분석 방 하나만 내보낸다.
- KakaoTalk의 일반 텍스트 `type=1`에도 mention/linkify/bot용 `attachment` JSON이 흔히 존재한다. 값이 있다는 이유만으로 `@file` 또는 `@image`로 치환하지 않는다.
- 텍스트·이미지·영상·이모티콘·위치·답장·다중 사진 등 앱별 타입 해석은 KakaoTalk 소스 어댑터에서 끝낸다. 채팅 표현 치환과 교차 메시지 최적화는 앱별로 복제하지 않는다.

## 한계

- DB에는 Mac 앱이 실제로 동기화한 기록만 있다. Mac에서 열지 않은 방의 과거 메시지와 톡서랍/톡드라이브 제한 너머 기록은 DB 리더가 만들어낼 수 없다.
- 최신 메시지를 DB에 받으려면 카카오톡 앱이 실행·로그인되어 동기화해야 한다. 캐시 읽기 자체는 카카오톡 창 없이 가능하다.
- 일부 그룹 채팅 이름은 DB에서 `(unknown)`일 수 있다. `harvest`는 UI를 열고 읽음 상태에 영향을 줄 수 있으므로 명시적 승인 없이는 사용하지 않는다.
- 텍스트 외 사진·영상·이모티콘은 현재 공개 CLI에서 완전하게 렌더링되지 않는다.
- 카카오톡 업데이트 뒤에는 DB 경로·스키마·키 파생법이 바뀔 수 있으므로 `auth`와 최소 조회로 다시 검증한다.

## 확인된 근거

- 2026-07-22 검증 환경의 KakaoTalk `26.6.1`에서 컨테이너와 78자리 DB·WAL·SHM 파일을 직접 확인했고 일반 SQLite 조회는 암호화 때문에 실패했다.
- 같은 검증 환경의 `AlertKakaoIDsList` 후보 4개는 실제 DB 파일명 파생값과 모두 불일치해 upstream `v0.6.0` 자동 탐지 실패 조건에 해당했다.
- 위 고정 패치 조합을 macOS `26.5.1`, Apple Silicon, Swift `6.3.3`, SQLCipher `4.17.0`에서 검증했다. 최초 병렬 탐색을 포함한 `status`는 약 27초, 캐시 후 `auth`는 약 0.15초였고 SQLCipher로 19개 테이블을 열었다.
- 메시지 본문·참여자·식별자를 읽지 않는 `NTChatRoom`·`NTChatMessage` `COUNT(*)` 조회가 성공했다. `DatabaseReader`는 `SQLITE_OPEN_READONLY | SQLITE_OPEN_NOMUTEX`로 원본 DB를 연다.
- PR #20 캐시가 `0644`로 생성되는 문제를 확인했으며, 검증 환경에서는 즉시 디렉터리 `0700`·파일 `0600`으로 교정했다.
- macOS DB 복호화 방식은 [2023년 연구(DOI 10.13089/JKIISC.2023.33.5.753)](https://doi.org/10.13089/JKIISC.2023.33.5.753)와 kakaocli 구현에서 교차 확인한다.
- 카카오 공식 문서는 메시지 **발송** API만 제공하며 개인 대화 이력 조회 API를 열거하지 않는다: [카카오톡 메시지 API](https://developers.kakao.com/docs/ko/kakaotalk-message/common).
- 다른 앱 데이터 접근 권한은 [Apple의 macOS 앱 컨테이너 보호 문서](https://developer.apple.com/documentation/xcode/protecting-local-app-data-using-containers)를 따른다.
