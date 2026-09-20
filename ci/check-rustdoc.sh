#!/bin/sh
# Ensure crate documentation builds without warnings.
#
# The workspace denies missing_docs, so a missing doc comment already fails
# the build. This covers what that lint cannot: broken intra-doc links and
# other rustdoc warnings. It mirrors the Documentation step in ci.yml, which
# sets RUSTDOCFLAGS="-D warnings".
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "check-rustdoc: cargo not found" >&2
    exit 1
fi

if ! RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all; then
    echo "check-rustdoc: cargo doc --no-deps --all failed" >&2
    exit 1
fi

echo "check-rustdoc: ok"
