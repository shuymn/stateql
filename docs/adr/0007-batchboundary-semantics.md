# ADR-0007: BatchBoundary Represents Synchronization, Not Commit

- Status: Accepted
- Date: 2026-02-21

## Context

Some dialects (notably MSSQL tooling) use client-side batch separators (for example `GO`) that are not SQL statements.
These separators must not be conflated with transaction control.

## Decision

Model batch separators as `Statement::BatchBoundary` with these semantics:

- It is never sent to the database.
- It enforces execution synchronization between adjacent SQL statements.
- It does not commit or rollback transactions.
- Rendering may emit dialect-specific text (for example `GO`) in dry-run output.

## Consequences

Positive:
- Correctly models client-side batch behavior without leaking pseudo-SQL into execution.
- Keeps transaction logic explicit and independent from rendering markers.

Negative:
- Requires dedicated executor logic and regression tests to prevent accidental commit coupling.

## Notes

Transaction grouping behavior is driven by statement transactional flags and executor policy, not batch boundaries.
