# YAML Migration Status (2026-02-22)

## Idempotency Migration (`task-43b` + `task-43d`)

| Dialect | Ported | Skipped | Coverage |
| --- | ---: | ---: | ---: |
| postgres | 26 | 4 | 86.7% |
| sqlite | 26 | 4 | 86.7% |
| mysql | 26 | 4 | 86.7% |
| mssql | 26 | 4 | 86.7% |

## Assertion Migration (`task-43c` + `task-43d`)

| Dialect | Tables | Indexes | Constraints | Views |
| --- | --- | --- | --- | --- |
| postgres | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) |
| sqlite | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) |
| mysql | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) |
| mssql | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) | 6 / 0 (100.0%) |

## Combined Coverage (`R16` completion gate)

- Ported: `200`
- Skipped: `16`
- Coverage (`ported / (ported + skipped)`): `92.6%`
- `legacy_ignore_quotes` rewrites: complete (no unresolved manifest entries)

## Remaining Backlog

- `legacy_ignore_quotes`-dependent rewrites are fully completed in `task-43d`.
- Remaining skipped entries are explicitly tracked in migration manifests with `reason` + `tracking` metadata.

## Notes

- The `tests/migration/idempotency-manifest.yml` file is the single source of truth for idempotency `ported`/`skipped(reason, tracking)` state.
- Matrix and manifest checks are automated via `yaml_idempotency_matrix_test` and `yaml_migration_manifest_test`.
- The `tests/migration/assertion-manifest.yml` file is the single source of truth for assertion `ported`/`skipped(reason, tracking)` state by feature group.
- Matrix and manifest checks are automated via `yaml_assertion_matrix_test` and `yaml_assertion_manifest_test`.
- Quote-aware regression checks are automated via `yaml_quote_aware_regression_test`.
