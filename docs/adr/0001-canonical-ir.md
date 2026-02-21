# ADR-0001: Canonical IR Between Parsers and Diff Engine

- Status: Accepted
- Date: 2026-02-21

## Context

The existing architecture uses a generic AST and a large normalization layer to compensate for dialect differences.
A rewrite with per-dialect parsers needs a stable representation that the core diff engine can consume without dialect branches.

## Decision

Introduce a canonical schema IR as the boundary between dialect implementations and the core engine.

- Dialect parsers convert SQL into `SchemaObject` values.
- The core diff engine consumes only canonical IR values.
- Dialect generators convert `DiffOp` values back to SQL.
- `SchemaObject` and `DiffOp` are closed enums.
- Dialect-specific attributes are carried in explicit `extra` maps on existing IR types.
- `extra` map keys are defined as dialect-local constants (not ad-hoc string literals) and reused by both AST→IR conversion and DDL generation paths.

## Consequences

Positive:
- Core diff logic remains dialect-agnostic.
- Exhaustive match checking prevents silent drops caused by unhandled object kinds.
- Per-dialect parsing can evolve independently from core diff logic.
- Shared key constants reduce typo-driven drift between parser and generator implementations.

Negative:
- AST to IR conversion can lose details if mappings are incomplete.
- New object kinds require core changes and a semver-impacting update.
- Dialect crates need a small constants module and disciplined usage in conversion/generation code.

## Notes

This ADR is the base for ADR-0003 (extensibility model).
