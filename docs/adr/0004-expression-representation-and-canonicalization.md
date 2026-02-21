# ADR-0004: Minimal Expression AST with Canonicalization for Raw Expressions

- Status: Accepted
- Date: 2026-02-21

## Context

Schema diffing must compare default and check expressions across dialects.
A full cross-dialect expression AST is expensive and fragile.
String comparison of raw expressions is not idempotent without canonicalization.

## Decision

Use a structured `Expr` model with enough variants to support the normalization patterns required for idempotent comparison, and allow `Expr::Raw(String)` only as a controlled escape hatch for patterns that cannot be practically modeled.

The variant set is derived from the Go implementation's `normalize.go` (1,412 lines), which handles 18+ expression types. The Expr enum includes: `Literal`, `Ident`, `QualifiedIdent`, `Null`, `BinaryOp`, `UnaryOp`, `Comparison` (with ANY/ALL quantifiers), `And`, `Or`, `Not`, `Is`, `Between`, `In`, `Paren`, `Tuple`, `Function`, `Cast`, `Collate`, `Case`, `ArrayConstructor`, `Exists`, and `Raw`. See DESIGN.md §4.4 for the full definition.

Normalization policy:
- Current-side raw expressions come from database exports.
- Desired-side expressions must be canonicalized by dialect normalizers before comparison.
- Dialects may use database-side rendering helpers where needed.
- Temporary escape hatch: if canonicalization is still insufficient, users can align desired SQL with the database-exported canonical expression text until normalizer coverage is expanded.
- `Raw` is reserved for expressions where structured modeling is not feasible (e.g., subqueries in CHECK constraints, complex window frame clauses). Expressions that can be represented structurally must not use `Raw`.

## Consequences

Positive:
- Practical implementation scope for v1.
- Preserves ability to represent complex expressions when structured modeling is not feasible.

Negative:
- False diffs remain possible if canonicalization coverage is incomplete.
- Requires continuous expansion of normalization rules based on regressions.
- Temporary workarounds may require users to adopt database-rendered expression forms in schema files.

## Acceptance Rule

A reproducible false diff is treated as a bug, not as permanent expected behavior.

## Related

- [ADR-0015: Semantic Equivalence Policy Injection for Diff Comparison](./0015-equivalence-policy-injection.md)
