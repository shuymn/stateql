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

for i in $(seq 1 "$MAX_ITERATIONS"); do
  echo "=== Iteration $i/$MAX_ITERATIONS ==="

  PROMPT_CONTENT=$(cat "$SCRIPT_DIR/prompt.md")

  echo "Agent running..."

  OUTPUT=$(echo "$PROMPT_CONTENT" | $AGENT_CMD 2>&1 | tee /dev/stderr) || true

  if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
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
