#!/bin/sh
# Ensure crate documentation builds without warnings.
#
# The workspace denies missing_docs, so a missing doc comment already fails
# the build. This covers what that lint cannot: broken intra-doc links and
# other rustdoc warnings. It mirrors the Documentation step in ci.yml, which
# sets RUSTDOCFLAGS="-D warnings".
#
# The check runs in a target directory of its own, which is deleted first.
# Both halves matter. Sharing the main target directory lets a `cargo check`
# or `cargo clippy` run leave fingerprints behind that convince cargo the
# documentation is already current, so rustdoc never re-runs and the guard
# reports success without having read the source. Deleting the directory
# removes the only other place that state could survive. The workspace has no
# external dependencies, so documenting it from nothing costs a second or two
# and buys a result that means what it says.
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "check-rustdoc: cargo not found" >&2
    exit 1
fi

doc_dir="${CARGO_TARGET_DIR:-target}/rustdoc-check"
rm -rf "$doc_dir"

if ! CARGO_TARGET_DIR="$doc_dir" RUSTDOCFLAGS="-D warnings" \
    cargo doc --no-deps --all; then
    echo "check-rustdoc: cargo doc --no-deps --all failed" >&2
    exit 1
fi

echo "check-rustdoc: ok"
