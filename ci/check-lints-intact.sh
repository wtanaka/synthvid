#!/bin/sh
# Assert the workspace lint configuration has not been weakened.
#
# Every other guard can be defeated by editing the lint table, which makes that
# table the softest target in the repository and the one most worth protecting.
# Its digest is recorded, so relaxing a lint is a two-file change that states
# plainly what it is doing.
#
# Also checks that no member crate has opted out: a crate whose manifest lacks
# `[lints] workspace = true` inherits nothing, and would be silently unlinted.
set -eu

digest_file="${1:-ci/lints.sha256}"
status=0

sha_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        echo "check-lints-intact: no sha256 tool available" >&2
        exit 1
    fi
}

if [ ! -f "$digest_file" ]; then
    echo "check-lints-intact: $digest_file not found" >&2
    exit 1
fi

expected=$(tr -dc '0-9a-f' < "$digest_file")
actual=$(sha_of Cargo.toml)

if [ "$expected" != "$actual" ]; then
    echo "check-lints-intact: the workspace manifest has changed" >&2
    echo "  expected $expected" >&2
    echo "  actual   $actual" >&2
    echo "  If the change is intended, update $digest_file in the same commit" >&2
    echo "  and state what lint changed and why." >&2
    status=1
fi

for manifest in crates/*/Cargo.toml; do
    [ -e "$manifest" ] || continue
    if ! grep -qE '^[[:space:]]*workspace[[:space:]]*=[[:space:]]*true' "$manifest"; then
        echo "check-lints-intact: $manifest does not inherit [lints]" >&2
        status=1
    fi
done

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-lints-intact: ok"
