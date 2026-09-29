#!/bin/sh
# Mirror the Tests step in ci.yml.
#
# CI runs `cargo test --release --locked --all` with `RUSTFLAGS="-D warnings"`
# in the environment. This guard replicates that behavior locally.
# `--release`: this workspace's own type-safety rules (checked arithmetic
# everywhere, no raw indexing) are expensive without optimization and nearly
# free with it -- an unoptimized debug build measured tens of times slower
# than release for this crate's JPEG path, which is also what the corpus
# generate/verify steps below need to stay near the reproducibility job's own
# time budget. `--locked`: fail rather than silently update `Cargo.lock` if it
# and `Cargo.toml` ever drift, and avoid probing the (nonexistent, since this
# workspace has no external dependencies) registry for a newer version.
#
# The toolchain is pinned by `rust-toolchain.toml`, which rustup honours for
# CI and for every local invocation alike, so the cargo run here is the cargo
# CI runs. When rustup is absent, fall back to the cargo on PATH and say so
# loudly: it may not be the pinned version.
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
if ! command -v rustup >/dev/null 2>&1; then
    echo "check-tests: rustup not found;" >&2
    echo "  using ambient cargo, which may not be the pinned toolchain" >&2
fi

test_dir="${CARGO_TARGET_DIR:-target}/test-check"
rm -rf "$test_dir"

set +e
output=$(CARGO_TARGET_DIR="$test_dir" RUSTFLAGS="-D warnings" \
    $cargo_cmd test --release --locked --all 2>&1)
exit_code=$?
set -e

if [ $exit_code -ne 0 ]; then
    echo "$output" >&2
    echo "check-tests: cargo test --release --locked --all failed" >&2
    exit 1
fi

echo "check-tests: ok"
