---
name: apple-reminders
description: Manage Apple Reminders on macOS with RemCTL for lists, reminders, recurrence, tags, sections, subtasks, and groups. Use for adding, reading, organizing, moving, completing, or auditing reminders and for deciding whether a request belongs in Reminders instead of Calendar.
---

# Apple Reminders

Use RemCTL as the primary adapter. It reads the local iCloud Reminders store and performs writes through EventKit or its explicit ReminderKit adapter. Never write to the Reminders SQLite database directly.

## Setup

```bash
SHERPA_BIN="${SHERPA_INSTALL_ROOT:-$HOME/.local}/bin"
REMCTL="$SHERPA_BIN/remctl"
test -x "$REMCTL" || REMCTL="$(command -v remctl || true)"
```

Resolve the enclosing Sherpa plugin root from this `SKILL.md` path and run `scripts/doctor.sh reminders`. If the managed runtime is missing or has the wrong version, explain the setup action and run:

```bash
bash <plugin-root>/scripts/install-runtime.sh reminders
```

The installer fetches verified RemCTL 1.5.1 source, checks the pinned commit, and installs only the required components under `~/.local/bin` by default. It does not install the upstream `rctl` or `reminders` aliases. Then run `$REMCTL onboard`, grant the requested macOS permissions, and verify through the bundled `scripts/doctor.sh reminders`, which suppresses private diagnostic paths and list details.

Read `references/remctl.md` before organization, bulk movement, or deletion. Use `references/capability-matrix.md` only when deciding whether RemCTL, EventKit, or AppleScript can represent a requested feature.

## Default workflow

1. Discover accounts, lists, groups, sections, and counts live. Do not assume names.
2. Narrow to the target list and include completed items only when the request requires history or a full audit.
3. Resolve an item by stable ID, not title alone, before editing, moving, completing, or deleting.
4. Preview destructive targets and bulk-move counts.
5. Execute the smallest mutation and read the result back.
6. Report the target list, section, due date, recurrence, and returned ID without exposing private notes.

## Routing

- A task, deadline, checklist, follow-up, or someday item belongs in Reminders.
- A time-bound appointment or event belongs in Calendar.
- Do not duplicate a due reminder as a Calendar event merely to make it visible; Scheduled Reminders may already surface it.

## Safety rules

- Tags, sections, subtasks, and groups use explicit private-adapter commands and may require revalidation after a macOS update.
- Cross-list moves may use clone-delete and return a new ID. Preserve and use the returned ID for subsequent edits.
- A parent with subtasks may require a separate move and section assignment.
- Bulk work requires pre/post counts and duplicate checks. Never treat clone-delete trash entries as items that need restoration.
- Deletion requires the user's confirmation after an exact preview.
- Non-iCloud Reminders accounts may require an alternate AppleScript/EventKit path; report the reduced capability instead of silently changing tools.
