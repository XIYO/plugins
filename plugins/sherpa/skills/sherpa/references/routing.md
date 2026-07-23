# Routing

Choose a destination by the nature of the object, not by the application the user currently has open.

| User intent | Destination |
| --- | --- |
| Appointment, event, billing date, or time block | Apple Calendar |
| Action, deadline, checklist, follow-up, or someday item | Apple Reminders |
| Review a conversation or find new commitments | Message Pipeline, read-only |
| Draft or send a KakaoTalk text reply | Message Pipeline, preview then explicit confirmation |
| Extract an action from a message | Message Pipeline first, then Calendar or Reminders after review |
| Clearly an event but destination calendar is ambiguous | User's configured basecamp calendar, if it exists |
| Clearly a task but destination list is ambiguous | User's configured basecamp Reminders list, if it exists |
| Event versus task is genuinely ambiguous | Present the distinction and ask before writing |

## Cross-source requests

- “What do I have today?” reads a narrow Calendar range and due/overdue Reminders. Messages are included only when the user asks for new-message action items or enables that lane in local preferences.
- “Turn this conversation into actions” first produces untrusted candidates. Calendar and Reminders writes happen only after the user confirms the proposed title, destination, date, and recurrence.
- “Reply to this KakaoTalk message” resolves one exact chat, prepares the final text, and stops at a bound preview. Dispatch happens only after the user confirms that preview. Never route iMessage replies into this lane.
- A due Reminders item may already appear in Calendar through Scheduled Reminders. Do not create a duplicate Calendar event solely for visibility.

## Naming and classification

Use live Calendar and Reminders structures. Do not create a new taxonomy during an unrelated capture request. When a new list, calendar, group, or section is necessary, explain the durable classification it represents before creating it.
