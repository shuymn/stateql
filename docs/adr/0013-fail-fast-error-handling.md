# ADR-0013: Fail-Fast Error Handling for Parse and Execution

- Status: Accepted
- Date: 2026-02-22

## Context

Schema management is safety-critical. Best-effort parsing or partial execution increases the risk of
silent omissions that can be misinterpreted as desired deletions, leading to destructive diffs.

The architecture already emphasizes safety-by-default and closed IR handling (ADR-0001).
A consistent error policy is required across parser and executor layers.

## Decision

Use fail-fast behavior for parser and execution flows:

- Parsing stops at the first unsupported construct or syntax error.
- Unsupported statements are errors; they are never skipped.
- Execution stops at the first statement failure and returns an error immediately.
- Error messages must include enough context to locate the failing statement
  (at minimum statement index and the underlying parser/driver error).

Error implementation policy by layer:

- Core and dialect crates expose strongly-typed error categories (`ParseError`, `DiffError`,
  `GenerateError`, `ExecutionError`) and implement them with `thiserror`.
- AST-to-IR conversion failures must be wrapped at the conversion site with statement-level context
  (statement index and source SQL fragment), not deferred to outer layers.
- `anyhow` is permitted only at the CLI/application boundary for additional operator-facing context,
  and must not replace typed error returns in core traits.
- Rich diagnostic rendering crates (e.g., `miette`) are presentation-layer concerns. They may format
  typed errors for humans but must not become the core error transport format.

Out of scope for v1:
- Multi-error recovery parsing.
- "Continue on error" execution mode.

## Consequences

Positive:
- Strong safety guarantees against silent drift and accidental destructive plans.
- Simpler control flow and easier reasoning in parser/executor implementations.
- Clearer operational behavior in CI/CD.
- Consistent error context regardless of parser backend (`pg_query.rs` / `sqlparser-rs`).

Negative:
- Users may need multiple runs to fix multiple independent SQL issues.
- Lint-style "show all issues at once" workflows are not provided by the main apply/dry-run path.
- Slightly more boilerplate in parser adapters to attach statement-level conversion context.
