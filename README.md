# message-pipeline marketplace

KakaoTalk와 iMessage 로컬 기록을 읽기 전용으로 추출하고 토큰 절약형 CCT로 변환하는 `msgpipe` Rust 앱과 에이전트 스킬을 함께 배포한다.

## 구성

- `plugins/message-pipeline/`: 자기완결형 플러그인, Rust 앱, 설치 스크립트
- `.agents/plugins/marketplace.json`: Codex marketplace
- `.claude-plugin/marketplace.json`: Claude Code marketplace

플러그인에는 다음 스킬이 들어 있다.

- `x-message-pipeline`: 기간별 비용 측정, 스레드별 CCT 내보내기, 중앙 맥락 관리
- `x-kakaotalk`: macOS KakaoTalk SQLCipher 읽기 진단
- `x-imessage`: macOS Messages DB 읽기 진단

## 로컬 설치

Codex:

```bash
codex plugin marketplace add "$PWD"
codex plugin add message-pipeline@personal
```

Claude Code:

```bash
claude plugin marketplace add "$PWD"
claude plugin install message-pipeline@message-pipeline
```

플러그인 설치는 스킬을 등록한다. `msgpipe` 실행 파일이 없으면 스킬이 번들된 `scripts/install-runtime.sh`를 사용해 Rust 앱을 별도로 설치한다. `kakaocli`와 `imsg`는 사용자 데이터에 접근하는 외부 읽기 도구이므로 자동 설치하지 않고 각 스킬의 검증 절차를 따른다.

## 개발

```bash
cd plugins/message-pipeline
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
python3 ../../scripts/check-version-sync.py
```

앱 구조와 CCT 규격은 [플러그인 README](plugins/message-pipeline/README.md)와 [CCT 계약](plugins/message-pipeline/contracts/cct/CCT.md)을 따른다.
