# ADR-0010: Parser Selection Policy by Dialect

- Status: Accepted
- Date: 2026-02-21

## Context

A parser strategy must balance fidelity, implementation effort, and maintainability across dialects.
No single parser is equally accurate for all supported SQL dialects.

## Decision

Use parser selection by dialect:
- PostgreSQL: `pg_query.rs` for native parser fidelity.
- MySQL, SQLite, MSSQL: start with `sqlparser-rs` and extend or replace per dialect when required.

## Consequences

Positive:
- Highest-fidelity path for PostgreSQL.
- Fast bootstrap path for other dialects with shared parser infrastructure.

Negative:
- Mixed parser stack increases maintenance surface.
- Coverage gaps in `sqlparser-rs` require active mitigation and potential custom parsing.

## Notes

Unsupported constructs must fail fast rather than being ignored.
