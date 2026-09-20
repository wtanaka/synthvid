#!/bin/sh
# Keep invalid states unrepresentable in the pure crates.
#
# The lint table cannot express any of this: it is about which shapes an API
# is allowed to have, not about whether a given expression is sound. Each rule
# below is here because the absence of it produced a real defect.
#
#   pub fn *_raw         A public method taking an unrestricted String as a
#                        value turns every guarantee its type makes into a doc
#                        comment. A canonical-JSON writer that promises no
#                        floats, no escapes and sorted keys promises nothing
#                        while `insert_raw(key, "1.5")` compiles. Such a method
#                        may exist as `pub(crate)`; it may not be public.
#
#   pub field: primitive A named public field of a bare numeric, bool or String
#                        type re-admits every state the newtype around it was
#                        built to refuse. Two public `u32`s in place of a
#                        checked span restore the empty and inverted spans its
#                        constructor rejects. Tuple newtypes -- `pub struct
#                        FrameIndex(pub u32)` -- are the exception this does not
#                        match, since they are how a primitive becomes a type.
#
#   pub fn(_: String)    Same failure at the parameter instead of the field. A
#                        closed set of four shape names is an enum; as a String
#                        it admits every value but the four that are legal.
#
#   assert!/panic!/      A runtime check standing where a type should. A
#   unwrap/expect        duplicate key is not a thing to assert against; it is a
#                        thing a map makes impossible.
#
#                        `clippy::panic` does not see `assert!`, and
#                        `clippy::unwrap_used` and `expect_used` -- which do see
#                        the rest -- can be switched off from inside the very
#                        file that needs them, with an `#[expect]` and a
#                        plausible reason. That is not hypothetical: the one
#                        panic this repository had reached the default branch
#                        that way, with the lint denied workspace-wide the whole
#                        time. A check that runs outside the compiler is the
#                        only kind a suppression cannot reach, which is the
#                        whole argument for putting determinism rules in `ci/`
#                        rather than in the lint table.
#
# Only code before the first `#[cfg(test)]` is scanned. Tests may assert, and
# may build values by hand that the public API refuses.
#
# The bare-field total is capped rather than forced to zero. The three that
# remain are the channels of an 8-bit colour, where the range of `u8` is
# exactly the range of valid values -- which is the property this guard exists
# to protect, arrived at from the other direction.
set -eu

budget_file="${1:-ci/type-safety-budget.txt}"
status=0

pure=""
for d in crates/*/; do
    case "$d" in
        */synthvid-cli/ | */synthvid-validate/) ;;
        *) pure="$pure $d" ;;
    esac
done

scan() {
    # $1 = extended regex, applied only to code before the first #[cfg(test)].
    #
    # grep does the matching and awk only compares line numbers. Passing the
    # pattern into awk instead looked tidier and silently did nothing: BSD awk
    # rejects the alternation these rules need, printed to stderr, and left the
    # guard reporting success with three of its four rules dead. A guard is
    # worth exactly what its negative control proves, so keep the matching in
    # the tool that has a portable regex engine.
    for d in $pure; do
        find "$d" -name '*.rs' -type f | sort | while read -r f; do
            cut=$(grep -n '#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
            grep -nE "$1" "$f" 2>/dev/null \
                | awk -F: -v c="${cut:-2147483647}" -v f="$f" \
                    '$1 + 0 < c + 0 { sub(/^[0-9]*:/, ""); print f ":" $0 }'
        done
    done
}

raw=$(scan '^[[:space:]]*pub fn [A-Za-z0-9_]*_raw[[:space:]]*\(' || true)
if [ -n "$raw" ]; then
    echo "check-type-safety: a raw-value escape hatch is public" >&2
    printf '%s\n' "$raw" | sed 's/^/  /' >&2
    echo "  Make it pub(crate). While it is public, nothing else here holds." >&2
    status=1
fi

strs=$(scan '^[[:space:]]*pub (const )?fn .*:[[:space:]]*String\b' || true)
if [ -n "$strs" ]; then
    echo "check-type-safety: a public function takes a bare String" >&2
    printf '%s\n' "$strs" | sed 's/^/  /' >&2
    echo "  Use a validated newtype or an enum over the values that are legal." >&2
    status=1
fi

aborts=$(scan '\b(assert|assert_eq|assert_ne|panic|unreachable|todo|unimplemented)![[:space:]]*\(|\.(unwrap|expect)[[:space:]]*\(' || true)
if [ -n "$aborts" ]; then
    echo "check-type-safety: a library crate can abort at runtime" >&2
    printf '%s\n' "$aborts" | sed 's/^/  /' >&2
    echo "  Make the state unrepresentable, or return an error." >&2
    status=1
fi

fields=$(scan '^[[:space:]]*pub [a-z_]+:[[:space:]]*(u8|u16|u32|u64|usize|i8|i16|i32|i64|isize|f32|f64|bool|String|&str)[[:space:]]*,?[[:space:]]*$' || true)
count=$(printf '%s' "$fields" | grep -c . || true)

if [ ! -f "$budget_file" ]; then
    echo "check-type-safety: $budget_file not found" >&2
    exit 1
fi
budget=$(tr -dc '0-9' < "$budget_file")
if [ -z "$budget" ]; then
    echo "check-type-safety: $budget_file must contain a number" >&2
    exit 1
fi

if [ "$count" -gt "$budget" ]; then
    echo "check-type-safety: $count bare public fields exceeds the budget of $budget" >&2
    printf '%s\n' "$fields" | sed 's/^/  /' >&2
    echo "  Wrap it in a type whose constructor refuses the invalid values," >&2
    echo "  rather than raising the budget. The budget ratchets down." >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-type-safety: ok ($count of $budget bare fields)"
