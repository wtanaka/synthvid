#!/bin/sh
# Keep source files small enough to read in one sitting, and keep each crate
# root a façade.
#
# A file that grows without bound stops being a module and becomes a dumping
# ground: the next change is appended to the end rather than placed where it
# belongs. This guard makes that growth fail the build instead of passing
# review unnoticed. When a module reaches the limit, split it by concern and
# declare the new module in the crate root -- do not raise the limit.
#
# The crate root is held to a stricter rule: it may declare modules and
# re-export their items, and nothing else. Code placed there has, by
# definition, no owning concern.
set -eu

limit=700
status=0

for file in $(find crates -name '*.rs' -not -path '*/target/*' | sort); do
    lines=$(wc -l <"$file" | tr -d ' ')
    if [ "$lines" -gt "$limit" ]; then
        echo "check-module-size: $file is $lines lines, over the $limit line limit" >&2
        echo "  split it by concern into a new module instead of raising the limit" >&2
        status=1
    fi
done

for root in crates/*/src/lib.rs crates/*/src/main.rs; do
    [ -f "$root" ] || continue
    offenders=$(grep -n '^[a-z#]' "$root" \
        | grep -v '^[0-9]*:\(pub \)\?\(pub(crate) \)\?mod ' \
        | grep -v '^[0-9]*:pub use ' \
        | grep -v '^[0-9]*:#!\[' \
        | grep -v '^[0-9]*:#\[cfg(test)\]' \
        | grep -v '^[0-9]*:fn main()' \
        || true)
    if [ -n "$offenders" ]; then
        echo "check-module-size: $root must contain only module declarations and re-exports" >&2
        echo "$offenders" | sed 's/^/  /' >&2
        status=1
    fi
done

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-module-size: ok"
