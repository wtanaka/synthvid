#!/bin/sh
# Ensure the workspace is formatted with rustfmt.
#
# rustfmt reads source files directly rather than compiling them, so unlike
# check-clippy.sh and check-rustdoc.sh it has no target-directory fingerprint
# to launder a stale result through. No isolated target dir is needed here.
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "check-fmt: cargo not found" >&2
    exit 1
fi

set +e
output=$(cargo fmt --all -- --check 2>&1)
exit_code=$?
set -e

if [ $exit_code -ne 0 ]; then
    echo "$output" >&2
    echo "check-fmt: cargo fmt --all -- --check failed" >&2
    exit 1
fi

echo "check-fmt: ok"
