# Ralph Agent Instructions — stateql v1

## Context

You are running in the root of the `stateql` project.
The loop configuration files are in `ralph-loop/`.
The implementation plan is in `docs/plans/2026-02-21-stateql-v1-plan.md`.

## Rules

- Follow `AGENTS.md` (project policy), `docs/style.md` (coding standards), and `docs/testing.md` (testing guidelines).
- `reference/sqldef/**` is reference-only. Never edit it.
- New code goes under `crates/**`.
- Use the `execute-plan` skill for each task. It enforces TDD (RED -> GREEN -> REFACTOR) and human review checkpoints.
- Each task has a **DoD** (Definition of Done) section in the plan. All DoD items must pass before marking the task as complete.
- **Do NOT run `git commit`.** Your sandbox cannot write to `.git/`. Instead, write the plan-specified commit message (just the `-m` string) to `ralph-loop/.commit-msg`. The loop script will commit for you.
- **Implement exactly ONE task per turn.** After completing one task, update tracking files and end your turn. Do NOT start the next task — the loop script handles iteration.
- **Quality gates — run these before finishing your turn:**
  1. `cargo +nightly-2026-02-20 fmt --all` — fix formatting.
  2. `cargo clippy --workspace --all-targets -- -D warnings` — fix all warnings (refactor code, do not suppress with `#[allow]`).
  3. `cargo nextest run --workspace` — all tests must pass.
  If any gate fails, fix the issue before writing `.commit-msg` and updating tracking files.

## Your Task

1. Read `ralph-loop/prd.json` to see all stories and their status.
2. Read `ralph-loop/progress.txt` to see what has been done and any codebase patterns discovered so far.
3. Find the highest-priority story where `passes: false` AND all its `deps` have `passes: true`.
   - Priority order: pick the task with the lowest numeric ID among eligible tasks.
   - If no task is eligible (all remaining tasks are blocked by incomplete deps), report which tasks are blocked and why, then end your turn.
4. Read the corresponding task section in `docs/plans/2026-02-21-stateql-v1-plan.md` for full details.
5. Use the `execute-plan` skill to implement that ONE task following TDD.
6. Run the quality gates listed above. Fix any failures.
7. After all DoD items and quality gates pass, write the plan-specified commit message to `ralph-loop/.commit-msg`.
8. Update `ralph-loop/prd.json`: set `passes: true` for the completed story.
9. Append progress to `ralph-loop/progress.txt`.
10. **End your turn.** Do not start another task.

## Progress Format

APPEND to `ralph-loop/progress.txt`:

```
## [Date] - [Story ID]: [Title]
- What was implemented
- Files changed
- DoD verification results
- **Learnings:**
  - Patterns discovered
  - Gotchas encountered
---
```

## Codebase Patterns

Add reusable patterns to the TOP of `ralph-loop/progress.txt` under a `## Codebase Patterns` header.
Update this section as you discover new patterns.

## Stop Condition

If ALL stories in `ralph-loop/prd.json` have `passes: true`, reply with EXACTLY:
<promise>COMPLETE</promise>

Otherwise end your turn normally after writing the commit message and updating the tracking files.
