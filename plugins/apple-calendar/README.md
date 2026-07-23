# Apple Calendar

A macOS plugin that lets Codex or Claude Code manage Apple Calendar through EventKit and keep structured event notes consistent with a versioned schema.

> **Stage:** Preview · **Platform:** macOS · **Access:** Calendar read and write

It does not drive the Calendar UI. The agent follows the bundled skill, `calctl` performs authorized EventKit operations, and `calmeta` validates metadata without touching Calendar state.

## Install

Add the [XIYO plugin marketplace](../../README.md) and install `apple-calendar@xiyo`:

```bash
codex plugin marketplace add XIYO/plugins
codex plugin add apple-calendar@xiyo
```

Start a new Codex task after installation. On first use, the skill can build its bundled runtimes when `calctl` or `calmeta` is missing.

## Prerequisites

- macOS
- Rust toolchain with `cargo`
- Xcode Command Line Tools with `swiftc` and `codesign`
- `~/.local/bin` on `PATH`

The first authorized operation opens the standard macOS Calendar permission flow. Full access can be reviewed later in **System Settings → Privacy & Security → Calendars**.

## First use

Begin with a read-only request:

```text
Check my Apple Calendar access and list the iCloud calendars. Do not change anything.
```

Then ask for a narrowly scoped change:

```text
Create one all-day event in my iCloud Basecamp calendar for tomorrow. Show me the target and date before writing it.
```

The skill inspects the destination and possible duplicates before a write, verifies the result afterward, and requires an explicit span for recurring-event edits.

## Verify the runtime

```bash
calctl doctor
calmeta spec
```

To install manually from a repository checkout:

```bash
bash scripts/install-runtime.sh
calctl doctor
calmeta spec
```

`calctl doctor` reports the authorization state and available calendars without printing event notes. `calmeta spec` reports the supported metadata contracts.

## Runtime boundary

| Layer | Responsibility |
| --- | --- |
| Plugin skill | Classification, naming, safety checks, and operation order |
| `calctl` | Calendar authorization and EventKit-backed calendar/event operations |
| `calmeta` | Pure Rust parsing, validation, and canonical rendering of event-note metadata |

The two current metadata contracts are `xiyo.calendar.telecom-billing@1` and `xiyo.calendar.card-payment@1`. Calendar data does not contain patch versions; schema compatibility uses `MAJOR.MINOR`, while application releases use full SemVer.

## Data and safety

- The runtime accesses Calendar only through Apple's EventKit framework.
- No Calendar data is uploaded by the runtime itself. Content returned to the agent may be processed by the model provider configured in Codex or Claude Code.
- Calendar and recurring-event deletion requires inspection of the exact target and explicit confirmation.
- Calendar names may be duplicated across accounts, so write operations should identify the source, normally iCloud.
- Logs must not contain event notes, email addresses, phone numbers, or card/account identifiers.

## Limitations

- macOS only; there is no iOS, web, or CalDAV runtime.
- Runtimes are currently built locally rather than downloaded as signed release binaries.
- EventKit permission is granted to the installed `calctl` executable and may need to be restored after replacing or re-signing it.
- Structured metadata currently covers telecom billing and card-payment events. Ordinary events can still be managed without a metadata schema.

## Development

```bash
bash scripts/check.sh
```

The check formats, lints, and tests the Rust parser, type-checks the Swift EventKit adapter, and verifies release-version consistency. Build output is kept outside the plugin directory so marketplace packages remain clean.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the component boundary and [CHANGELOG.md](CHANGELOG.md) for release notes.
