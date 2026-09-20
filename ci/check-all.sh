#!/bin/sh
# Run every guard. Reports all failures rather than stopping at the first.
#
# A guard that invokes cargo must not share the main target directory. Cargo
# decides whether to re-run a tool from recorded fingerprints, so a guard that
# reuses `target/` can report success on source it never read -- a green run
# that proves nothing, which is the one result this repository cannot absorb.
# `check-rustdoc.sh` and `check-clippy.sh` are the only guards that run
# cargo today; each builds in a directory of its own that it deletes first.
# A new guard that runs cargo does the same.
set -u

status=0
for check in \
    check-lints-intact.sh \
    check-no-external-deps.sh \
    check-forbidden-tokens.sh \
    check-suppressions.sh \
    check-catalog-generated.sh \
    check-clippy.sh \
    check-rustdoc.sh \
    check-module-size.sh
do
    if ! sh "ci/$check"; then
        status=1
    fi
done

# Only meaningful once the frozen test vectors exist.
if [ -f ci/frozen-vectors.sha256 ]; then
    if ! sh ci/check-frozen-vectors.sh; then
        status=1
    fi
fi

if [ "$status" -ne 0 ]; then
    echo "check-all: one or more guards failed" >&2
    exit 1
fi

echo "check-all: ok"
