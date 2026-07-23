# Briefing

## Collection

Collect independent lanes in parallel when the host permits it.

1. Planner Events: query only the requested day or range with `sherpa planner calendar`.
2. Planner Tasks: query due, overdue, and explicitly requested lists with `sherpa planner reminders`; include completed items only for history.
3. Context: only when requested or enabled, inspect `sherpa context status` first and analyze pending conversations or relevant mail only.

Do not redirect errors to `/dev/null`. Capture a concise diagnostic for the affected lane and continue with the others.

## Presentation

```text
Sherpa briefing — <date or range>

Important now
- conflicts, overdue actions, or commitments due soon

Events
- time · title

Tasks
- due/priority · title

New context
- source/conversation alias · PlanningCandidate or reply needed

Unavailable
- capability · actionable diagnostic
```

Omit an empty section unless the absence itself matters. Do not expose full Context bodies, participant identities, Event notes, or Task notes unless the user explicitly asks for them.

## Mutation boundary

A briefing is read-only by default. Context-derived `PlanningCandidate` values are not commitments until the user reviews them. Perform a Planner write only when the request includes it and the Event or Task target is sufficiently clear.
