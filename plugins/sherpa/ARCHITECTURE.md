# Architecture

## Product model

Consumers install one product and use one command: `sherpa`.

```text
Sherpa
├── Context
│   ├── collect and normalize
│   ├── search and incremental review
│   ├── derive PlanningCandidate values
│   └── prepare and confirm a reply
├── Planner
│   ├── Event
│   └── Task
└── Orchestration
    ├── ContextItem -> PlanningCandidate
    ├── PlanningCandidate -> Event | Task
    └── Context + Planner -> Briefing
```

Applications are not domains. KakaoTalk, iMessage, and mail providers are Context sources. Apple Calendar and Apple Reminders are Planner adapters.

## Clean Architecture boundaries

Dependencies point inward.

```text
CLI and skills
    -> application services
        -> domain model and ports
            <- source, storage, and platform adapters
```

- Domain code owns entities, invariants, and approval rules. It does not execute platform commands.
- Application services coordinate use cases through traits.
- Adapters implement traits for local storage, KakaoTalk, EventKit, RemCTL, and compatibility engines.
- Skills own routing and user-confirmation policy. They do not reimplement parsers or domain validation.

The Rust reply flow demonstrates the boundary directly:

```text
ReplyService
├── ConversationGateway
├── ApprovalRepository
├── Clock
└── TokenGenerator
```

The service knows no file paths, subprocesses, or KakaoTalk JSON shape. Those details live under `adapters`.

## Public interface

Only goal-oriented commands are public:

```text
sherpa context ...
sherpa planner ...
```

The previous `msgpipe`, `calmeta`, `calctl`, and `remctl` executables remain internal compatibility or platform adapters during the migration window. New skills and user documentation do not instruct consumers to call them directly.

## Context

Context owns collected information and analysis state.

- `ConversationMessage`: a KakaoTalk or iMessage item
- `MailMessage`: an email item independent of provider
- `PlanningCandidate`: an unconfirmed possible Event or Task
- `ReplyDraft`: a target- and message-bound outbound draft

Source databases remain read-only. Context writes only to its separate owner-only state and, after explicit confirmation, to a channel's outbound adapter.

## Planner

Planner owns commitments.

- `Event`: an appointment, billing date, time block, or occurrence
- `Task`: an action, deadline, checklist, follow-up, or someday item

Apple Calendar stores Events. Apple Reminders stores Tasks. Platform object IDs never become the domain classification.

## State and compatibility

New Context state uses Sherpa's application directory. If no canonical state exists but the previous context-engine database does, the runtime opens that database in place so existing analysis history is preserved. New reply approvals use `sherpa/context/replies`, store no message body, expire quickly, and are single-use.

Compatibility plugin sources remain frozen until the review date in `catalog-policy.json`. New domain behavior is implemented only in Sherpa.

## Versioning

The Sherpa plugin and unified Rust CLI use independent SemVer declarations that currently share the same base version. Platform and compatibility adapters retain their own pinned versions in `runtime-versions.json`. Data contracts use their own versions.
