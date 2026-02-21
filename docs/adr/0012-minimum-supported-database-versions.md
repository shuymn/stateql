# ADR-0012: Minimum Supported Database Versions for v1

- Status: Accepted
- Date: 2026-02-22

## Context

The v1 scope must define a clear support baseline per dialect.
Without explicit minimum versions, test strategy, bug triage, and feature expectations become ambiguous.

The chosen baseline should:
- Align with currently exercised versions in CI where possible.
- Avoid legacy-only branches that complicate parser/diff logic.
- Keep behavior predictable for features commonly used by the test corpus.

## Decision

Minimum supported database versions for v1 are:

- PostgreSQL: `13+`
- MySQL: `8.0+`
- SQL Server: `2019+`
- SQLite: `3.35+`

Notes:
- MySQL 5.7 compatibility is not a v1 guarantee.
- Higher versions are expected to work, but regressions are evaluated against this baseline first.

## Consequences

Positive:
- Clear support contract for users and maintainers.
- Reduced compatibility burden from legacy database behavior.
- Cleaner implementation boundaries for parser and DDL generation.

Negative:
- Users on older engines need to upgrade before adopting v1.
- Some sqldef-era edge cases tied to legacy versions may not be carried over.
