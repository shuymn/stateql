# Git Init Bootstrap Checklist (stateql)

This document is a practical "day-0/day-1" checklist for starting a new `stateql` repository after `git init`.

## 1. Create the baseline files first

1. Add `README.md` with:
   - Project positioning: "sqldef-inspired, not a drop-in replacement"
   - Current status (design phase / implementation phase)
   - Quick links to `DESIGN.md` and ADR index
2. Add `LICENSE` for your new project.
3. Add `THIRD_PARTY_NOTICES.md` if you copy or vendor any upstream code.
4. Add `.gitignore` for Rust (`target/`, `.DS_Store`, editor files).

## 2. Bring in design decisions before code

1. Copy or create `DESIGN.md` as the single high-level source of truth.
2. Create `docs/adr/README.md` (ADR index).
3. Add accepted ADRs before implementation starts:
   - Canonical IR boundary
   - DiffOp + batch SQL generation
   - Parser policy by dialect
   - Single connection adapter
   - Synchronous I/O
   - CLI shape / naming (`stateql`, no `*def` aliases)
   - Minimum supported database versions
   - Fail-fast error handling

If a decision may be debated later, make it an ADR now instead of burying it in prose.

## 3. Initialize Rust workspace skeleton

1. Create workspace root:
   - `Cargo.toml`
   - `rust-toolchain.toml`
2. Create crates:
   - `crates/core`
   - `crates/testkit`
   - `crates/dialect-postgres`
   - `crates/dialect-mysql`
   - `crates/dialect-sqlite`
   - `crates/dialect-mssql`
   - `crates/cli`
3. Add placeholder modules and traits:
   - `Dialect`
   - `DatabaseAdapter`
   - `SchemaObject`
   - `DiffOp`
   - `Statement`

Keep placeholders minimal, compile-clean, and aligned with ADR text.

## 4. Set quality gates immediately

1. Add format/lint/test scripts:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
2. Add CI workflow to run those commands on pull requests.
3. Enforce "tests + lint green" before merging anything.

## 5. Prepare sqldef reference workflow safely

1. If you need upstream reference code, add it as a Git submodule under `reference/sqldef/` to keep the repository history clean:
   ```bash
   git submodule add https://github.com/sqldef/sqldef.git reference/sqldef
   ```
2. Document policy in `AGENTS.md`:
   - `reference/sqldef/**` must not be edited
   - new code must be written in `crates/**`
3. Keep attribution/license files for any copied upstream files if you explicitly copy them into `crates/`.
4. When implementation is complete and the reference code is no longer needed, remove it safely to leave no trace in the core codebase history:
   ```bash
   git rm -f reference/sqldef
   rm -rf .git/modules/reference/sqldef
   ```

## 6. Define first implementation slice

Use a narrow vertical slice first:

1. PostgreSQL only
2. Table + column + index subset
3. Offline diff/dry-run only (no apply)
4. YAML test runner with a tiny starter corpus

Do not start with all dialects at once.

## 7. Day-1 exit criteria

`day-1` is complete when all of the following are true:

1. Workspace compiles.
2. CI runs fmt/lint/test.
3. ADR index exists and links to accepted decisions.
4. `stateql` naming and non-drop-in policy are documented.
5. One end-to-end smoke test passes (parse -> diff -> render).
