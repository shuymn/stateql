# ADR-0009: CLI Uses Single Binary with Dialect Subcommands

- Status: Accepted
- Date: 2026-02-22

## Context

Two CLI shapes were considered:
- One binary with dialect subcommands
- Separate binaries per dialect for full legacy parity (`*def` style)

The project is inspired by sqldef but is intentionally a distinct product.
Reusing `*def` command names or requiring drop-in compatibility would blur product identity
and force architecture decisions to follow legacy behavior instead of v1 design priorities.

## Decision

Primary interface is a single binary named `stateql` with dialect subcommands.

- No `*def` compatibility aliases/symlinks are shipped.
- v1 is not a drop-in replacement for sqldef.
- Familiar concepts and options may be reused where useful, but strict flag-level and output-level parity is not required.

## Consequences

Positive:
- One distributable artifact and unified command discovery (`stateql <dialect> ...`).
- Clear product identity and reduced naming confusion with sqldef-derived tools.
- CLI/API can evolve for clarity and safety without being constrained by legacy parity.

Negative:
- Existing sqldef scripts cannot be reused unchanged.
- Users migrating from sqldef need explicit command/flag mapping documentation.
