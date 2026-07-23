---
id: DESIGN-PLAN
title: Planner Design
status: draft
owner: maintainer
---
# Planner Design

## Model

Planner classifies a commitment before choosing a platform adapter.

```text
PlanningCandidate
    -> Event -> Apple Calendar adapter
    -> Task  -> Apple Reminders adapter
```

## Application boundary

The unified CLI exposes:

```text
sherpa planner calendar ...
sherpa planner reminders ...
sherpa planner metadata ...
```

The names after `planner` identify adapter capabilities, not separate product domains. Skills select Event or Task first, then call the corresponding adapter.

## Adapters

- EventKit adapter: Calendar permissions and Event/calendar CRUD
- metadata engine: pure Event-note parsing, rendering, and validation
- RemCTL adapter: Reminders reads, standard writes, and explicit organization features

Adapter output is validated and read back after mutation. Platform-specific IDs stay at the adapter boundary.

## Failure handling

- Permission failures remain distinguishable from missing objects and invalid input.
- An adapter non-zero exit preserves the boundary and exit status without logging private stderr bodies.
- A successful mutation with failed read-back is reported as `changed, verification failed`, not as fully verified.

## Related documents

**Requirements** — [Planner Requirements](../../requirements/planner/README.md)

**Parent** — [ARCHITECTURE](../../../ARCHITECTURE.md)
