# ADR-0011: Synchronous I/O for All Layers

- Status: Accepted
- Date: 2026-02-22

## Context

Open Question 2 in the design document asked whether database adapters should use async I/O. The `DatabaseAdapter` trait was already drafted with synchronous signatures, creating a contradiction with the open question.

Key factors:

1. **Single-connection model (ADR-0008)**: The adapter uses one dedicated connection with sequential statement execution. There is no concurrency within a single schema management operation, so async provides no throughput benefit.
2. **Driver availability**: `rusqlite` is synchronous only. `sqlx` supports both sync and async. `tokio-postgres` is async-first but can be wrapped. Choosing async would force a runtime dependency (`tokio`) even for SQLite, which needs no network I/O.
3. **Complexity cost**: Async infects all callers. Making `DatabaseAdapter` async would require the diff engine tests, CLI entry point, and test runner to pull in an async runtime. The `Transaction` RAII pattern (rollback on drop) is harder to implement correctly with async drop.
4. **Binary size and compile time**: `tokio` adds measurable overhead to both. For a CLI tool that runs once and exits, this cost is not justified by the benefits.

## Decision

All layers (core, dialect, adapter, CLI) use synchronous I/O.

- `DatabaseAdapter` trait methods are synchronous (`fn execute(&self, sql: &str) -> Result<()>`).
- Database drivers that are async-first (e.g., `tokio-postgres`) are used via their synchronous wrappers or blocking runtimes scoped to the adapter internals. The async boundary does not leak into the `DatabaseAdapter` trait or any core API.
- No async runtime dependency is required at the workspace level.

## Consequences

Positive:
- Simpler trait signatures, test harness, and error handling.
- `Transaction` RAII (rollback on drop) works naturally with synchronous `Drop`.
- No runtime overhead for SQLite (in-process, no network).
- Faster compilation and smaller binaries.

Negative:
- Schema export cannot run concurrently across multiple catalog queries within a single adapter call. This is acceptable because export is not a performance bottleneck for schema management workloads.
- If a future use case requires concurrent multi-database operations (e.g., diffing two live databases simultaneously), concurrency would be handled at the orchestrator level (threads), not within the adapter.
