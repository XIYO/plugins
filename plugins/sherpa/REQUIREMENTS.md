---
status: draft
owner: maintainer
---
# Software Requirements Specification

## Product goal

Sherpa collects the owner's requested communication context, identifies possible commitments, and records only confirmed commitments as Events or Tasks.

## Domains

- [DOM-CTX Context Requirements](docs/requirements/context/README.md)
- [DOM-PLAN Planner Requirements](docs/requirements/planner/README.md)

## Cross-domain rules

- `BR-SHERPA-001`: Applications and external tools are sources or adapters, not top-level domains.
- `BR-SHERPA-002`: Context-derived commitments remain `PlanningCandidate` values until the owner confirms kind, title, destination, date, and recurrence.
- `BR-SHERPA-003`: Planner writes must be read back from the target adapter.
- `BR-SHERPA-004`: Public runtime commands begin with `sherpa context` or `sherpa planner`.
- `BR-SHERPA-005`: A failed source stays explicitly unavailable; it is not reported as an empty result.

## State definitions

- `draft`: implementation and verification are in progress
- `review`: acceptance criteria are being compared with the implementation
- `approved`: automated tests satisfy the acceptance criteria
- `superseded`: replaced by a newer document
- `deprecated`: scheduled for removal
- `archived`: retained only for history

## Traceability

Requirements and tests use `FR/BR/NFR/UC/AC/TEST` identifiers. Design documents refer back to requirements instead of duplicating implementation paths.

## Related documents

**Parent** — [README](README.md) · [ARCHITECTURE](ARCHITECTURE.md) · [TESTING](TESTING.md)
