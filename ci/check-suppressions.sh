#!/bin/sh
# Enforce suppression discipline.
#
# The lint table denies `clippy::allow_attributes`, so `#[allow]` is already a
# build error. This guard exists because that denial lives in a file, and a
# file can be edited; a check that runs outside the compiler cannot be turned
# off from inside the code.
#
# `#[expect(lint, reason = "...")]` is the permitted form. It carries a written
# justification, and the compiler removes it the moment the underlying lint
# stops firing, so a suppression cannot quietly outlive its cause. The total is
# capped so that adding one is a visible change to a committed number rather
# than an invisible change to a source file.
set -eu

budget_file="${1:-ci/suppression-budget.txt}"
status=0

bare=$(grep -rn --include='*.rs' -E '#!?\[allow\(' crates 2>/dev/null || true)
if [ -n "$bare" ]; then
    echo "check-suppressions: #[allow] is not permitted; use" >&2
    echo "  #[expect(lint, reason = \"...\")] instead" >&2
    printf '%s\n' "$bare" | sed 's/^/  /' >&2
    status=1
fi

# An #[expect] spanning several lines carries its reason on a later line, so
# only single-line forms are checked for a missing reason.
noreason=$(grep -rn --include='*.rs' -E '#!?\[expect\([^]]*\)\]' crates 2>/dev/null \
    | grep -v 'reason' || true)
if [ -n "$noreason" ]; then
    echo "check-suppressions: every #[expect] must carry reason = \"...\"" >&2
    printf '%s\n' "$noreason" | sed 's/^/  /' >&2
    status=1
fi

count=$(grep -rc --include='*.rs' -E '#!?\[expect\(' crates 2>/dev/null \
    | awk -F: '{ total += $2 } END { print total + 0 }')

if [ ! -f "$budget_file" ]; then
    echo "check-suppressions: $budget_file not found" >&2
    exit 1
fi

budget=$(tr -dc '0-9' < "$budget_file")
if [ -z "$budget" ]; then
    echo "check-suppressions: $budget_file must contain a number" >&2
    exit 1
fi

if [ "$count" -gt "$budget" ]; then
    echo "check-suppressions: $count suppressions exceeds the budget of $budget" >&2
    echo "  Remove one, or raise the budget deliberately and say why in the" >&2
    echo "  commit. The budget ratchets down over time and never up by accident." >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-suppressions: ok ($count of $budget used)"
