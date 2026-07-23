---
id: DOM-PLAN
title: Planner Requirements
status: draft
owner: maintainer
---
# Planner Requirements

## Purpose

Represent personal commitments as Events or Tasks before selecting an Apple storage adapter.

## Business rules

- BR-PLAN-001: Appointments, occurrences, billing dates, and time blocks are Events.
- BR-PLAN-002: Actions, deadlines, checklists, follow-ups, and someday items are Tasks.
- BR-PLAN-003: An undated Task still belongs to Planner.
- BR-PLAN-004: A genuinely ambiguous Event versus Task classification requires owner review before writing.
- BR-PLAN-005: A Context item cannot write Planner state without becoming a confirmed `PlanningCandidate`.

## Functional requirements

- FR-PLAN-EVENT-001: Discover Calendar sources and calendars live.
- FR-PLAN-EVENT-002: Create, read, update, move, and delete Events through the EventKit adapter.
- FR-PLAN-EVENT-003: Require an explicit occurrence span for recurring Event edits.
- FR-PLAN-EVENT-004: Render and validate versioned structured Event metadata.
- FR-PLAN-TASK-001: Discover Reminders accounts, groups, lists, sections, and counts live.
- FR-PLAN-TASK-002: Create, read, update, move, complete, and delete Tasks by stable ID.
- FR-PLAN-TASK-003: Preserve returned IDs when a cross-list move uses clone-delete.
- FR-PLAN-VERIFY-001: Read every mutation back from its target adapter.

## Non-functional requirements

- NFR-PLAN-SEC-001: Destructive and bulk operations require an exact preview and explicit confirmation.
- NFR-PLAN-PRI-001: Logs exclude Event notes, Task notes, contact details, and payment identifiers.
- NFR-PLAN-REL-001: Adapter failures preserve the original error chain and do not fabricate success.
- NFR-PLAN-OPS-001: Public commands use the `sherpa planner` namespace.

## Related documents

**Design** — [Planner Design](../../design/planner/DESIGN.md)

**Parent** — [ARCHITECTURE](../../../ARCHITECTURE.md) · [REQUIREMENTS](../../../REQUIREMENTS.md)
