# ADR-0014: Schema Search Path for Name Resolution

- Status: Accepted
- Date: 2026-02-22

## Context

The diff engine must match unqualified object names (e.g., `users`) against qualified names (e.g., `public.users`). The original design used `default_schema: Option<String>`, which handles the common single-schema case but breaks when the database connection uses a multi-schema search path.

PostgreSQL's `search_path` can contain multiple schemas (e.g., `search_path = 'app,public'`). In this configuration, an unqualified reference to `users` resolves to `app.users` if it exists there, otherwise `public.users`. A single `default_schema` cannot express this priority ordering.

## Decision

Replace `DiffConfig.default_schema: Option<String>` with `DiffConfig.schema_search_path: Vec<String>`.

Name resolution rules:
1. If both names are qualified, compare schema and name directly.
2. If one name is unqualified and the other is qualified, check whether the qualified name's schema appears in `schema_search_path`. If it does, compare by name only.
3. If both names are unqualified, compare by name only (the schema is irrelevant for matching).
4. When an unqualified name could match objects in multiple schemas of the search path, the **first match** in search path order wins.

The `DatabaseAdapter` trait method `default_schema()` is replaced with `schema_search_path() -> Vec<String>`. Implementations query the actual connection-level search path from the database:
- PostgreSQL: `SHOW search_path` (expanded, excluding implicit schemas like `pg_catalog`).
- MySQL: single-element vec containing the connected database name.
- MSSQL: single-element vec containing the default schema (typically `dbo`).
- SQLite: empty vec (no schema concept; all objects are unqualified).

## Consequences

Positive:
- Correctly handles PostgreSQL connections with custom `search_path` settings.
- Eliminates false diffs where `app.users` (qualified from export) fails to match `users` (unqualified from desired SQL) when `app` is not the sole default schema.
- Generalizes cleanly to all dialects: single-element paths degrade to the old behavior.

Negative:
- Slightly more complex matching logic in the diff engine.
- The adapter must query the database for the search path at connection time.

## Notes

For most users the search path is a single schema, so this change has no observable effect. It becomes critical only for PostgreSQL deployments with non-default `search_path` configurations.
