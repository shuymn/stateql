# YAML Migration Status (2026-02-21)

## Idempotency Migration (`task-43b`)

| Dialect | Ported | Skipped | Coverage |
| --- | ---: | ---: | ---: |
| postgres | 25 | 5 | 83.3% |
| sqlite | 25 | 5 | 83.3% |
| mysql | 25 | 5 | 83.3% |
| mssql | 25 | 5 | 83.3% |

## Assertion Migration (`task-43c`)

| Dialect | Tables | Indexes | Constraints | Views |
| --- | --- | --- | --- | --- |
| postgres | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) |
| sqlite | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) |
| mysql | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) |
| mssql | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) | 5 / 1 (83.3%) |

## Remaining Idempotency Backlog

- `postgres`: view/constraint/dependency-order/enable_drop assertions remain in `task-43c`; `legacy_ignore_quotes` rewrites remain in `task-43d`.
- `sqlite`: trigger/dependency-order/enable_drop/rename assertions remain in `task-43c`; `legacy_ignore_quotes` rewrites remain in `task-43d`.
- `mysql`: views+triggers/constraints/partition/enable_drop assertions remain in `task-43c`; `legacy_ignore_quotes` rewrites remain in `task-43d`.
- `mssql`: fk-deps/dependency-order/enable_drop/rename assertions remain in `task-43c`; `legacy_ignore_quotes` rewrites remain in `task-43d`.

## Remaining Assertion Backlog

- `postgres`: `legacy_ignore_quotes` dependent assertion rewrites remain in `task-43d`.
- `sqlite`: `legacy_ignore_quotes` dependent assertion rewrites remain in `task-43d`.
- `mysql`: `legacy_ignore_quotes` dependent assertion rewrites remain in `task-43d`.
- `mssql`: `legacy_ignore_quotes` dependent assertion rewrites remain in `task-43d`.

## Notes

- The `tests/migration/idempotency-manifest.yml` file is the single source of truth for idempotency `ported`/`skipped(reason, tracking)` state.
- Matrix and manifest checks are automated via `yaml_idempotency_matrix_test` and `yaml_migration_manifest_test`.
- The `tests/migration/assertion-manifest.yml` file is the single source of truth for assertion `ported`/`skipped(reason, tracking)` state by feature group.
- Matrix and manifest checks are automated via `yaml_assertion_matrix_test` and `yaml_assertion_manifest_test`.
