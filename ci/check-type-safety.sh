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
# remain are the channels of an 8-bit colour: `pub r: u8`, `pub g: u8`, `pub b: u8`
# in Rgb8 (crates/synthvid-scene/src/color.rs). The range of `u8` is exactly
# the range of valid values for an 8-bit colour channel -- every `u8` is legal.
# Wrapping them in a newtype would forbid nothing. The budget of 3 is therefore
# a floor rather than a debt to be paid down.
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

# Aborting and substituting are different defects and are counted apart.
# `unwrap`/`expect`/`panic` stop the process, which is loud; there are none and
# there should stay none. `unwrap_or` and friends keep going with a value
# nobody asked for, which is silent which agents tend to use because
# the lint table denies `arithmetic_side_effects` and the shortest
# route from a forced `checked_*` is to discard the `None`.
aborts=$(scan '\b(assert|assert_eq|assert_ne|panic|unreachable|todo|unimplemented)![[:space:]]*\(|\.(unwrap|expect)[[:space:]]*\(' || true)
if [ -n "$aborts" ]; then
    echo "check-type-safety: a library crate can abort at runtime" >&2
    printf '%s\n' "$aborts" | sed 's/^/  /' >&2
    echo "  Make the state unrepresentable, or return an error." >&2
    status=1
fi

fallback_budget_file="ci/silent-fallback-budget.txt"
if [ ! -f "$fallback_budget_file" ]; then
    echo "check-type-safety: $fallback_budget_file not found" >&2
    exit 1
fi
fallback_budget=$(tr -dc '0-9' < "$fallback_budget_file")
if [ -z "$fallback_budget" ]; then
    echo "check-type-safety: $fallback_budget_file must contain a number" >&2
    exit 1
fi

fallbacks=$(scan '\.(unwrap_or|unwrap_or_default|unwrap_or_else)[[:space:]]*\(' || true)
fallback_count=$(printf '%s' "$fallbacks" | grep -c . || true)

if [ "$fallback_count" -gt "$fallback_budget" ]; then
    echo "check-type-safety: $fallback_count silent fallbacks exceeds the budget of $fallback_budget" >&2
    printf '%s\n' "$fallbacks" | sed 's/^/  /' >&2
    echo "  A checked_* returning None is information; discarding it to carry on" >&2
    echo "  with a substituted value is the defect. Propagate the None, or make" >&2
    echo "  the failure unrepresentable. Never raise this budget." >&2
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
        cut=$(grep -n '#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
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
        cut=$(grep -n '#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
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

# Detect functions with adjacent parameters of the same bare primitive type.
#
# A function taking two or more adjacent parameters of the same
# primitive type invites silent transposition:
# write_sof_component(out, h_factor: u8, v_factor: u8) compiles with
# the arguments swapped and produces a valid file with a wrong image.
transposable_params_budget_file="ci/transposable-params-budget.txt"

if [ ! -f "$transposable_params_budget_file" ]; then
    echo "check-type-safety: $transposable_params_budget_file not found" >&2
    exit 1
fi
transposable_budget=$(tr -dc '0-9' < "$transposable_params_budget_file")
if [ -z "$transposable_budget" ]; then
    echo "check-type-safety: $transposable_params_budget_file must contain a number" >&2
    exit 1
fi

# Find functions with adjacent same-type parameters.
# We search for each primitive type T in the form "name: T", appearing twice
# adjacently within a function signature.
#
# To avoid shell syntax issues, we grep for each type in separate passes,
# then combine and deduplicate the results.
transposable=$(
    for d in $pure; do
        find "$d" -name '*.rs' -type f | sort | while read -r f; do
            cut=$(grep -n '#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
            # Grep for each primitive type pattern separately to avoid variable expansion issues.
            # Each pattern matches: fn ... (... : TYPE ... , ... : TYPE ...)
            (
                grep -n "fn.*([^)]*: u8[^)]*,[^)]*: u8" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: u16[^)]*,[^)]*: u16" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: u32[^)]*,[^)]*: u32" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: u64[^)]*,[^)]*: u64" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: usize[^)]*,[^)]*: usize" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: i8[^)]*,[^)]*: i8" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: i16[^)]*,[^)]*: i16" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: i32[^)]*,[^)]*: i32" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: i64[^)]*,[^)]*: i64" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: isize[^)]*,[^)]*: isize" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: f32[^)]*,[^)]*: f32" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: f64[^)]*,[^)]*: f64" "$f" 2>/dev/null || true
                grep -n "fn.*([^)]*: bool[^)]*,[^)]*: bool" "$f" 2>/dev/null || true
            ) | awk -F: -v c="${cut:-2147483647}" -v f="$f" \
                '$1 + 0 < c + 0 { print f ":" $1 }'
        done
    done | sort -u
)

transposable_count=$(printf '%s' "$transposable" | grep -c . || true)

if [ "$transposable_count" -gt "$transposable_budget" ]; then
    echo "check-type-safety: $transposable_count functions with adjacent same-type parameters exceeds the budget of $transposable_budget" >&2
    printf '%s\n' "$transposable" | sed 's/^/  /' >&2
    echo "  Consider using a newtype to bundle related parameters. The budget" >&2
    echo "  includes legitimate constructors and ratchets down over time." >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-type-safety: ok ($count of $budget bare fields, $fallback_count of $fallback_budget silent fallbacks, $transposable_count of $transposable_budget transposable-param functions, $conflated_count of $conflated_budget failures-as-absences, $discarded_count of $discarded_budget discarded errors)"
