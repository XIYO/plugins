# Briefing

## Collection

Collect independent lanes in parallel when the host permits it.

1. Calendar: query only the requested day or range with `calctl`.
2. Reminders: collect due today, overdue, and flagged items with RemCTL.
3. Messages: only when requested or enabled, inspect `msgpipe status` first and analyze pending threads only.

Do not redirect errors to `/dev/null`. Capture a concise diagnostic for the affected lane and continue with the others.

## Presentation

```text
Sherpa briefing — <date or range>

Important now
- conflicts, overdue actions, or commitments due soon

Calendar
- time · title

Reminders
- due/priority · title

New-message actions
- source/thread alias · candidate action

Unavailable
- capability · actionable diagnostic
```

Omit an empty section unless the absence itself matters. Do not expose full message bodies, contact identities, Calendar notes, or Reminders notes in the combined briefing unless the user explicitly asks for them.

## Mutation boundary

A briefing is read-only by default. Offer or perform a follow-up write only when the user's request includes registration, completion, movement, or editing and the target is sufficiently clear.
