# Routing

Choose a destination by the nature of the object, not by the application the user currently has open.

| User intent | Destination |
| --- | --- |
| Collect or review KakaoTalk, iMessage, or mail | Context |
| Search a conversation or find new commitments | Context |
| Draft or send a KakaoTalk text reply | Context, preview then explicit confirmation |
| Appointment, event, billing date, or time block | Planner as an Event |
| Action, deadline, checklist, follow-up, or someday item | Planner as a Task |
| Extract a plan from a message or mail | Context candidate first, then Planner after review |
| Clearly an event but destination calendar is ambiguous | User's configured basecamp calendar, if it exists |
| Clearly a task but destination list is ambiguous | User's configured basecamp Reminders list, if it exists |
| Event versus task is genuinely ambiguous | Present the distinction and ask before writing |

## Cross-source requests

- “What do I have today?” reads a narrow Event range and due/overdue Tasks. Context is included only when the user asks for new commitments or enables that lane in local preferences.
- “Turn this conversation into actions” first produces untrusted `PlanningCandidate` values. Planner writes happen only after the user confirms the proposed kind, title, destination, date, and recurrence.
- “Reply to this KakaoTalk message” resolves one exact chat, prepares the final text, and stops at a bound preview. Dispatch happens only after the user confirms that preview. Never route iMessage replies into this lane.
- A due Task may already appear in Calendar through Scheduled Reminders. Do not create a duplicate Event solely for visibility.

## Naming and classification

Use live Planner structures. Do not create a new taxonomy during an unrelated capture request. When a new list, calendar, group, or section is necessary, explain the durable classification it represents before creating it.
