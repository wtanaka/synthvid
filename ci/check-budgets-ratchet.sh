#!/bin/sh
# Budgets must only ever ratchet down.
#
# Three files cap a count: suppression-budget.txt, test-hygiene-budget.txt,
# and type-safety-budget.txt. Each guard checks count <= budget. This guard
# ensures the budget itself has not been raised. A change raised the
# suppression budget from 3 to 6 in the same commit that needed the extra room,
# and every guard passed. This check prevents that.
#
# If a budget file does not exist at the base revision, it is a new budget and
# not a raise. If git is unavailable, exit non-zero rather than silently
# passing -- a guard that cannot check must not report success.
set -eu

base="${BUDGET_BASE:-main}"
status=0

# Every budget file, discovered rather than listed. A hardcoded list is a
# registration step someone has to remember, and the first two budgets added
# after this guard was written were both missed by it -- so the guard meant to
# stop budgets drifting upward did not cover the budgets added beside it.
budgets=$(find ci -maxdepth 1 -name '*-budget.txt' -type f | sort)
if [ -z "$budgets" ]; then
    echo "check-budgets-ratchet: no ci/*-budget.txt files found" >&2
    exit 1
fi

for budget_file in $budgets; do
    # Get current value
    if [ ! -f "$budget_file" ]; then
        echo "check-budgets-ratchet: $budget_file not found" >&2
        exit 1
    fi

    current=$(tr -dc '0-9' < "$budget_file")
    if [ -z "$current" ]; then
        echo "check-budgets-ratchet: $budget_file must contain a number" >&2
        exit 1
    fi

    # Try to get the base value using git
    if ! command -v git >/dev/null 2>&1; then
        echo "check-budgets-ratchet: git not found in PATH" >&2
        exit 1
    fi

    # git show "$base:<path>" to get the value at the base revision
    base_content=$(git show "$base:$budget_file" 2>/dev/null || echo "")

    if [ -z "$base_content" ]; then
        # File didn't exist at base revision - this is a new budget, not a raise
        continue
    fi

    base_value=$(printf '%s' "$base_content" | tr -dc '0-9')
    if [ -z "$base_value" ]; then
        # Base file didn't contain a number, skip
        continue
    fi

    # Check if current > base (budget increased)
    if [ "$current" -gt "$base_value" ]; then
        echo "check-budgets-ratchet: $budget_file increased from $base_value to $current" >&2
        echo "  Budgets must ratchet down, never up. Revert this change or justify it" >&2
        echo "  in the commit message explaining why a higher bound is necessary." >&2
        status=1
    fi
done

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-budgets-ratchet: ok (all budgets ratcheted down or unchanged)"
