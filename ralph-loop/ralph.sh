#!/bin/bash
set -e

# Configuration
MAX_ITERATIONS=${MAX_ITERATIONS:-60}
AGENT_CMD="$1"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

if [ -z "$AGENT_CMD" ]; then
  echo "Usage: ./ralph-loop/ralph.sh \"<agent command>\" [max_iterations]"
  echo ""
  echo "Example:"
  echo "  ./ralph-loop/ralph.sh \"codex exec --full-auto\" 60"
  echo ""
  echo "Environment variables:"
  echo "  MAX_ITERATIONS  Max loop iterations (default: 60)"
  exit 1
fi

if [ -n "$2" ]; then
  MAX_ITERATIONS=$2
fi

echo "Starting Ralph with agent: '$AGENT_CMD'"
echo "Working directory: $ROOT_DIR"
echo "Max iterations: $MAX_ITERATIONS"
echo ""

cd "$ROOT_DIR"

# Run quality gates (fmt, clippy fix) and verify tests before committing.
# Returns 0 if gates pass, 1 if tests fail (commit should be skipped).
quality_gate() {
  echo "  [ralph] Running quality gates..."

  # Auto-fix formatting
  cargo +nightly-2026-02-20 fmt --all 2>&1 || true

  # Auto-fix clippy warnings (allow dirty working tree)
  cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged -- -D warnings 2>&1 || true

  # Verify tests pass
  if ! cargo nextest run --workspace 2>&1; then
    echo "  [ralph] WARNING: tests failed, skipping commit"
    return 1
  fi

  echo "  [ralph] Quality gates passed"
  return 0
}

# Auto-commit changes left by the agent (sandbox cannot write .git/)
auto_commit() {
  local commit_msg_file="$SCRIPT_DIR/.commit-msg"

  # Check for any uncommitted changes (tracked or untracked under monitored dirs)
  if git diff --quiet && git diff --cached --quiet \
     && [ -z "$(git ls-files --others --exclude-standard crates/ docs/ tests/)" ]; then
    return 0
  fi

  echo "  [ralph] Uncommitted changes detected, auto-committing..."

  # Run quality gates; abort loop if tests fail so diffs don't accumulate
  if ! quality_gate; then
    echo "  [ralph] ERROR: quality gates failed — stopping loop to avoid mixed diffs"
    exit 2
  fi

  # Stage quality-gate results immediately — fmt/clippy --fix may have updated
  # crates/ source, Cargo.lock, docs/, and tests/. This ensures the commit
  # includes everything quality_gate produced, matching what lefthook will verify.
  git add crates/ Cargo.lock docs/ tests/

  # Read commit message written by the agent
  local code_msg="feat(core): implement task (auto-commit)"
  if [ -f "$commit_msg_file" ]; then
    code_msg=$(cat "$commit_msg_file")
    rm -f "$commit_msg_file"
  fi

  # 1. Commit code changes (crates/, Cargo.lock, docs/, tests/) — check staged changes
  if ! git diff --cached --quiet -- crates/ Cargo.lock docs/ tests/; then
    if ! git commit -m "$code_msg"; then
      echo "  [ralph] WARN: code commit failed, retrying with --no-gpg-sign..."
      if ! git commit --no-gpg-sign -m "$code_msg"; then
        echo "  [ralph] ERROR: code commit failed — stopping loop"
        exit 2
      fi
    fi
    echo "  [ralph] Code committed: $code_msg"
  fi

  # Fallback: pre-existing Cargo.lock changes may be staged (line 75) but not
  # included in the code commit — the pre-commit hook (lefthook stage_fixed) can
  # modify the index, dropping non-glob files like Cargo.lock.
  # Quality gate already validated the workspace, so hooks are skipped here.
  if ! git diff --quiet -- Cargo.lock; then
    git add Cargo.lock
    if LEFTHOOK=0 git commit --no-gpg-sign -m "chore: update Cargo.lock"; then
      echo "  [ralph] Cargo.lock committed (post-hook fallback)"
    fi
  fi

  # 2. Commit tracking changes (ralph-loop/)
  if ! git diff --quiet -- ralph-loop/; then
    # Extract task-ID by comparing completed tasks in HEAD vs working copy
    local task_id
    task_id=$(diff \
      <(git show HEAD:ralph-loop/prd.json | jq -r '[.stories[] | select(.passes==true) | .id] | sort[]') \
      <(jq -r '[.stories[] | select(.passes==true) | .id] | sort[]' ralph-loop/prd.json) \
      | grep '^>' | sed 's/^> //' | paste -sd, -) || true
    task_id=${task_id:-unknown}

    git add ralph-loop/
    if ! git commit -m "chore(ralph): mark ${task_id} complete in PRD and progress"; then
      echo "  [ralph] WARN: tracking commit failed, retrying with --no-gpg-sign..."
      if ! git commit --no-gpg-sign -m "chore(ralph): mark ${task_id} complete in PRD and progress"; then
        echo "  [ralph] ERROR: tracking commit failed — stopping loop"
        exit 2
      fi
    fi
    echo "  [ralph] Tracking committed for ${task_id}"
  fi
}

for i in $(seq 1 "$MAX_ITERATIONS"); do
  echo "=== Iteration $i/$MAX_ITERATIONS ==="

  PROMPT_CONTENT=$(cat "$SCRIPT_DIR/prompt.md")

  echo "Agent running..."

  OUTPUT=$(echo "$PROMPT_CONTENT" | $AGENT_CMD 2>&1 | tee /dev/stderr) || true

  # Auto-commit any changes the agent made
  auto_commit

  # Check completion signal only in the tail of the output to avoid
  # false-positive matches against the echoed prompt.
  if echo "$OUTPUT" | tail -20 | grep -qF "<promise>COMPLETE</promise>"; then
    echo ""
    echo "Done! All tasks completed."
    exit 0
  fi

  echo "Iteration $i finished. Sleeping for 5 seconds..."
  sleep 5
done

echo ""
echo "WARNING: Max iterations ($MAX_ITERATIONS) reached without completion signal."
exit 1
