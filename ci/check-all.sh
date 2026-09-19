#!/bin/sh
# Run every guard. Reports all failures rather than stopping at the first.
set -u

status=0
for check in \
    check-lints-intact.sh \
    check-no-external-deps.sh \
    check-forbidden-tokens.sh \
    check-suppressions.sh \
    check-catalog-generated.sh
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
