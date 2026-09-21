#!/bin/sh
# Mirror the Tests step in ci.yml.
#
# CI runs `cargo test --all` with `RUSTFLAGS="-D warnings"` in the environment.
# This guard replicates that behavior locally.
#
# CI also installs the stable toolchain on every run, so "passes CI" means
# "passes the current stable toolchain". An older toolchain cannot emit warnings
# introduced after it was cut, so running the ambient cargo when its default
# toolchain is stale passes locally on code CI rejects -- a green run that
# proves nothing. When rustup has a stable toolchain installed, run tests
# through it; otherwise fall back to the cargo on PATH and say so loudly.
#
# The check runs in a target directory of its own, which is deleted first.
# Both halves matter. Sharing the main target directory lets a `cargo check`,
# `cargo clippy`, or `cargo doc` run leave fingerprints behind that convince
# cargo the tests are already current, so testing never re-runs and the guard
# reports success without having compiled the test binaries. Deleting the
# directory removes the only other place that state could survive. The
# workspace has no external dependencies, so testing it from nothing costs a
# second or two and buys a result that means what it says.
set -eu

if ! command -v cargo >/dev/null 2>&1; then
    echo "check-tests: cargo not found" >&2
    exit 1
fi

cargo_cmd="cargo"
if command -v rustup >/dev/null 2>&1 \
    && rustup toolchain list 2>/dev/null | grep -q '^stable'; then
    cargo_cmd="rustup run stable cargo"
else
    echo "check-tests: no stable toolchain under rustup;" >&2
    echo "  using ambient cargo, which may be older than CI" >&2
fi

test_dir="${CARGO_TARGET_DIR:-target}/test-check"
rm -rf "$test_dir"

set +e
output=$(CARGO_TARGET_DIR="$test_dir" RUSTFLAGS="-D warnings" \
    $cargo_cmd test --all 2>&1)
exit_code=$?
set -e

if [ $exit_code -ne 0 ]; then
    echo "$output" >&2
    echo "check-tests: cargo test --all failed" >&2
    exit 1
fi

echo "check-tests: ok"
