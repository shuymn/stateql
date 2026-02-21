# ADR-0006: Explicit Rename Annotations, No Heuristic Rename Inference

- Status: Accepted
- Date: 2026-02-21

## Context

Automatic rename inference can produce false positives that lead to unintended data loss.
The tool must prioritize safety over convenience in rename handling.

## Decision

Renames are applied only when explicitly declared in desired SQL comments.

- Supported syntax: `@renamed from=...`
- Deprecated syntax accepted for compatibility: `@rename from=...` (with warning)
- No heuristic matching by similarity, position, or edit distance

Annotation handling is a required pre-parse step:
- Extract annotations from comments.
- Parse cleaned SQL.
- Attach annotations to matching IR objects.
- Return error for orphan or invalid annotations.

Examples:

```sql
-- Table rename
CREATE TABLE new_name (  -- @renamed from=old_name
  id bigint NOT NULL
);

-- Column rename
CREATE TABLE users (
  user_id bigint NOT NULL, -- @renamed from=username
  name text
);

-- Quoted identifiers (case-sensitive rename)
CREATE TABLE users (
  "UserId" bigint NOT NULL, -- @renamed from="user_id"
  name text
);
```

## Consequences

Positive:
- Eliminates implicit data migration caused by heuristic rename detection.
- Makes rename intent explicit and reviewable in schema files.

Negative:
- Users must add annotations for rename scenarios.
- Missing annotations lead to create/drop behavior.

## Notes

Fail-fast behavior for orphan annotations is part of the safety model.
