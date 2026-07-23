# XIYO Plugins

[한국어](README.ko.md)

Install one local-first macOS assistant that collects personal context and turns only confirmed commitments into plans.

> **Current stage:** Preview. Planner is Preview; Context is Experimental because local message readers and mail connections vary by host.

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

Start a new task or session after installation.

## One product, two domains

| Domain | Inputs or destinations | Purpose |
| --- | --- | --- |
| Context | KakaoTalk, iMessage, connected mail | Collect, search, review, summarize, and derive PlanningCandidate values |
| Planner | Apple Calendar, Apple Reminders | Record confirmed commitments as Events or Tasks |

Sherpa orchestrates `ContextItem -> PlanningCandidate -> Event | Task`. Applications and external tools are adapters, not peer product domains.

The public runtime interface is limited to:

```text
sherpa context ...
sherpa planner ...
```

## First use

```text
Sherpa, check whether Context and Planner are ready. Do not change or print my data.
```

Manual setup from the plugin root:

```bash
bash scripts/install-runtime.sh context
bash scripts/install-runtime.sh planner
bash scripts/doctor.sh all
```

### Prerequisites

- macOS 14 or newer; the full stack is tested on macOS 26.x.
- Rust and Xcode Command Line Tools for managed runtimes.
- Calendar, Reminders, Full Disk Access, and Accessibility permissions are requested only by the adapters that need them.
- Optional `kakaocli` and `imsg` tools are not installed automatically.
- Mail collection requires a mail app connected to the host.

## Data boundaries

- Conversation source databases remain read-only.
- Selected Context is retained in an owner-only local SQLite archive until explicitly purged.
- Context findings never write Planner state without owner review.
- Planner mutations are read back from Apple Calendar or Reminders.
- KakaoTalk text dispatch requires an exact target, message-bound preview, short-lived token, and explicit confirmation.
- iMessage and email sending, attachments, reactions, and batch sends are unsupported.
- Only selected content returned to the agent enters the configured model context.

Read the complete [Sherpa guide](plugins/sherpa/README.md) before enabling private-data access.

## Update and remove

```bash
codex plugin marketplace upgrade xiyo
codex plugin add sherpa@xiyo
```

Before removal, inspect and optionally purge Context state:

```bash
~/.local/bin/sherpa context state-path
~/.local/bin/sherpa context purge --force
codex plugin remove sherpa@xiyo
```

## Repository layout

- `.agents/plugins/marketplace.json`: Codex marketplace catalog
- `.claude-plugin/marketplace.json`: Claude Code marketplace catalog
- `plugins/sherpa/`: the only self-contained product and source of truth
- `catalog-policy.json`: published-product policy
- `scripts/`: catalog, version, and repository checks

## Contributing and support

Read [CONTRIBUTING.md](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md) before opening a pull request. Report vulnerabilities privately according to the [security policy](https://github.com/XIYO/.github/blob/main/SECURITY.md).

## License

[MIT](LICENSE) © 2026 XIYO
