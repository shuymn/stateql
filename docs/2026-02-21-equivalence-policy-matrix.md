# Equivalence Policy Matrix (Task 34e)

This matrix documents the explicit `equivalence_policy()` choice for non-PostgreSQL dialects.
Normalization remains the primary canonicalization layer. Policy is limited to residual `Expr::Raw`
differences that can still produce false diffs after normalization.

| Dialect | Policy choice | Normalization-owned differences | Policy-owned residual differences | Intentionally not covered by policy |
| --- | --- | --- | --- | --- |
| MySQL | Custom `MysqlEquivalencePolicy` | Identifier case folding, data type canonicalization (`BOOL`/`INTEGER` aliases, integer display width stripping), source SQL hint cleanup | Integer literal cast aliases in raw expressions (`CAST('000' AS SIGNED INTEGER)` vs `0`), redundant outer parentheses, whitespace-only expression differences | Structural mismatches (`Expr::Literal` vs `Expr::Raw`), non-integer casts, generalized SQL rewrites |
| SQLite | Custom `SqliteEquivalencePolicy` | Type-affinity canonicalization (`INT`/`TEXT` families), source SQL hint cleanup, expression trim | Integer literal cast aliases in raw expressions (`CAST('000' AS INT)` vs `0`), redundant outer parentheses, whitespace-only expression differences | Structural mismatches (`Expr::Literal` vs `Expr::Raw`), non-integer casts, affinity/DDL rewrites that must stay in normalize |
| MSSQL | Custom `MssqlEquivalencePolicy` | Identifier/schema case canonicalization, data type canonicalization (`INT`/`INTEGER` families), source SQL hint cleanup | Integer literal cast aliases in raw expressions (`CAST('000' AS INT)` vs `0`), redundant outer parentheses, whitespace-only expression differences | Structural mismatches (`Expr::Literal` vs `Expr::Raw`), non-integer casts, batch/DDL semantics |

## Shared Contract Rules

- All three policies are `Expr`-only and keep `is_equivalent_custom_type` strict (`left == right`).
- Each policy must satisfy core symmetry/stability checks via `verify_equivalence_policy_contract`.
- Policy is a fallback only after normalize; if a difference can be represented and canonicalized in normalize, normalize owns it.
