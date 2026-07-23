# XIYO Plugins

[한국어](README.ko.md)

Local-first plugins that give Codex and Claude Code inspectable command-line access to personal macOS workflows.

This repository is a public marketplace. Each plugin ships its agent skill, runtime source, data contract, installer, and checks together so an installation does not depend on a private skills repository or an absolute path on the author's machine.

> **Current stage:** Preview. The plugins are usable on macOS, but they still build local runtimes from source and their interfaces may change before a stable release.

## Choose a plugin

| Goal | Install ID | Runtime | Data access | Stage |
| --- | --- | --- | --- | --- |
| Manage Apple Calendar without UI automation | `apple-calendar@xiyo` | `calctl`, `calmeta` | Calendar read and write through EventKit | Preview |
| Analyze only new KakaoTalk and iMessage content | `message-pipeline@xiyo` | `msgpipe` | Source databases read-only; protected local archive write | Experimental |

`message-pipeline` is marked Experimental because its KakaoTalk and iMessage source readers still require separate setup. The shared archive, incremental state, and CCT export live in the bundled `msgpipe` application.

## Install from GitHub

### Codex

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add apple-calendar@xiyo
codex plugin add message-pipeline@xiyo
codex plugin list
```

### Claude Code

```bash
claude plugin marketplace add XIYO/plugins
claude plugin install apple-calendar@xiyo
claude plugin install message-pipeline@xiyo
```

Start a new Codex task or Claude Code session after installation so the new skills are loaded.

## Prerequisites

Both plugins currently support macOS only.

- **Apple Calendar:** Rust toolchain and Xcode Command Line Tools (`swiftc`, `codesign`). The first use asks for Calendar access through the standard macOS permission prompt.
- **Message Pipeline:** Rust toolchain plus a supported read-only source reader: `kakaocli` for KakaoTalk and `imsg` for iMessage. Full Disk Access may be required by the source reader.
- Ensure `~/.local/bin` and `~/.cargo/bin` are on `PATH` for the installed runtimes.

The bundled skill can run its plugin-local installer when a runtime is missing. Installers build in a temporary directory and do not write build artifacts into the marketplace cache.

## First use

Try one of these prompts in a new task or session:

```text
Check my Apple Calendar access and list the available iCloud calendars. Do not change anything.
```

```text
Check whether the iMessage source reader is ready. Do not print or analyze message content.
```

For a manual runtime check:

```bash
calctl doctor
calmeta spec
msgpipe doctor imessage
msgpipe doctor kakao
```

Successful installation is not the same as granted data access. The `doctor` commands report missing executables and macOS permissions without dumping calendar or message content.

## Data boundaries

- The runtimes do not upload source data by themselves.
- Content returned to an agent may be processed by the model provider configured in the host application. Request read-only diagnosis first and review the selected scope before asking for analysis or mutation.
- Apple Calendar writes require EventKit permission. The skill inspects the target before edits and requires explicit scope for recurring changes.
- Message Pipeline never modifies the KakaoTalk or Messages source database. It stores synchronized raw content in an owner-only local SQLite database; FileVault is recommended because that database is not application-level encrypted.
- Message analysis exports only pending content with thread and speaker aliases, but the selected message text still enters the configured model context.
- Logs and bug reports must not contain message bodies, calendar notes, names, contact details, source identifiers, credentials, or local database paths.

See each plugin README for its complete permission and storage model:

- [Apple Calendar](plugins/apple-calendar/README.md)
- [Message Pipeline](plugins/message-pipeline/README.md)

## Update or remove

Codex:

```bash
codex plugin marketplace upgrade xiyo
codex plugin add apple-calendar@xiyo
codex plugin remove apple-calendar@xiyo
```

Claude Code:

```bash
claude plugin marketplace update xiyo
claude plugin update apple-calendar@xiyo
claude plugin uninstall apple-calendar@xiyo
```

Replacing `apple-calendar` with `message-pipeline` applies the same flow. Removing a plugin does not automatically delete a runtime binary or local application data; review the plugin's own README before deleting either.

## Repository layout

- `.agents/plugins/marketplace.json`: Codex marketplace catalog
- `.claude-plugin/marketplace.json`: Claude Code marketplace catalog
- `plugins/<name>/`: self-contained plugin, skill, runtime, installer, contracts, and tests
- `scripts/`: repository-wide consistency checks

The marketplace manifests are the machine-readable catalog. The table above is consumer-facing documentation and is checked against those manifests.

## Contributing and support

Read [CONTRIBUTING.md](https://github.com/XIYO/.github/blob/main/CONTRIBUTING.md) before opening a pull request. Use the affected repository's issue form for bugs and feature requests. Report vulnerabilities privately according to the [security policy](https://github.com/XIYO/.github/blob/main/SECURITY.md).

## License

[MIT](LICENSE) © 2026 XIYO
