# Dialect Trait Stabilization

- Date: 2026-02-22
- Scope: Task 45 (`R19`)
- Source: `crates/core/src/dialect.rs`

## Purpose

Stabilize implementation-facing rules for `stateql_core::Dialect`, with explicit
fail-fast behavior (ADR-0013) and batch-based SQL generation boundaries (ADR-0002).

## Contract Summary

- `parse`: return typed errors immediately on unsupported/invalid SQL. No skip behavior.
- `normalize`: canonicalize IR objects in place before diff comparison.
- `generate_ddl`: consume full `&[DiffOp]` batch and emit `Vec<Statement>`.
- `to_sql`: return object SQL only when dialect supports it.
- `connect`: build a single-connection `DatabaseAdapter` implementation.

## Fail-Fast and Unsupported-Op Rules

- Unsupported parse/generate cases are errors, not warnings.
- `generate_ddl`/`to_sql` must return
  `GenerateError::UnsupportedDiffOp` when an operation cannot be represented.
- Do not silently drop unsupported `DiffOp` values.
- Keep typed error transport in core/dialect crates; presentation layering belongs to CLI.

## Implementation Template

- Reference implementation template:
  `crates/core/examples/dialect_template.rs`
- Trait-level rustdoc doctest in `crates/core/src/dialect.rs` validates the contract.

## ADR Alignment

- ADR-0002: `docs/adr/0002-diffop-batch-sql-generation.md`
- ADR-0013: `docs/adr/0013-fail-fast-error-handling.md`
