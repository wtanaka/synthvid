#!/bin/sh
# Assert the catalogue is generated from declared axes, not hand-enumerated.
#
# The corpus is a covering array over explicit axes: the smallest set of
# entries in which every pair of values drawn from any two axes appears
# together at least once. Pairwise coverage is a standard test-design
# technique because it finds most interaction defects from a small number of
# cases, and generating the set rather than typing it keeps the corpus honest
# -- it cannot accumulate one-off entries that no axis explains and that
# nobody maintains.
#
# Enforced by permitting Entry to be constructed in exactly one file.
set -eu

crate=crates/synthvid-catalog/src
builder="$crate/cover.rs"

if [ ! -d "$crate" ]; then
    echo "check-catalog-generated: $crate not found, nothing to check"
    exit 0
fi

offenders=$(grep -rln --include='*.rs' -E '\bEntry\s*\{' "$crate" 2>/dev/null \
    | grep -v -x -- "$builder" || true)

if [ -n "$offenders" ]; then
    echo "check-catalog-generated: Entry constructed outside $builder:" >&2
    printf '%s\n' "$offenders" | sed 's/^/  /' >&2
    echo "check-catalog-generated: the catalogue must be a covering array" >&2
    echo "  over declared axes, never a hand-written list of entries" >&2
    exit 1
fi

echo "check-catalog-generated: ok"
