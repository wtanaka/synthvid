#!/bin/sh
# Verify the frozen test vectors are unchanged.
#
# The hard-coded literals in the tests for the pseudo-random generator and the
# trigonometry module define the output of those modules exactly. If a change
# makes them fail, the change is wrong -- regenerating them to match would
# quietly void every digest in catalog.lock and every corpus generated so far.
#
# This guard cannot make that impossible; one commit could edit the vectors and
# this digest file together. It makes it impossible to MISS, which is the
# achievable goal: a diff touching ci/frozen-vectors.sha256 is always a change
# to defined behaviour and must be reviewed as one.
set -eu

digests="${1:-ci/frozen-vectors.sha256}"

if [ ! -f "$digests" ]; then
    echo "check-frozen-vectors: $digests not found" >&2
    echo "check-frozen-vectors: record it with sha256sum once the frozen" >&2
    echo "  test vectors exist" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$digests"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$digests"
else
    echo "check-frozen-vectors: no sha256 tool available" >&2
    exit 1
fi

echo "check-frozen-vectors: ok"
