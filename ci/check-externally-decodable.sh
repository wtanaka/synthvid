#!/bin/sh
# Confirm every well-formed file in the corpus can be read by a decoder that
# did not write it. An external tool is permitted here because this verifies
# output; it is never used to produce output.
#
# Files carrying a deliberate defect are skipped: failing to decode is the
# point of those, and which failure a given decoder produces is not something
# this project specifies.
set -eu

corpus="${1:-./corpus}"

if [ ! -d "$corpus" ]; then
    echo "check-externally-decodable: $corpus not found" >&2
    exit 1
fi

if ! command -v ffprobe >/dev/null 2>&1; then
    echo "check-externally-decodable: ffprobe not available, skipping" >&2
    exit 0
fi

checked=0
status=0

for f in "$corpus"/*.avi "$corpus"/*.mp4 "$corpus"/*.mov; do
    [ -e "$f" ] || continue
    case "$(basename "$f")" in
        *defect*) continue ;;
    esac
    if ! ffprobe -v error -show_entries stream=width,height,nb_frames \
            -of default=noprint_wrappers=1 "$f" >/dev/null 2>&1; then
        echo "check-externally-decodable: not decodable: $f" >&2
        status=1
    fi
    checked=$((checked + 1))
done

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "check-externally-decodable: ok ($checked files)"
