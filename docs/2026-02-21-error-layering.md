# Error Layering Boundaries

- Date: 2026-02-22
- Scope: Task 45b (`R01`)
- Source: `DESIGN.md` §3.10

## Purpose

Fix layer boundaries so `core`/dialects keep typed errors and the CLI owns
operator-facing context and presentation.

## Layer Contract

- `crates/core`:
  - Uses typed stage errors (`ParseError`, `DiffError`, `GenerateError`, `ExecutionError`)
    and top-level `Error`.
  - Uses `thiserror` derives for error modeling.
  - Must not depend on or expose `anyhow`/`miette`.
- `crates/cli`:
  - Converts runtime failures to presentation output only at the CLI boundary.
  - Adds operator context with `anyhow::Context`.
  - Uses `miette::Report` for human-facing rendering.
  - Preserves typed category labels (`parse`, `diff`, `generate`, `execute`) when
    rendering core errors.

## Enforcement

- `crates/core/tests/error_layering_boundary_test.rs`
  enforces `thiserror` usage and rejects `anyhow`/`miette` in core boundary files.
- `crates/cli/tests/error_presentation_test.rs`
  verifies CLI runtime parse failures keep typed category + context.
