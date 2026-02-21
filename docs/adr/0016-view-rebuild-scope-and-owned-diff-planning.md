# ADR-0016: Core-Managed View Rebuild Scope and Owned Diff Planning

- Status: Accepted
- Date: 2026-02-21

## Context

Two implementation risks were identified in diff planning:

1. View rebuild safety:
   - A changed base view can require dropping and recreating dependent views, even when dependent view definitions are unchanged.
   - If dependency expansion is left to dialect-specific SQL generation, behavior can diverge across dialects and unchanged dependents may be missed.

2. Ownership complexity:
   - Diff planning transforms IR through parse, normalize, diff, and generation boundaries.
   - Lifetime-heavy borrowed APIs make these transformations harder to reason about and maintain in Rust.

## Decision

1. View rebuild scope is computed in core:
   - The diff engine computes transitive dependent-view closure for changed views.
   - It emits explicit `DropView`/`CreateView` ops for the full rebuild set, including unchanged dependents.
   - Dialect generators render and optionally optimize this explicit plan, but do not discover hidden dependents via ad-hoc catalog queries.

2. Diff planning uses owned data by default:
   - IR and `DiffOp` payloads remain owned values across phase boundaries.
   - Clone operations at phase boundaries are acceptable and preferred over lifetime-heavy public APIs.
   - Performance optimizations are introduced only with profiling evidence and should preserve the owned API model.

## Consequences

### Positive
- Deterministic, dialect-consistent view rebuild behavior.
- Prevents partial rebuild plans that omit unchanged but dependent views.
- Simpler and safer Rust APIs for core planning stages.
- Lower implementation risk from lifetime complexity.

### Negative
- Core diff planning must maintain accurate view dependency closure logic.
- Explicit rebuild expansion can increase the number of emitted ops.
- Owned transforms can increase allocation/clone overhead in large schemas.

### Neutral
- Dialect generators still retain optimization freedom (`CREATE OR REPLACE VIEW`, batching), but operate on a fully explicit plan.
- Future performance tuning may add targeted sharing (`Arc`, interning) without changing planning semantics.

## Notes

- This ADR complements ADR-0002 (batch SQL generation) and ADR-0004/ADR-0015 (expression equivalence).
- Safety regression test coverage should include unchanged dependent-view rebuild behavior.
