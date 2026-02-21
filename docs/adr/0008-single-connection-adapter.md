# ADR-0008: DatabaseAdapter Uses a Single Dedicated Connection

- Status: Accepted
- Date: 2026-02-21

## Context

Schema changes are order-sensitive and often interact with transaction semantics.
If `begin()` and `execute()` occur on different physical connections, transactional guarantees break silently.

## Decision

`DatabaseAdapter` represents one dedicated connection for the full operation lifecycle.

- `export_schema`, `execute`, and `begin` operate on the same connection.
- Transaction handles execute on that same connection.
- No connection pooling is part of adapter responsibilities.

## Consequences

Positive:
- Preserves transaction integrity and ordering guarantees.
- Simplifies execution reasoning for schema operations.

Negative:
- No parallel statement execution through the adapter.
- Long-running operations monopolize one connection.

## Notes

This design matches schema-management workloads where correctness dominates throughput.
