# ADR-0003: Source-Level Extensibility via Dialect Traits

- Status: Accepted
- Date: 2026-02-21

## Context

The current architecture requires touching many core locations for each new database.
A plugin model is desirable for extension, but runtime plugin loading introduces ABI and security complexity.

## Decision

Adopt source-level extensibility:

- New dialects are Rust crates implementing `Dialect`.
- Dialects are enabled via Cargo features and linked at build time.
- Official binaries include built-in dialects.
- No runtime plugin loading in v1.
- Core uses `&dyn Dialect` trait objects for runtime dialect selection in CLI flow, but the concrete implementations are statically linked at compile time.

## Consequences

Positive:
- Keeps runtime simple and avoids ABI compatibility concerns.
- Third-party dialects can be added without forking as long as they fit existing IR object kinds.
- Binary size can be reduced by selecting only needed dialect features.

Negative:
- Pre-built binaries cannot be extended without recompilation.
- Dialects needing new object kinds still require core changes.

## Notes

This relies on ADR-0001's closed IR model for safety.
Trait-object dispatch here is an internal polymorphism mechanism, not a runtime plugin boundary.
