# Changelog

## Unreleased

- Reframed Sherpa around Context and Planner instead of application-specific domains.
- Added one public `sherpa` Rust CLI and moved KakaoTalk reply approval into a Clean Architecture Context service.
- Replaced the `apple-calendar`, `apple-reminders`, and `message-pipeline` skills with `planner` and `context`.
- Added connected mail as a bounded Context source and explicit `PlanningCandidate` handoff rules.
- Removed the Python KakaoTalk reply runtime.
- Added confirmation-gated KakaoTalk text replies with exact chat resolution, message-bound expiring previews, one-time dispatch tokens, and ambiguity rejection.
- Kept KakaoTalk and Messages source databases read-only and left iMessage sending unsupported.

## 0.1.0 - 2026-07-23

- Added one Sherpa installation for Apple Calendar, Apple Reminders, KakaoTalk, and iMessage workflows.
- Bundled the existing `calctl`, `calmeta`, `msgpipe`, and specialist skills under one orchestration boundary.
- Added a verified RemCTL 1.5.1 bootstrap path and kept message source readers optional.
- Preserved existing executable names and message archive paths for migration from the specialist plugins.
- Added managed runtime version and provenance checks, privacy-filtered diagnostics, and isolated end-to-end installer verification.
- Added prompt-injection boundaries and an explicit, confirmation-gated message archive purge path.
