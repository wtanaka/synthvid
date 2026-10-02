#!/bin/sh
# Mirror the Lints step in ci.yml.
#
# CI runs `cargo clippy --all-targets --all-features -- -D warnings` with
# `RUSTFLAGS="-D warnings"` in the environment. `--all-targets` matters: a
# lint that fires only in test code (for example `clippy::unnecessary-sort-by`
# inside a `#[cfg(test)]` module) is invisible to `cargo check` or
# `cargo clippy` without it, so the flags here must stay identical to ci.yml.
#
# The toolchain is pinned by `rust-toolchain.toml`, which rustup honours for
# CI and for every local invocation alike, so the cargo run here is the cargo
# CI runs. When rustup is absent, fall back to the cargo on PATH and say so
# loudly: it may not be the pinned version.
#
# The check runs in a target directory of its own, which is deleted first.
# Both halves matter. Sharing the main target directory lets a `cargo check`
# or `cargo doc` run leave fingerprints behind that convince cargo the
# lint output is already current, so clippy never re-runs and the guard
# reports success without having read the source. Deleting the directory
# removes the only other place that state could survive. The workspace has no
# external dependencies, so linting it from nothing costs a second or two
# and buys a result that means what it says.
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "check-clippy: cargo not found" >&2
    exit 1
fi

cargo_cmd="cargo"
if ! command -v rustup >/dev/null 2>&1; then
    echo "check-clippy: rustup not found;" >&2
    echo "  using ambient cargo, which may not be the pinned toolchain" >&2
fi

clippy_dir="${CARGO_TARGET_DIR:-target}/clippy-check"
rm -rf "$clippy_dir"

set +e
output=$(CARGO_TARGET_DIR="$clippy_dir" RUSTFLAGS="-D warnings" \
    $cargo_cmd clippy --all-targets --all-features -- -D warnings 2>&1)
exit_code=$?
set -e

if [ $exit_code -ne 0 ]; then
    echo "$output" >&2
    echo "check-clippy: cargo clippy --all-targets --all-features -- -D warnings failed" >&2
    exit 1
fi

echo "check-clippy: ok"
