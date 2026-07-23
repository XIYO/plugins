# Changelog

All notable changes to Message Pipeline will be documented in this file. Releases follow Semantic Versioning.

## Unreleased

- Marked the standalone package as a compatibility path; new installations use `sherpa@xiyo`.

## 0.2.1 - 2026-07-23

- Refuse custom state paths whose existing parent directory is not owner-only instead of changing that directory's permissions.
- Added an explicit, confirmation-gated `purge` command for the raw archive and SQLite sidecars.
- Replaced host-specific analysis provenance defaults with `host-selected`.

## 0.2.0 - 2026-07-23

- Prepared the `0.2.0` preview with a protected local raw archive and idempotent source synchronization.
- Added pending-only CCT export, session analysis commits, and recoverable thread/global summary rollups.
- Added source-specific read-only diagnostics for KakaoTalk and iMessage.
