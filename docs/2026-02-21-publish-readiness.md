# Publish Readiness (`stateql-core`, `stateql-testkit`)

This document defines the preflight checks and dry-run publish steps for `R20`.

## Scope

- Target crates:
  - `stateql-core`
  - `stateql-testkit`
- Goal: ensure `cargo publish --dry-run` passes without metadata or packaging issues.

## Required Metadata Policy

- Each publishable crate must define or inherit:
  - `license`
  - `repository`
  - `description`
  - `readme`
  - `keywords`
  - `categories`
- Internal path dependencies must specify an explicit `version` to satisfy publish checks.

## Local Dry-Run Dependency Resolution

- `stateql-testkit` depends on internal crates that are not yet published on crates.io.
- This repository includes `.cargo/config.toml` `[patch.crates-io]` entries so local `cargo publish --dry-run` can resolve internal dependencies during preflight.
- Real release order is still:
  1. Publish `stateql-core`
  2. Wait for crates.io index propagation
  3. Publish `stateql-testkit`

## Preflight Checks

Run these commands from workspace root:

```bash
cargo +nightly-2026-02-20 fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --doc
cargo doc --workspace --no-deps
```

## Dry-Run Publish Procedure

1. Verify clean working tree:

   ```bash
   git status --short
   ```

2. Run package dry-run checks:

   ```bash
   cargo publish --dry-run -p stateql-core
   cargo publish --dry-run -p stateql-testkit
   ```

3. Review packaged file list and warnings in command output.

## Failure Checklist

- `all dependencies must have a version specified`:
  - Add `version = "<crate-version>"` for internal path dependencies.
- `manifest has no description` (or other metadata warnings):
  - Fill missing `package` metadata fields listed above.
- Packaging includes unexpected files:
  - Tighten package include/exclude settings in crate `Cargo.toml`.
