# XIYO Plugins

XIYO가 공개 배포하는 Codex·Claude Code 플러그인 모음이다. 각 플러그인은 설치에 필요한 스킬, 실행 코드, 계약과 설치 스크립트를 자기완결형으로 포함한다.

## 카탈로그

| 플러그인 | 설치되는 스킬 | 지원 소스 | 설명 |
|---|---|---|---|
| `message-pipeline` | `message-pipeline` 하나 | KakaoTalk, iMessage | 원문을 보호된 로컬 DB에 동기화하고 미분석 메시지만 CCT로 변환한다. |

KakaoTalk와 iMessage는 별도 스킬이 아니라 `message-pipeline` 내부 소스 어댑터다. 플러그인 하나를 설치하면 두 소스를 모두 사용할 수 있다.

## 구조

- `plugins/message-pipeline/`: 자기완결형 플러그인, Rust 앱, 설치 스크립트
- `.agents/plugins/marketplace.json`: Codex marketplace
- `.claude-plugin/marketplace.json`: Claude Code marketplace

## 로컬 설치

Codex:

```bash
codex plugin marketplace add "$PWD"
codex plugin add message-pipeline@xiyo
```

Claude Code:

```bash
claude plugin marketplace add "$PWD"
claude plugin install message-pipeline@xiyo
```

플러그인 설치는 단일 `message-pipeline` 스킬을 등록한다. `msgpipe` 실행 파일이 없으면 스킬이 번들된 `scripts/install-runtime.sh`를 사용해 Rust 앱을 별도로 설치한다. 사용자 데이터에 접근하는 `kakaocli`와 `imsg`는 자동 설치하지 않으며, 같은 스킬의 소스별 참조 문서가 진단·검증 절차를 제공한다. `msgpipe sync`가 만드는 원문 아카이브는 저장소가 아니라 사용자 로컬 앱 데이터 디렉터리에만 존재한다.

## 개발

```bash
cd plugins/message-pipeline
bash scripts/check.sh
```

검증 스크립트는 임시 Rust 빌드 디렉터리를 사용하므로 marketplace 설치본에 `target/`이 섞이지 않는다.

앱 구조와 모델 입력 규격은 [플러그인 README](plugins/message-pipeline/README.md), [CCT 계약](plugins/message-pipeline/contracts/cct/CCT.md), [CTX 계약](plugins/message-pipeline/contracts/context/CTX.md)을 따른다.
