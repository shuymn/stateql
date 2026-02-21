# Architecture Decision Records

This directory stores architecture decisions extracted from `DESIGN.md`.

## Index

- [ADR-0001: Canonical IR Between Parsers and Diff Engine](./0001-canonical-ir.md)
- [ADR-0002: DiffOp-Based Planning and Batch SQL Generation](./0002-diffop-batch-sql-generation.md)
- [ADR-0003: Source-Level Extensibility via Dialect Traits](./0003-source-level-extensibility.md)
- [ADR-0004: Minimal Expression AST with Canonicalization for Raw Expressions](./0004-expression-representation-and-canonicalization.md)
- [ADR-0005: Reuse sqldef YAML Tests with Incremental Porting](./0005-yaml-test-reuse.md)
- [ADR-0006: Explicit Rename Annotations, No Heuristic Rename Inference](./0006-explicit-rename-annotations.md)
- [ADR-0007: BatchBoundary Represents Synchronization, Not Commit](./0007-batchboundary-semantics.md)
- [ADR-0008: DatabaseAdapter Uses a Single Dedicated Connection](./0008-single-connection-adapter.md)
- [ADR-0009: CLI Uses Single Binary with Dialect Subcommands](./0009-single-binary-cli-shape.md)
- [ADR-0010: Parser Selection Policy by Dialect](./0010-parser-selection-policy.md)
- [ADR-0011: Synchronous I/O for All Layers](./0011-synchronous-io.md)
- [ADR-0012: Minimum Supported Database Versions for v1](./0012-minimum-supported-database-versions.md)
- [ADR-0013: Fail-Fast Error Handling for Parse and Execution](./0013-fail-fast-error-handling.md)
- [ADR-0014: Schema Search Path for Name Resolution](./0014-schema-search-path.md)
- [ADR-0015: Semantic Equivalence Policy Injection for Diff Comparison](./0015-equivalence-policy-injection.md)
- [ADR-0016: Core-Managed View Rebuild Scope and Owned Diff Planning](./0016-view-rebuild-scope-and-owned-diff-planning.md)
