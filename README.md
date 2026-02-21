# stateql

`stateql` is a schema-diff and migration planning tool inspired by `sqldef`.
It is **not** a drop-in replacement for `sqldef`.

## Current Status

Design and bootstrap phase. The Rust workspace and crate skeleton are initialized.

## First Implementation Slice

The first vertical slice is intentionally narrow:

- PostgreSQL only
- Table, column, and index subset
- Offline diff / dry-run only (no apply)
- YAML test runner with a tiny starter corpus

## Documents

- [Design Document](./DESIGN.md)
- [Architecture Decision Records (ADR Index)](./docs/adr/README.md)
- [Bootstrap Checklist](./docs/bootstrap-after-git-init.md)
