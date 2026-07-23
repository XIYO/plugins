# Sherpa

Sherpa is one installable macOS assistant for Calendar, Reminders, KakaoTalk, and iMessage workflows.

The plugin is the product boundary. Its specialist skills and runtimes remain separate internally so Calendar writes, Reminders organization, message analysis, and confirmation-gated KakaoTalk replies keep their own safety rules.

Calendar and Reminders are Preview features. The message lane is Experimental because it depends on optional third-party readers and retains a protected local raw archive. Sherpa requires macOS 14 or newer; the complete stack is tested on macOS 26.x.

## Install

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

## Capabilities

| Capability | Internal skill | Runtime | Data boundary |
| --- | --- | --- | --- |
| Calendar | `apple-calendar` | `calctl`, `calmeta` | EventKit read/write after macOS permission |
| Reminders | `apple-reminders` | RemCTL 1.5.1 | iCloud database read; EventKit/ReminderKit write |
| Messages | `message-pipeline` | `msgpipe`, `kakao-reply.py` | Sources remain read-only; KakaoTalk text dispatch requires confirmation |
| Coordination | `sherpa` | Specialist skills above | Routes requests and combines bounded results |

KakaoTalk text replies are optional and use `kakaocli` UI automation. Sherpa first resolves one exact chat and presents a message-bound preview; every dispatch requires explicit confirmation. iMessage sending, attachments, reactions, and batch sends are not supported.

## First use

Ask Sherpa to diagnose one capability without changing data:

```text
셰르파, 캘린더·미리 알림·메시지 기능 준비 상태만 확인해줘. 데이터는 변경하거나 출력하지 마.
```

When a runtime is missing, the relevant skill runs the bundled installer for that capability. Runtime binaries are installed under `~/.local/bin` by default. Override the root with `SHERPA_INSTALL_ROOT`.

Manual setup:

```bash
bash scripts/install-runtime.sh calendar
bash scripts/install-runtime.sh reminders
bash scripts/install-runtime.sh messages
bash scripts/doctor.sh all
```

RemCTL is fetched from the verified `v1.5.1` source commit during the Reminders setup. The installer builds it in an isolated staging root and copies only `remctl`, its required helpers, and a provenance marker. It does not copy the upstream `rctl` or `reminders` aliases, completions, or temporary config. `remctl onboard` may later create `~/.config/remctl`. `kakaocli` and `imsg` remain optional external tools and are never installed automatically. KakaoTalk replies additionally require a `kakaocli` build that exposes `send` plus macOS Accessibility permission for the host driving KakaoTalk.

## Personal configuration

Sherpa discovers live Calendar and Reminders names instead of assuming the author's taxonomy. Optional local preferences can live at `~/.config/xiyo/sherpa/config.toml`; never commit that file to this repository.

```toml
timezone = "Asia/Seoul"
basecamp_calendar = "Basecamp"
basecamp_reminders_list = "Basecamp"
message_sources = ["imessage"]
```

## Data and permission boundaries

- Calendar mutations use EventKit and inspect the target before writing.
- Reminders organization may use RemCTL's private ReminderKit adapter. Run the privacy-filtered `bash scripts/doctor.sh reminders` again after macOS upgrades.
- Message source databases are read-only. `msgpipe` writes a separate owner-only local SQLite archive and exports only selected pending messages for analysis.
- KakaoTalk reply previews create owner-only, short-lived confirmation records under the user's local state directory. The record stores the exact chat and message digest, not the message body, and is removed after dispatch, cancellation, or expiry cleanup.
- KakaoTalk UI automation may foreground the app and open the target chat. A successful command confirms dispatch automation, not recipient delivery or reading.
- Message content is retained until the user runs `msgpipe purge --force`. There is no implicit retention-window deletion.
- Model providers configured in the host application may process content intentionally returned to the agent.
- Logs and bug reports must not include calendar notes, reminder bodies, message text, names, contact details, credentials, source identifiers, or local database paths.

## Migration from the specialist plugins

`apple-calendar@xiyo` and `message-pipeline@xiyo` remain available for a compatibility window. Do not enable them together with Sherpa because overlapping skill triggers can produce duplicate routing.

After Sherpa works in a new task, remove the old installs:

```bash
codex plugin remove apple-calendar@xiyo
codex plugin remove message-pipeline@xiyo
```

Existing `calctl`, `calmeta`, `msgpipe`, and msgpipe data locations are preserved.

## Update and removal

For a Git-backed Codex marketplace, refresh and reinstall:

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

The plugin cache and local runtimes have separate lifecycles. Before removal, inspect the archive path. If the user wants the raw messages and derived analysis deleted, require confirmation and then purge them:

```bash
~/.local/bin/msgpipe state-path
~/.local/bin/msgpipe purge --force
codex plugin remove sherpa@xiyo
```

`purge --force` removes only the selected msgpipe database and its SQLite `-wal`, `-shm`, and rollback `-journal` sidecars from an owner-only directory. It does not remove unrelated files. `codex plugin remove` leaves these managed files in place for compatibility with existing installations:

- `~/.local/bin/calctl`, `calmeta`, and `msgpipe`
- `~/.local/bin/remctl`, `remctl-bridge`, `remctl-permissions`, `remctl-private`, and `remctl_*.py`
- `~/.local/share/sherpa/remctl.provenance`
- `~/.local/share/licenses/sherpa/remctl/LICENSE`
- `~/.config/remctl` if the user later created it through RemCTL onboarding

Review exact ownership before deleting shared runtime files. Sherpa never removes them as a side effect of removing the plugin.

## Development

```bash
bash scripts/check.sh
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for component boundaries and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the pinned RemCTL dependency.
