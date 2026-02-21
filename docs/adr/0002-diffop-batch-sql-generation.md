# ADR-0002: DiffOp-Based Planning and Batch SQL Generation

- Status: Accepted
- Date: 2026-02-21

## Context

In the current implementation, diffing and SQL string generation are tightly coupled.
This makes testing difficult and prevents dialects from performing global rewrites that require visibility across multiple changes.

## Decision

Split migration planning into two stages:

1. Core diff engine outputs structured `Vec<DiffOp>`.
2. Dialect receives the full operation batch and returns `Vec<Statement>`.

`Statement` carries execution metadata:
- `Sql { transactional: bool }`
- `BatchBoundary`

## Consequences

Positive:
- Clear separation between "what changed" and "how SQL is rendered".
- Core diff testing does not require SQL string assertions.
- Dialects can merge and rewrite operations (for example MySQL column-change merge, SQLite table recreation).

Negative:
- Dialect generators are responsible for batch-level optimization logic.
- Output SQL may differ significantly from legacy expectations, increasing test migration effort.

## Notes

`BatchBoundary` transaction semantics are defined by ADR-0007.
