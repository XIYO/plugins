# ADR-0005: Context and Planner are the product domains

## Status

Accepted

## Context

The initial plugin structure treated Apple applications and a message-processing mechanism as peer domains. Adding outbound KakaoTalk replies produced a separate `kakao-reply` component because the read-only `message-pipeline` name could not contain the new use case coherently.

## Decision

Use two user-goal domains:

- Context collects and interprets KakaoTalk, iMessage, and mail.
- Planner represents commitments as Events or Tasks and persists them through Calendar or Reminders adapters.

Keep `sherpa` as the only CLI. Place approval-bound replies inside Context. Link domain engines into the application process and isolate only operating-system boundaries as adapters.

## Consequences

- Skills reduce to `sherpa`, `context`, and `planner`.
- Python is removed from the KakaoTalk reply path.
- New documentation uses domain names instead of implementation names.
- Context and Planner metadata are Rust libraries, not subprocesses.
- Calendar and Reminders are the only separate platform adapters.
- Email collection can be added as a Context source without changing the Planner model.
