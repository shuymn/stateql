# ADR-0005: Reuse sqldef YAML Tests with Incremental Porting

- Status: Accepted
- Date: 2026-02-21

## Context

The existing YAML corpus captures years of schema edge cases.
Rewriting all tests from scratch would lose proven coverage and significantly delay delivery.

## Decision

Reuse the existing YAML format and migrate tests incrementally with a Rust test runner (`testkit`).

Porting order per dialect:
1. Idempotency-only cases.
2. Assertion cases grouped by feature.
3. Remaining compatibility and message-specific cases.

## Consequences

Positive:
- Preserves domain knowledge and avoids restarting test discovery.
- Keeps test definitions declarative and language-agnostic.

Negative:
- `up` and `down` expectations require large-scale updates due to output differences.
- `legacy_ignore_quotes`-dependent cases require intentional rewrites.

## Notes

Reuse is strategic, not zero-cost copy.
