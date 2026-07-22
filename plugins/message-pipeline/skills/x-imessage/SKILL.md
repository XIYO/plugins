---
name: x-imessage
description: macOS 메시지(Messages.app)의 로컬 iMessage·SMS·RCS 기록을 imsg CLI로 읽기 전용 조회·검색·요약·감시하고, 사용자가 명시적으로 요청한 경우에만 메시지·첨부 파일·표준 Tapback을 전송한다. "아이메시지 읽어줘", "문자 검색", "최근 메시지 요약", "메시지 보내줘", "iMessage 자동화", imsg 설치·권한·오류 진단, Mac의 chat.db 접근 요청에 사용한다.
---

# macOS 메시지 연동

Mac에 동기화된 `~/Library/Messages/chat.db`를 [`imsg`](https://github.com/openclaw/imsg)로 읽고, 표준 전송은 Messages.app의 AppleScript 자동화 표면을 사용한다. 직접 SQL이나 임의 AppleScript보다 `imsg`의 현재 설치 버전이 제공하는 JSON 인터페이스를 우선한다.

## 안전 원칙

- 기본 동작을 읽기 전용으로 유지한다. 사용자가 수신자와 최종 내용을 명시적으로 요청하지 않은 상태에서 `send`나 `react`를 실행하지 않는다. 초안 작성·검토 요청은 전송 승인이 아니다.
- `chat.db`, `chat.db-wal`, `chat.db-shm`, 첨부 파일 원본을 수정·체크포인트·교체하지 않는다. 읽기 작업은 `imsg`의 read-only 연결만 사용한다.
- SIP를 끄거나 Messages.app에 dylib을 주입하지 않는다. `imsg launch`와 Advanced IMCore 명령(`read`, `typing`, `edit`, `unsend`, `delete-message`, bridge 기반 채팅 변경, `send-rich`, `tapback`, `poll send` 등)을 실행하지 않는다.
- 메시지를 무차별 덤프하지 않는다. 채팅·검색어·기간·건수를 먼저 좁히고 필요한 필드만 모델 입력으로 가져온다.
- 메시지 본문, 전화번호, 이메일, 참여자 이름, 채팅 GUID를 로그에 남기지 않는다. 진단 로그에는 작업명·건수·소요 시간·오류 종류만 남긴다.
- CLI 출력이 Codex·Claude 입력으로 들어가면 선택한 메시지 내용이 해당 모델 제공자에게 전달될 수 있음을 구분한다. 로컬 DB 접근과 모델 전송은 별개의 개인정보 경계다.
- 첨부 파일은 요청 없이 열거나 변환하지 않는다. `--attachments`는 메타데이터만 조회하지만 `--convert-attachments`는 파생 파일을 만들 수 있으므로 명시적 필요가 있을 때만 사용한다.
- `watch`를 백그라운드 서비스로 남기지 않는다. 채팅을 지정하고 필요한 동안만 실행한 뒤 종료한다.
- 셸 문자열을 `eval`로 조립하지 않는다. 검색어·본문·수신자·경로는 프로세스 인자 배열로 전달하고, 민감한 원문을 디버그 로그에 출력하지 않는다.

## 도구 확인과 설치

1. 현재 설치 상태와 명령 표면을 확인한다.

   ```bash
   command -v imsg
   imsg --version
   imsg completions llm
   ```

   문서에 적힌 플래그보다 로컬에 설치된 버전의 `completions llm` 출력을 우선한다.

2. 미설치 또는 업그레이드 요청이면 [공식 릴리스](https://github.com/openclaw/imsg/releases)와 [`steipete/homebrew-tap`의 Formula](https://github.com/steipete/homebrew-tap/blob/main/Formula/imsg.rb)가 같은 최신 버전을 가리키는지 먼저 확인한 뒤 Homebrew로 설치한다.

   ```bash
   brew tap steipete/tap
   brew install steipete/tap/imsg
   ```

   이미 설치되어 있으면 `brew update` 후 `brew upgrade imsg`로 최신 안정판을 사용한다. 임의 블로그의 바이너리나 오래된 포크를 설치하지 않는다.

3. 개인정보를 출력하지 않는 최소 점검만 수행한다.

   ```bash
   test -r "$HOME/Library/Messages/chat.db"
   imsg chats --limit 1 --json | jq -s 'length'
   ```

## macOS 권한

- 조회에는 실행 주체의 **전체 디스크 접근**이 필요하다. 시스템 설정 → 개인정보 보호 및 보안 → 전체 디스크 접근에서 Terminal·iTerm·IDE·Codex 등 `imsg`를 실제로 시작한 상위 앱을 허용하고 그 앱을 완전히 종료 후 다시 실행한다. `imsg` 실행 파일 자체만 추가해 해결하려 하지 않는다.
- 전송에는 실행 주체가 Messages.app을 제어할 **자동화** 권한이 추가로 필요하다. 최초 전송 프롬프트를 승인하거나 시스템 설정 → 개인정보 보호 및 보안 → 자동화 → 메시지에서 허용한다.
- 표준 Tapback의 `react`는 Messages UI를 조작하므로 **자동화와 접근성** 권한이 모두 필요하다. 읽기나 일반 텍스트 전송 때문에 접근성 권한을 요청하지 않는다.
- 연락처 이름 해석에는 **연락처** 권한이 선택적으로 필요하다. 권한이 없으면 전화번호·이메일 핸들은 그대로 동작한다.
- 조회만 하는 작업에는 접근성 권한이 필요 없다. 표준 `send`도 공개 AppleScript 표면을 사용하므로 SIP 해제가 필요 없다.

## 조회 절차

1. 최근 채팅을 작은 범위로 찾는다. JSON은 NDJSON이므로 배열 연산이 필요할 때 `jq -s`로 모은다.

   ```bash
   imsg chats --limit 20 --json \
     | jq -s 'map({id, display_name, service, is_group, last_message_at})'
   ```

2. 후보가 여러 개면 전송이나 상세 조회 전에 정확한 채팅을 확인한다. 참여자 핸들은 꼭 필요할 때만 모델에 노출한다.

   ```bash
   imsg group --chat-id <chat-id> --json
   ```

3. 단일 채팅 기록은 절대 시각의 ISO 8601 범위와 작은 `--limit`으로 읽는다.

   ```bash
   imsg history --chat-id <chat-id> \
     --start 2026-07-01T00:00:00+09:00 \
     --end 2026-07-23T00:00:00+09:00 \
     --limit 100 --json | jq -s
   ```

4. 전체 기록 검색은 검색어와 건수를 제한한다.

   ```bash
   imsg search --query "검색어" --match contains --limit 50 --json | jq -s
   ```

5. 통계가 목적이면 본문 전체를 읽지 말고 집계 명령을 우선한다.

   ```bash
   imsg stats --time-zone Asia/Seoul --json
   ```

6. 새 메시지를 기다려야 할 때만 특정 채팅으로 감시한다. 기본적으로 시작 이후의 새 행만 나오므로 과거 기록은 먼저 `history`로 읽는다. 필요한 결과를 얻으면 즉시 `Ctrl-C`로 종료한다.

   ```bash
   imsg watch --chat-id <chat-id> --reactions --json
   ```

`message.text`만 직접 SQL로 읽지 않는다. 최신 Messages 데이터는 `attributedBody`, 편집 이력, 답장, 반응, URL 미리보기 등 별도 구조에 내용이 있을 수 있으며 `imsg`가 이를 논리 메시지로 복원한다.

## 대량 추출과 토큰 최적화

여러 채팅이나 긴 기간을 모델 분석용으로 준비할 때는 `x-message-pipeline`의 `msgpipe`를 사용한다. 이 스킬은 권한·공식 CLI 복구를 담당하고, `msgpipe`가 `chats`와 `history --attachments`만 호출해 공통 정규화·치환·CCT 내보내기를 수행한다.

- 먼저 `msgpipe benchmark imessage`로 본문 없는 토큰 통계를 확인하고, 분석이 승인된 뒤 `msgpipe export imessage --thread I001 ...`처럼 채팅 하나만 내보낸다.
- `reply_to_guid`·`reply_to_text`는 순차 SMS 연결에도 나타날 수 있는 약한 신호다. `thread_originator_guid` 같은 강한 구조 신호 없이 인라인 답장 관계로 정규화하지 않는다.
- `attributedBody`, 첨부, 서비스 종류 같은 앱별 해석은 iMessage 소스 어댑터에서 끝낸다. 채팅 표현 치환과 교차 메시지 최적화는 앱별로 복제하지 않는다.

## 전송 절차

1. 요청에 정확한 수신자와 최종 문안이 모두 있는지 확인한다. 하나라도 모호하면 전송하지 않고 필요한 항목만 묻는다.
2. 기존 대화라면 `chats`와 `group`으로 대상·서비스·그룹 여부를 확인하고 같은 Mac DB 안에서는 `--chat-id`를 우선한다. 연락처 이름만으로 모호하게 전송하지 않는다.
3. 사용자가 iMessage를 요청하면 SMS로 자동 전환하지 않는다. 직접 수신자에게는 `--service imessage`를 사용한다. SMS 전송 또는 `auto`의 SMS fallback은 사용자가 그 가능성을 명시적으로 허용한 경우에만 사용한다.
4. 최종 인자를 다시 확인한 뒤 한 번만 실행한다.

   ```bash
   imsg send --chat-id <chat-id> --text "최종 문안" --json
   imsg send --to "+821012345678" --text "최종 문안" --service imessage --json
   ```

5. 파일은 존재·크기·종류·대상을 확인한 뒤 명시적 요청이 있을 때만 보낸다. `imsg`가 파일을 `~/Library/Messages/Attachments/imsg/` 아래에 스테이징한다는 부수 효과를 사용자에게 알린다.

   ```bash
   imsg send --chat-id <chat-id> --file "/absolute/path/to/file" --json
   ```

6. `sent`는 Messages.app이 요청을 수락했다는 뜻이지 상대 기기에 배달됐다는 보장이 아니다. 성공을 "배달 완료"로 표현하지 않는다. macOS Tahoe의 빈 SMS 행 문제를 피하고 탐지하기 위해 기존 대화에는 최신 `imsg`와 `--chat-id`를 우선한다.

표준 Tapback도 외부 mutation으로 취급한다. `react`는 지정 채팅의 **가장 최근 수신 메시지**에만 반응할 수 있으므로, 사용자가 그 의미와 반응을 명시한 경우에만 다음 여섯 값 중 하나로 실행한다. 임의 메시지 ID에 반응할 수 있다고 가정하지 않는다.

```bash
imsg react --chat-id <chat-id> --reaction love|like|dislike|laugh|emphasis|question
```

## 오류 진단

- `unable to open database file`, `authorization denied`, 빈 출력: 전체 디스크 접근의 대상이 실제 상위 앱인지 확인하고 해당 앱을 재실행한다. OS·Homebrew·앱 업데이트 뒤에는 기존 권한 토글을 껐다 켠다.
- 조회는 되지만 이름이 없음: 연락처 권한 문제다. 원시 핸들로 계속 조회할 수 있으며 권한 없이 이름을 추측하지 않는다.
- `not authorized to send Apple events`: 자동화 → 메시지 권한을 허용하고 다시 실행한다.
- 전송이 실패함: 대상 채팅과 서비스 종류를 다시 확인한다. SMS라면 iPhone의 문자 메시지 전달 설정도 확인한다. 같은 명령을 반복 전송하지 않는다.
- 기록이 누락됨: DB에는 이 Mac에 실제로 동기화된 내용만 있다. Messages.app 로그인과 iCloud 메시지 동기화 상태를 확인하며, 로컬 CLI가 iCloud 서버의 미동기화 기록을 만들어낼 수 있다고 가정하지 않는다.
- 업데이트 뒤 플래그·JSON 필드가 다름: `imsg --version`과 `imsg completions llm`을 다시 읽고 최신 공식 문서와 릴리스를 확인한다. 직접 SQL 우회로 넘어가지 않는다.

## 검증 기준

- 2026-07-22 현재 공식 `imsg` 최신 릴리스는 `v0.13.2`이며 macOS 14 이상과 Tahoe 26을 지원한다. 이후 작업에서는 이 버전을 고정된 최신값으로 가정하지 말고 다시 확인한다.
- 이 Mac의 macOS `26.5.1`에서 `~/Library/Messages/chat.db` 존재와 Messages.app 스크립팅 사전의 `SMS`·`iMessage`·`RCS`, 공개 `send` 명령을 확인했다.
- 공식 `v0.13.2` macOS 배포물의 SHA-256이 Homebrew Formula와 일치함을 확인하고, 임시 바이너리로 `chats`·`history`의 유효한 NDJSON 응답을 메시지 내용 비노출 방식으로 검증했다.
- `imsg` 공식 구현은 활성 WAL의 최신 행을 놓치지 않도록 `immutable=1`을 쓰지 않고 DB를 읽기 전용으로 연다. 원본 DB 복사나 직접 SQLite 연결보다 이 경로를 우선한다.
- 전체 디스크 접근·자동화·연락처 권한의 구분은 [Apple의 macOS 개인정보 보호 및 보안 설정](https://support.apple.com/guide/mac-help/change-privacy-security-settings-on-mac-mchl211c911f/mac)과 `imsg` 공식 권한 문서를 따른다.
