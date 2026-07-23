# Sherpa

Sherpa is one local-first macOS assistant with two clear responsibilities:

- Context collects and reviews the owner's requested KakaoTalk, iMessage, and mail context.
- Planner records confirmed commitments as Events or Tasks through Apple Calendar and Reminders.

The central flow is `ContextItem -> PlanningCandidate -> Event | Task`. Nothing found in a conversation or email becomes a plan without review.

Context is Experimental because local message readers and mail connections vary by host. Planner is Preview. Sherpa requires macOS 14 or newer; the complete stack is tested on macOS 26.x.

## Install

Codex:

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add sherpa@xiyo
```

Claude Code:

```bash
claude plugin marketplace add XIYO/plugins
claude plugin install sherpa@xiyo
```

Start a new task or session after installation.

## Product model

| Domain | Internal skill | Result | Boundary |
| --- | --- | --- | --- |
| Context | `context` | Collected context, summaries, PlanningCandidate values, confirmed replies | Sources read-only; outbound text requires approval |
| Planner | `planner` | Events and Tasks | Platform writes are previewed and read back |
| Orchestration | `sherpa` | Briefings and confirmed Context-to-Planner transitions | No automatic cross-domain mutation |

Applications are adapters rather than product domains. KakaoTalk and iMessage are conversation sources; mail providers supply mail context. Apple Calendar stores Events and Apple Reminders stores Tasks.

## First use

Begin with a read-only diagnosis:

```text
셰르파, Context와 Planner 준비 상태만 확인해줘. 내 데이터는 변경하거나 출력하지 마.
```

When a runtime is missing, the selected skill runs the bundled installer. Managed binaries use `~/.local/bin` by default; override the root with `SHERPA_INSTALL_ROOT`.

Manual setup:

```bash
bash scripts/install-runtime.sh context
bash scripts/install-runtime.sh planner
bash scripts/doctor.sh all
```

The only public runtime interface is:

```text
sherpa context ...
sherpa planner ...
```

Context sources `kakaocli` and `imsg` remain optional external tools and are not installed automatically. Mail collection uses a mail app connected to the host. KakaoTalk replies require a `kakaocli` build with `send` support and macOS Accessibility permission.

Planner stages a pinned Reminders implementation, exposes it only as `sherpa-reminders-adapter`, and does not install upstream command aliases.

## Examples

```text
오늘 온 카카오톡·아이메시지·메일에서 내가 대응할 것만 검토해줘.
이 대화에서 약속 후보를 뽑고, 아직 캘린더에는 넣지 마.
확인한 약속은 Event로, 해야 할 일은 Task로 등록해줘.
이 카카오톡 답장을 준비하고 보내기 전에 보여줘.
```

## Personal configuration

Sherpa discovers live sources and destinations instead of assuming the author's taxonomy. Optional preferences can live at `~/.config/xiyo/sherpa/config.toml`; never commit that file.

```toml
timezone = "Asia/Seoul"
basecamp_calendar = "Basecamp"
basecamp_reminders_list = "Basecamp"
context_sources = ["imessage", "mail"]
```

## Data and permission boundaries

- KakaoTalk and iMessage source databases remain read-only.
- Selected conversation context is stored in a separate owner-only local SQLite archive.
- Mail is read from a connected provider only within the requested search scope.
- Context-derived commitments remain PlanningCandidate values until the owner confirms them.
- Planner writes through Apple adapters and reads the result back.
- KakaoTalk reply approvals bind one exact conversation, the message digest, and a short expiry. Approval files contain no message body and are single-use.
- KakaoTalk UI automation may foreground the app and affect read state. Dispatch success does not prove delivery or reading.
- iMessage sending, email sending, attachments, reactions, and batch sends are unsupported.
- Model providers configured in the host may process content intentionally returned to the agent.
- Logs and bug reports exclude context bodies, Event and Task notes, names, contact details, credentials, source identifiers, and local database paths.

## Update and removal

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

Before removal, inspect and optionally purge the Context archive:

```bash
~/.local/bin/sherpa context state-path
~/.local/bin/sherpa context purge --force
codex plugin remove sherpa@xiyo
```

`purge --force` removes only the selected Context database and its SQLite sidecars after path confirmation. Removing the plugin does not delete managed runtimes, Context state, approval state, or configuration.

## Development

```bash
bash scripts/check.sh
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the Clean Architecture boundaries and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for third-party dependencies.
