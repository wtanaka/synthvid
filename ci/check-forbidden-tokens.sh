#!/bin/sh
# Grep for constructs that break determinism or purity.
#
# Clippy already denies unwrap/expect/panic/indexing. This covers the rules
# clippy cannot express: platform-dependent maths, unordered collections,
# ambient inputs, and I/O outside the shell crate.
set -eu

status=0

report() {
    # $1 = message, $2 = matches
    if [ -n "$2" ]; then
        echo "check-forbidden-tokens: $1" >&2
        printf '%s\n' "$2" | sed 's/^/  /' >&2
        status=1
    fi
}

scan() {
    # $1 = pattern, $2 = paths
    # Comment lines are excluded: rustdoc examples legitimately mention I/O,
    # and a rule about what the code does should not be tripped by prose.
    grep -rnE --include='*.rs' -- "$1" $2 2>/dev/null \
        | grep -vE '^[^:]*:[0-9]+:[[:space:]]*(///|//!|//)' || true
}

# --- Determinism: banned in every crate, tests included. ---------------------
# Platform math libraries are not bit-identical across targets.
report "platform transcendental maths (use the in-crate implementation)" \
    "$(scan '\.(sin|cos|tan|asin|acos|atan|atan2|exp|ln|log10|log2|powf|hypot|cbrt)\(' crates)"

# No floating point. The banned transcendental functions above are the
# famous case, but ordinary float arithmetic is also not bit-identical
# across targets once an optimiser is allowed to contract or
# reassociate it, and a value that round-trips through f64 has already
# lost the exactness this workspace is built on. The pure crates
# compute in integers and exact rationals; there is nothing here a
# float may legitimately do.
#
# This rule went in the moment the last one disappeared.
# `Ratio::to_f64` was the only float conversion in the workspace,
# documented as being "for the render path" and carrying the
# workspace's only lint suppression, and it had no caller anywhere
# outside its own test. Deleting it took the suppression budget to
# zero.
report "floating point (compute in integers or exact rationals)" \
    "$(scan '\bf32\b|\bf64\b' crates/synthvid-scene crates/synthvid-catalog crates/synthvid-encode)"

# Iteration order is randomised per process.
report "unordered collection (use BTreeMap or BTreeSet)" \
    "$(scan '\b(HashMap|HashSet|RandomState)\b' crates)"

# Ambient inputs.
report "clock or ambient input" \
    "$(scan '\b(SystemTime|Instant|std::time|std::env|env::var)\b' crates)"

# Any external crate reference at all.
report "external crate reference" \
    "$(scan '^\s*(use|extern crate)\s+(rand|serde|libc|anyhow|thiserror|itertools|num|byteorder|once_cell|lazy_static)\b' crates)"

report "unsafe code" \
    "$(scan '\bunsafe\b' crates)"

# --- Purity: I/O is permitted only in the command-line crate. ----------------
pure_crates=""
for d in crates/*/; do
    case "$d" in
        */synthvid-cli/) ;;
        */synthvid-validate/) ;;
        *) pure_crates="$pure_crates $d" ;;
    esac
done

if [ -n "$pure_crates" ]; then
    report "I/O in a pure crate (only synthvid-cli may touch the outside world)" \
        "$(scan '\b(std::fs|std::net|std::process|std::io::(stdin|stdout|stderr)|File::|println!|eprintln!|print!)' "$pure_crates")"
fi

if [ "$status" -ne 0 ]; then
    echo "check-forbidden-tokens: these constructs make output depend on" >&2
    echo "  the platform, the process, or the environment, and this project" >&2
    echo "  guarantees identical output everywhere" >&2
    exit 1
fi

echo "check-forbidden-tokens: ok"
