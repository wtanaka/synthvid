#!/bin/sh
# Keep failures from being disguised in the pure crates.
#
# `unrepresentable` (configured in .unrepresentable/) now enforces the shapes
# this guard used to grep for: public `*_raw` escape hatches, primitive
# parameters and fields, aborts, silent fallbacks, modular arithmetic,
# transposable parameters and `Option` returns without `#[must_use]`. What is
# left here are the patterns it does not see, each of which once let a
# failure pass as an ordinary value:
#
#   an arithmetic suppression whose reason names no compile-time proof,
#   an overflow renamed as some other error,
#   an error destructured away and replaced by a value,
#   a failure reported as an absence,
#   a failure that can be dropped without the compiler objecting.
#
# Only code before the first `#[cfg(test)]` is scanned.
set -eu

status=0

pure=""
for d in crates/*/; do
    case "$d" in
        */synthvid-cli/ | */synthvid-validate/ | */synthvid-testkit/) ;;
        *) pure="$pure $d" ;;
    esac
done

# The line where a file's test-only code begins. A file that is all test
# code -- a `tests.rs` or `*_tests.rs` declared as `#[cfg(test)] mod`, or an
# integration test under `tests/` -- starts at line 1, so nothing in it is
# scanned; any other file starts at its first `#[cfg(test)]`, or not at all.
test_start() {
    case "$1" in
        */tests.rs | *_tests.rs | */tests/*) echo 1 ;;
        *) grep -n '#\[cfg(test)\]' "$1" | head -1 | cut -d: -f1 ;;
    esac
}

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
            cut=$(test_start "$f")
            grep -nE "$1" "$f" 2>/dev/null \
                | awk -F: -v c="${cut:-2147483647}" -v f="$f" \
                    '$1 + 0 < c + 0 { sub(/^[0-9]*:/, ""); print f ":" $0 }'
        done
    done
}

# An arithmetic suppression must point at a proof the compiler checks.
#
# `#[expect(clippy::arithmetic_side_effects, reason = "...")]` is the
# only way left to write arithmetic that is total by construction,
# because the lint is syntactic and no amount of widening satisfies
# it. That makes the reason string load-bearing, and a prose reason is
# exactly the thing that reads as convincing while being subtly wrong
# on a target nobody built for.
#
# So the reason may not stand alone. It must name at least one
# constant that exists in the same file, and that constant must carry
# the bound:
#
#     const _MAX_PIXEL_INDEX: u32 = 65_535 * 65_535 + 65_535;
#
# Arithmetic that overflows in a `const` is a compile error, evaluated
# for the target actually being built. So the constant is checked by
# the compiler, and this rule only has to check that the suppression
# points at one. If the bound is ever falsified the crate stops
# building rather than addressing the wrong byte.
#
# Held at zero. A suppression whose reason names no constant, or names
# one that does not exist, is refused.
unproved=""
for d in $pure; do
    found=$(find "$d" -name '*.rs' -type f | sort | while read -r f; do
        awk -v f="$f" '
            FNR == NR {
                if ($0 ~ /^[[:space:]]*const _[A-Z0-9_]+/) {
                    line = $0
                    sub(/^[[:space:]]*const /, "", line)
                    sub(/[^A-Za-z0-9_].*$/, "", line)
                    declared[line] = 1
                }
                next
            }
            /#\[expect\(/ { inblock = 1; blk = ""; start = FNR }
            inblock { blk = blk " " $0 }
            inblock && /\)\]/ {
                inblock = 0
                if (blk ~ /arithmetic_side_effects/) {
                    ok = 0
                    tmp = blk
                    while (match(tmp, /_[A-Z][A-Z0-9_]+/)) {
                        name = substr(tmp, RSTART, RLENGTH)
                        if (name in declared) { ok = 1 }
                        tmp = substr(tmp, RSTART + RLENGTH)
                    }
                    if (ok == 0) { print f ":" start ": arithmetic suppression names no constant declared in this file" }
                }
            }
        ' "$f" "$f"
    done)
    unproved="$unproved$found"
done
if [ -n "$unproved" ]; then
    echo "check-type-safety: an arithmetic suppression is not backed by a compile-time proof" >&2
    printf '%s\n' "$unproved" | sed 's/^/  /' >&2
    echo "  Add a const whose arithmetic overflows if the bound is false, and" >&2
    echo "  name it in the reason. A prose reason alone is not accepted." >&2
    status=1
fi

# An arithmetic failure renamed as something else.
#
# The arithmetic lint forces `checked_*`, which yields an `Option` the
# caller must discharge.  One cheap way is to keep the `checked_*` and
# hand its `None` to `ok_or(SomeOtherError)` which obscures an
# overflow as a domain specific error.
#
# Example:
#
#     // Bounds check guarantees arithmetic will not overflow
#     let pixel_index = u64::from(coord.y)
#         .checked_mul(u64::from(width))
#         .ok_or(PixelOutOfBounds)?;
#
# If the bound is guaranteed, the `checked_*` is dead and the widening
# should be stated in the types -- `u32` holds `65535 * 65535 +
# 65535`, so nothing can fail. If the bound is not guaranteed, the
# error must say `Overflow`.
#
# So the rule is narrow: a `checked_*` whose `None` is converted by
# `ok_or` or `ok_or_else` into anything that is not `Overflow`.
# Discharging into `Overflow` is not counted.
mislabelled_budget_file="ci/mislabelled-failure-budget.txt"
if [ ! -f "$mislabelled_budget_file" ]; then
    echo "check-type-safety: $mislabelled_budget_file not found" >&2
    exit 1
fi
mislabelled_budget=$(tr -dc '0-9' < "$mislabelled_budget_file")
if [ -z "$mislabelled_budget" ]; then
    echo "check-type-safety: $mislabelled_budget_file must contain a number" >&2
    exit 1
fi

mislabelled=""
for d in $pure; do
    found=$(find "$d" -name '*.rs' -type f | sort | while read -r f; do
        cut=$(test_start "$f")
        awk -v c="${cut:-2147483647}" -v f="$f" '
            NR + 0 >= c + 0 { exit }
            # A statement never spans two functions; without this reset a
            # `checked_*` in one function pairs with an `ok_or` in the next
            # when the first ends in a tail expression with no `;`.
            /^[[:space:]]*(pub(\([a-z()]+\))? )?(const )?fn / { buf = "" }
            { buf = buf " " $0 }
            /ok_or/ {
                if (buf ~ /checked_[a-z_]+\(/ && $0 !~ /Overflow/) {
                    sub(/^[[:space:]]+/, "")
                    print f ":" NR ": " $0
                }
            }
            /;/ { buf = "" }
        ' "$f"
    done)
    mislabelled="$mislabelled$found"
done
mislabelled_count=$(printf '%s' "$mislabelled" | grep -c . || true)

if [ "$mislabelled_count" -gt "$mislabelled_budget" ]; then
    echo "check-type-safety: $mislabelled_count arithmetic failures renamed exceeds the budget of $mislabelled_budget" >&2
    printf '%s\n' "$mislabelled" | sed 's/^/  /' >&2
    echo "  An overflow is an Overflow. If it cannot happen, widen the type and" >&2
    echo "  delete the check; if it can, do not give it another failure's name." >&2
    status=1
fi

# An error destructured away and replaced by a value.
#
# `let Ok(v) = fallible() else { ... }` binds only the success case. The `Err`
# is not merely ignored, it is unnameable inside the `else` -- the binding
# discards it before the block runs. What the block then returns decides
# whether this is sound:
#
#   return Err(...)    Fine. The error is being translated into this layer's
#                      own error type, which is what a layer boundary is for.
#
#   return None,       Not fine. A failure has become a value. The caller sees
#   return false,      an absence, or a plain `false`, and cannot tell it from
#   return 0, ...      the ordinary case. The coverage predicates did exactly
#                      this: an overflow inside `disc_covers` returned `false`,
#                      meaning "this pixel is not covered", so a frame was
#                      painted with pixels missing and nothing reported. That
#                      is the same defect `.ok()?` produces, spelled so that
#                      the rule against `.ok()?` does not see it -- which is
#                      how it got written.
#
# So the test is not the `let ... else` itself but what its block returns.
# Anything other than an `Err` is counted.
discarded_budget_file="ci/discarded-error-budget.txt"
if [ ! -f "$discarded_budget_file" ]; then
    echo "check-type-safety: $discarded_budget_file not found" >&2
    exit 1
fi
discarded_budget=$(tr -dc '0-9' < "$discarded_budget_file")
if [ -z "$discarded_budget" ]; then
    echo "check-type-safety: $discarded_budget_file must contain a number" >&2
    exit 1
fi

discarded=""
for d in $pure; do
    found=$(find "$d" -name '*.rs' -type f | sort | while read -r f; do
        cut=$(test_start "$f")
        awk -v c="${cut:-2147483647}" -v f="$f" '
            NR + 0 >= c + 0 { exit }
            /let Ok\(/ {
                # The whole construct may sit on one line; judge it immediately
                # rather than looking ahead, or a single-line spelling escapes.
                if ($0 ~ /return/) {
                    if ($0 !~ /return Err\(/) { print f ":" NR ": " $0 }
                    pending = 0
                } else {
                    pending = NR; sig = $0
                }
                next
            }
            pending > 0 && NR <= pending + 3 && /return/ {
                if ($0 !~ /return Err\(/) { print f ":" pending ": " sig }
                pending = 0
            }
        ' "$f"
    done)
    discarded="$discarded$found"
done
discarded_count=$(printf '%s' "$discarded" | grep -c . || true)

if [ "$discarded_count" -gt "$discarded_budget" ]; then
    echo "check-type-safety: $discarded_count errors discarded for a value exceeds the budget of $discarded_budget" >&2
    printf '%s\n' "$discarded" | sed 's/^/  /' >&2
    echo "  Return an error of this layer's own, or propagate with ?. A failure" >&2
    echo "  must not become None, false, or a default." >&2
    status=1
fi

# A failure reported as an absence.
#
# `None` means "there is no value". An arithmetic overflow in this
# crate means "the computation this crate exists to do could not be
# done". When one function reports both through `None`, the caller
# cannot tell them apart, and the overwhelmingly common handling --
# treat `None` as the empty case and carry on -- silently turns a
# failure into a wrong answer. This is not hypothetical:
# `clipped_pixel_range` collapsed four outcomes into `None`, and a
# frame was emitted with a shape missing rather than an error
# reported.
#
# Two detectors, because the mistake has two shapes:
#
#   .ok()?             A `Result` narrowed back into an `Option`. The error was
#                      named, and this throws the name away to make an absence.
#                      Information flowing backwards through the layer that
#                      just produced it. Use `?` and return `Result`, or
#                      `map_err` to a named error of your own.
#
#   fn -> Option with  A function that returns `None` both for a domain
#   a failure path     condition ("radius is not positive, nothing to draw")
#                      and for an arithmetic failure. `disc_extent` documented
#                      exactly this: "Returns None for a non-positive radius,
#                      which draws nothing, or on overflow." Separate them --
#                      return `Result`, and express emptiness in the success
#                      type the way `PixelSpan::Empty` does.
#
# The total is budgeted rather than zero so the tree stays green while
# the remaining ones are worked down. It ratchets and is never raised.
conflated_budget_file="ci/conflated-failure-budget.txt"
if [ ! -f "$conflated_budget_file" ]; then
    echo "check-type-safety: $conflated_budget_file not found" >&2
    exit 1
fi
conflated_budget=$(tr -dc '0-9' < "$conflated_budget_file")
if [ -z "$conflated_budget" ]; then
    echo "check-type-safety: $conflated_budget_file must contain a number" >&2
    exit 1
fi

narrowed=$(scan '\.ok\(\)\?' || true)
narrowed_count=$(printf '%s' "$narrowed" | grep -c . || true)

mixed=""
for d in $pure; do
    found=$(find "$d" -name '*.rs' -type f | sort | while read -r f; do
        cut=$(test_start "$f")
        awk -v c="${cut:-2147483647}" -v f="$f" '
            NR + 0 >= c + 0 { exit }
            /^[[:space:]]*(pub(\([a-z()]+\))? )?(const )?fn / {
                if (sig != "" && none > 0 && fail > 0) { print f ":" start ": " sig }
                sig = $0; start = NR; none = 0; fail = 0
                if ($0 !~ /Option</) { sig = "" }
            }
            /return None;/ { none = 1 }
            /\.ok\(\)\?/ { fail = 1 }
            /checked_[a-z_]*\(.*\)\?/ { fail = 1 }
            /\.ok_or\(/ { fail = 1 }
            END { if (sig != "" && none > 0 && fail > 0) { print f ":" start ": " sig } }
        ' "$f"
    done)
    mixed="$mixed$found"
done
mixed_count=$(printf '%s' "$mixed" | grep -c . || true)
conflated_count=$((narrowed_count + mixed_count))

if [ "$conflated_count" -gt "$conflated_budget" ]; then
    echo "check-type-safety: $conflated_count failures reported as absences exceeds the budget of $conflated_budget" >&2
    if [ -n "$narrowed" ]; then
        echo "  a Result narrowed into an Option:" >&2
        printf '%s\n' "$narrowed" | sed 's/^/    /' >&2
    fi
    if [ -n "$mixed" ]; then
        echo "  returns None for both an empty case and a failure:" >&2
        printf '%s\n' "$mixed" | sed 's/^/    /' >&2
    fi
    echo "  Return Result and express the empty case in the success type." >&2
    status=1
fi

# A failure that can be dropped without the compiler objecting.
#
#   pub fn -> Option<()>   `Result` is `#[must_use]`; `Option` is not. A public
#                          function returning `Option<()>` is using it purely as
#                          a success/failure signal, and a caller that writes
#                          `draw(..);` discards that signal and compiles clean.
#                          Five drawing functions were converted from `()` to
#                          `Option<()>` and every existing caller kept compiling
#                          with the failure dropped -- the mistake only became
#                          visible once they returned `Result`.
#
#   let _ =                Discards a result explicitly. `unrepresentable`
#                          refuses this as discarded-value too; library code
#                          had six, each one a pixel write whose failure was
#                          thrown away.
#
#   is_some_and,           Turns a failure into a decision. `pixel_centre(x, y)
#   unwrap_or(true/false)  .is_some_and(|p| covers(p))` reads a `None` as "this
#                          pixel is not covered", so an overflow painted a frame
#                          with the shape missing rather than reporting anything.
#
# All three are held at zero. None of them is a judgement call: each has a
# mechanical replacement -- return `Result`, propagate with `?`, or match the
# failure explicitly.
droppable=$(scan 'pub (const )?fn [^;]*-> Option<\(\)>|let _ = |\.is_some_and\(|\.unwrap_or\((true|false)\)' || true)
if [ -n "$droppable" ]; then
    echo "check-type-safety: a failure here can be dropped silently" >&2
    printf '%s\n' "$droppable" | sed 's/^/  /' >&2
    echo "  Return Result rather than Option<()>, propagate with ?, or match the" >&2
    echo "  failure. Never let it become a default." >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-type-safety: ok ($conflated_count of $conflated_budget failures-as-absences, $discarded_count of $discarded_budget discarded errors, $mislabelled_count of $mislabelled_budget renamed failures)"
