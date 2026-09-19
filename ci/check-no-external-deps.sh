#!/bin/sh
# Assert Cargo.lock contains only this workspace's own crates.
#
# This is the highest-value guard in the repository. Reaching for a crate is
# the most likely way the no-dependencies rule gets broken, and it is the one
# rule whose violation silently destroys the reproducibility guarantee: a
# third-party crate can change its output between patch versions.
#
# The allowed set is derived from the filesystem rather than written down, so
# it cannot drift away from the actual workspace.
set -eu

lock="${1:-Cargo.lock}"

if [ ! -f "$lock" ]; then
    echo "check-no-external-deps: $lock not found" >&2
    exit 1
fi

if [ ! -d crates ]; then
    echo "check-no-external-deps: no crates/ directory" >&2
    exit 1
fi

allowed=$(find crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)
found=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$lock" | sort -u)

status=0
for pkg in $found; do
    if ! printf '%s\n' "$allowed" | grep -qx -- "$pkg"; then
        echo "check-no-external-deps: forbidden dependency: $pkg" >&2
        status=1
    fi
done

if [ "$status" -ne 0 ]; then
    echo "check-no-external-deps: $lock must contain only workspace crates" >&2
    exit 1
fi

echo "check-no-external-deps: ok ($(printf '%s\n' "$found" | wc -l | tr -d ' ') workspace crates, no others)"
