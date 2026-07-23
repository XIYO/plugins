# XIYO Plugins

[한국어](README.ko.md)

Install one local-first macOS assistant for Calendar, Reminders, KakaoTalk, and iMessage workflows.

> **Current stage:** Preview. Calendar and Reminders are preview integrations; the KakaoTalk and iMessage lane is Experimental because it depends on optional third-party readers and stores a local raw archive. Interfaces and setup may change before a stable release.

## Install Sherpa

### Codex

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add sherpa@xiyo
```

### Claude Code

```bash
claude plugin marketplace add XIYO/plugins
claude plugin install sherpa@xiyo
```

Start a new Codex task or Claude Code session after installation so all bundled skills are loaded.

## One plugin, specialist internals

| What Sherpa coordinates | Internal skill | Runtime | Access boundary |
| --- | --- | --- | --- |
| Apple Calendar | `apple-calendar` | `calctl`, `calmeta` | EventKit read/write after permission |
| Apple Reminders | `apple-reminders` | RemCTL 1.5.1 | iCloud read; EventKit/ReminderKit write |
| KakaoTalk and iMessage | `message-pipeline` | `msgpipe` | Source stores read-only; protected local archive write |
| Cross-source briefings and capture | `sherpa` | Specialists above | Bounded routing and combined presentation |

The plugin is the installation and product boundary. The internal skills remain separate so each data source keeps its own permission, validation, and destructive-action rules.

## First use

Begin with a read-only diagnosis:

```text
Sherpa, check whether Calendar, Reminders, and message analysis are ready. Do not change or print my data.
```

Sherpa installs a missing runtime only when that capability is first used. The bundled installer uses `~/.local/bin` by default and supports `SHERPA_INSTALL_ROOT` as an override.

Manual setup and diagnosis from the plugin root:

```bash
bash scripts/install-runtime.sh calendar
bash scripts/install-runtime.sh reminders
bash scripts/install-runtime.sh messages
bash scripts/doctor.sh all
```

### Prerequisites

- macOS 14 or newer. Calendar requires Rust and Xcode Command Line Tools, Reminders requires Python 3 plus the Swift and Clang tools from Xcode, and Message Pipeline requires Rust. The complete Sherpa stack is tested on macOS 26.x.
- Calendar and Reminders permissions are requested by their own adapters on first use.
- RemCTL is fetched from a pinned, verified 1.5.1 source commit by the bundled installer. Sherpa stages the upstream install and copies only the required components; it does not create the generic `rctl` or `reminders` aliases.
- `kakaocli` and `imsg` are optional external readers. They are not installed automatically.
- Add `~/.local/bin` to `PATH` for direct CLI use.

## Data boundaries

- Calendar changes go through EventKit and are read back after mutation.
- Reminders changes go through RemCTL. Sherpa never writes directly to the Reminders database.
- Message Pipeline never modifies KakaoTalk or Messages source databases and never sends messages.
- Synchronized message text is stored in an owner-only local SQLite archive until the user explicitly purges it. FileVault is recommended because the archive is not application-level encrypted.
- Only selected content returned to the agent enters the configured model context.
- Logs and bug reports must not include message bodies, calendar or reminder notes, names, contact details, credentials, source identifiers, or local database paths.

Read the complete [Sherpa guide](plugins/sherpa/README.md) before enabling private-data access.

## Update and remove

Refresh the Git marketplace and reinstall the current snapshot:

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

Before removing Sherpa, inspect and optionally purge the sensitive message archive:

```bash
~/.local/bin/msgpipe state-path
~/.local/bin/msgpipe purge --force
codex plugin remove sherpa@xiyo
```

Run `purge --force` only after confirming the printed archive path; it deletes the full msgpipe archive and analysis history. Removing the plugin does not remove managed runtime binaries, the message archive, or configuration created later by `remctl onboard`. See the [Sherpa removal notes](plugins/sherpa/README.md#update-and-removal).

## Compatibility plugins

The original `apple-calendar@xiyo` and `message-pipeline@xiyo` packages remain available temporarily so existing installations do not break. New users should install only `sherpa@xiyo`.

Do not enable Sherpa and a compatibility plugin together because their skill triggers overlap. After verifying Sherpa in a new task, remove the old installations.

## Repository layout

- `.agents/plugins/marketplace.json`: Codex marketplace catalog
- `.claude-plugin/marketplace.json`: Claude Code marketplace catalog
- `plugins/sherpa/`: primary self-contained plugin and its specialist skills
- `plugins/apple-calendar/`, `plugins/message-pipeline/`: compatibility packages during migration
- `catalog-policy.json`: primary and compatibility-package policy
- `scripts/`: catalog, version, source-sync, and repository checks

The marketplace manifest is the machine-readable catalog. CI verifies its order, membership, bundled skill set, runtime pins, documentation, and compatibility-source synchronization.

## Contributing and support

Read [CONTRIBUTING.md](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md) before opening a pull request. Use the issue forms for bugs and feature requests. Report vulnerabilities privately according to the [security policy](https://github.com/XIYO/.github/blob/main/SECURITY.md).

## License

[MIT](LICENSE) © 2026 XIYO
