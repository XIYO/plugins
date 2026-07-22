# CLAUDE.md

이 저장소는 Codex와 Claude Code가 설치할 수 있는 `message-pipeline` marketplace다. 실제 플러그인과 Rust 앱은 `plugins/message-pipeline/` 안에 함께 둔다. 플러그인 설치기는 디렉터리를 캐시로 복사하므로 저장소 밖 파일이나 절대 경로에 의존하지 않는다.

## 구조

```text
.agents/plugins/marketplace.json             Codex marketplace
.claude-plugin/marketplace.json              Claude Code marketplace
plugins/message-pipeline/
├── .codex-plugin/plugin.json
├── .claude-plugin/plugin.json
├── skills/                                  번들 스킬
├── scripts/install-runtime.sh
└── Cargo.toml                               msgpipe Rust 앱
```

## 불변 규칙

- 소스 데이터는 읽기 전용으로 다룬다. KakaoTalk는 검증된 `kakaocli query`, iMessage는 `imsg chats/history`만 허용한다.
- 메시지 전송, 읽음 처리, 감시, UI 자동화, 첨부 변환 명령을 코드 경로에 추가하지 않는다.
- 원문 채팅은 상태 DB나 로그에 저장하지 않는다. 상태 DB에는 식별자·별칭·정확한 시각·본문 SHA-256·변환 감사와 명시적으로 입력된 파생 분석 요약만 저장한다.
- 기본 모델 출력은 스레드와 화자를 별칭으로 치환한다. 실명 매핑은 소유자 전용 로컬 SQLite에만 둔다.
- 로그에는 메시지 본문, 이름, 전화번호, 이메일, 원본 식별자, 토큰, 키, 파일 경로를 남기지 않는다.
- plugin manifest와 Cargo package의 기본 semver를 함께 갱신한다. Codex 로컬 재설치용 `+codex.<cachebuster>` suffix만 예외다.

## 로깅

- 접두사는 `[layer:module:action]` 형식을 쓴다.
- 외부 명령과 DB 경계에서 시작·성공·실패를 기록한다.
- 기본 레벨은 `warn`; `LOG_LEVEL`로 조절한다.
- 실패를 잡았다면 원본 오류를 보존해 상위로 전달한다.

## 품질 게이트

Rust 명령은 `plugins/message-pipeline/`에서 실행한다.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
python3 ../../scripts/check-version-sync.py
python3 ~/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/x-message-pipeline
python3 ~/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py .
claude plugin validate ../..
```

실데이터 회귀 검증은 집계 수치와 해시만 출력하며 채팅 본문은 터미널에도 표시하지 않는다.
