# ADR-0015: Semantic Equivalence Policy Injection for Diff Comparison

- Status: Accepted
- Date: 2026-02-22

## Context

Expression and custom type comparison is a primary source of false diffs (`'0'::integer` vs `0`, equivalent `Custom` type spellings, etc.). Normalization remains the main tool for idempotency, but normalization alone cannot cover every dialect-specific equivalent form without increasing complexity and maintenance cost.

At the same time, the core diff engine must remain dialect-agnostic (ADR-0001). Adding direct dialect branches in diff comparison would violate this boundary.

## Decision

Introduce an injectable semantic equivalence policy for comparison:

- Define a core `EquivalencePolicy` interface with:
  - `is_equivalent_expr(&Expr, &Expr) -> bool`
  - `is_equivalent_custom_type(&str, &str) -> bool`
- Default implementation is strict equality (`==`) for both methods.
- `DiffConfig` carries `Arc<dyn EquivalencePolicy>`.
- The orchestrator obtains the policy from the selected dialect and injects it into `DiffConfig`.
- Diff comparison order is:
  1. Compare normalized values with strict equality.
  2. If unequal, evaluate the policy hook.
  3. Treat as changed only if both checks fail.

Policy constraints:
- Pure and deterministic (no DB access, no I/O, no time-dependent behavior).
- Symmetric and stable across runs.
- Narrowly scoped; broad catch-all equivalence rules are not allowed.

## Consequences

Positive:
- Reduces false diffs that survive normalization.
- Preserves core dialect-agnostic architecture by injecting a small comparison interface.
- Allows incremental dialect-specific equivalence improvements without redesigning the IR.

Negative:
- Adds a new extension point that requires discipline and regression tests.
- Incorrectly broad policy rules can hide real schema drift.
- Requires explicit wiring from orchestrator to diff config.

## Notes

- This ADR complements ADR-0004 (expression representation + normalization).
- Normalization remains the primary mechanism; policy hooks are fallback only.
