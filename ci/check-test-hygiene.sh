#!/bin/sh
# Enforce that a test which fails reports as failing.
#
# Four patterns in test code turn a broken test into a passing one. Each has
# been found in this repository, and the first was found only because two
# competing implementations of the same work happened to be available to diff
# against each other.
#
#   return;          A test that returns early never reaches its assertions and
#                    is reported as passing. The idiom that produces this is
#                    `let Some(x) = expr else { return };`, written because
#                    `clippy::unwrap_used` is denied workspace-wide. clippy.toml
#                    now permits unwrap in tests, so a fixture that cannot be
#                    built should panic and fail the test instead.
#
#   let _ =          Launders an unused binding past `unused_variables`, which
#                    hides a value the test computed and then forgot to assert
#                    anything about.
#
#   unwrap_or*       In an expected value, silently substitutes a fallback, so
#                    the assertion compares against something the test never
#                    meant. `unwrap_or` is also how an expected value ends up
#                    computed by the same code under test, which asserts
#                    nothing at all.
#
#   no assertion     A `#[test]` function containing no assert! / assert_eq! /
#                    assert_ne! passes unconditionally.
#
# Only code from the first `#[cfg(test)]` line of a file to its end is scanned.
# Every test module in this workspace sits at the end of its file, so that span
# is exactly the test code and nothing else. Scanning the whole file instead
# would count production code, and the total would then move whenever unrelated
# work changed a `return` outside a test -- a guard whose reading drifts for
# reasons it does not name is worse than no guard, because it still looks like
# it is ratcheting.
#
# The total is capped so that adding a violation is a visible change to a
# committed number rather than an invisible change to a source file.
set -eu

budget_file="${1:-ci/test-hygiene-budget.txt}"

counts=$(find crates -name '*.rs' -type f 2>/dev/null | sort | while read -r f; do
    awk '
        /#\[cfg\(test\)\]/ { intest = 1 }
        intest {
            line = $0; returns += gsub(/return[ \t]*[;}]/, "", line)
            line = $0; lets    += gsub(/let[ \t]+_[ \t]*=/, "", line)
            line = $0; uors    += gsub(/unwrap_or[a-z_]*\(/, "", line)
            if ($0 ~ /#\[test\]/) {
                if (seen && !asserted) { noassert++ }
                seen = 1; asserted = 0
            }
            if ($0 ~ /assert(_eq|_ne)?!/) { asserted = 1 }
        }
        END {
            if (seen && !asserted) { noassert++ }
            printf "%d %d %d %d\n", returns + 0, lets + 0, uors + 0, noassert + 0
        }
    ' "$f"
done | awk '{ r += $1; l += $2; u += $3; n += $4 }
            END { printf "%d %d %d %d\n", r + 0, l + 0, u + 0, n + 0 }')

returns=$(echo "$counts" | cut -d' ' -f1)
lets=$(echo "$counts" | cut -d' ' -f2)
uors=$(echo "$counts" | cut -d' ' -f3)
noassert=$(echo "$counts" | cut -d' ' -f4)
count=$((returns + lets + uors + noassert))

if [ ! -f "$budget_file" ]; then
    echo "check-test-hygiene: $budget_file not found" >&2
    exit 1
fi

budget=$(tr -dc '0-9' < "$budget_file")
if [ -z "$budget" ]; then
    echo "check-test-hygiene: $budget_file must contain a number" >&2
    exit 1
fi

if [ "$count" -gt "$budget" ]; then
    echo "check-test-hygiene: $count violations exceeds the budget of $budget" >&2
    echo "  early return in a test   $returns" >&2
    echo "  let _ = binding          $lets" >&2
    echo "  unwrap_or in a test      $uors" >&2
    echo "  test with no assertion   $noassert" >&2
    echo "  Each of these lets a broken test report success. Fix the test," >&2
    echo "  never the budget: it ratchets down and is never raised." >&2
    exit 1
fi

echo "check-test-hygiene: ok ($count of $budget used)"
