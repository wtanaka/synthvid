# synthvid

## 1. Introduction

synthvid generates video files from declarative scene descriptions. The
same description produces byte-identical output on every platform and CPU
architecture. Every quantity in the generated frames is reported exactly
in an accompanying manifest.

## 2. Artifacts

For each catalog entry, `generate` writes two files to the output
directory:

- `<name>.json`: the manifest (Section 3).
- `<name>-media.mp4` or `<name>-media.avi`: the media file. ISO base
  media container (`.mp4`) or AVI container (`.avi`), per the entry's
  container axis.

`catalog.lock` records the expected SHA-256 digest and byte length of
every artifact. It contains one line per artifact:

```text
name SP hex-digest SP length LF
```

Lines are sorted by name in ascending byte order. The last line is
LF-terminated.

## 3. Manifest format

A manifest is a JSON document with the following members:

| Member           | Type     | Definition                                              |
| ---------------- | -------- | ------------------------------------------------------- |
| `background`     | string   | one of `solid`, `checker`, `gradient`, `blobs`, `grid`  |
| `dimensions`     | object   | `{"height":H,"width":W}`, frame size in pixels          |
| `frame_count`    | integer  | number of frames                                        |
| `frame_rate`     | rational | playback rate                                           |
| `frames`         | array    | one element per frame, in index order                   |
| `name`           | string   | catalog entry name                                      |
| `objects`        | array    | invariant object declarations                           |
| `scale`          | rational | scene units per world unit; absent when undeclared      |
| `schema_version` | integer  | `1`                                                     |

Each element of `frames` contains `index` (integer), `camera` (object),
and `objects` (array). `camera` contains `a`, `b`, `c`, `d`, `tx`, `ty`,
each a rational; it maps scene space to screen space. Each element of
`frames[].objects` contains `bbox` (`min_x`, `min_y`, `max_x`, `max_y`,
all rationals), `centre_screen` (point), `centre_world` (point; present
only when `scale` is present), `index` (position in scene declaration
order), and `on_screen` (rational in `[0, 1]`). An object appears in a
frame's `objects` only when its `visible` span contains that frame and its
extent is non-empty.

Each element of `objects` contains `index` (integer), `shape` (one of
`disc`, `rect`, `polygon`, `cross`), and `visible` (`{"end":E,"start":S}`).
The span is half-open: `E` is one past the last visible frame.

A point is `{"x":R,"y":R}`. A rational `R` is
`{"den":D,"num":N}` with `D > 0` and `gcd(|N|, D) = 1`; `den` precedes
`num`.

Serialization conforms to OLPC Canonical JSON: UTF-8, no whitespace
outside string literals, object keys sorted by byte value, no
floating-point numbers. The output is valid JSON per RFC 8259.

Example:

```json
{"frame_rate":{"den":1,"num":30}}
```

## 4. Catalog

The catalog is a pairwise covering array over the axes below: every
pair of values from any two axes appears together in at least one entry.

| Axis         | Values                                              |
| ------------ | --------------------------------------------------- |
| dimensions   | 16x16, 320x240, 640x480, 1280x720, 1920x1080        |
| frame count  | 1, 10, 100, 1000, 20000                             |
| frame rate   | 30, 60, 120, 240, NTSC (30000/1001), absurd (8000/1) |
| background   | solid, checker, gradient, blobs, grid               |
| shape        | disc, rect, polygon, cross                          |
| motion       | fixed, linear, ballistic, circular, oscillating, walk |
| coding       | raw, mjpeg                                          |
| container    | iso (`.mp4`), avi (`.avi`)                          |
| track matrix | identity, rot90, rot180, rot270                     |
| scale        | scaled (5/2 scene units per world unit), unscaled   |
| defect       | nodef, plus one single fault per entry below        |

`raw` is uncompressed RGB; `mjpeg` is motion-JPEG.

The defect axis contains the no-defect case (`nodef`) and fourteen single
faults: `trunc-at` (file truncated mid-stream), `trunc-box` (box
truncated), `zero-pay` (zero-length payload), `over-len` / `under-len`
(overstated / understated box length), `off-end` (offset past the end),
`dup-box` (duplicated box), `unk-box` (unknown box), `zero-fps` (zero
frame rate), `zero-dim` (zero dimension), `absurd-fps` (absurd frame
rate), `non-mono` (non-monotonic timestamps), `count-mis` (declared frame
count disagreeing with the payload), `reorder` (reordered boxes).

Constraints: faults addressing ISO boxes combine only with the ISO
container. Raw entries whose frame data would exceed a classic
container's 32-bit size limit are not generated.

Entry names are derived from the axis values, never hand-written:
identical axes yield identical names; any axis difference changes the
name. The media extension follows the container.

## 5. Operation

The workspace has zero dependencies; all commands below work offline.

```sh
cargo build --release -p synthvid-cli
cargo run --release -p synthvid-cli -- verify
```

`verify` with no `--corpus` regenerates the catalogue one entry at a
time, compares each manifest and media file against `catalog.lock`, and
discards the entry before generating the next. Nothing is written to
disk; peak memory is one entry's manifest and media bytes. To check a
corpus that is already on disk, generate it into a scratch directory
(it is several gigabytes), copy the committed `catalog.lock` into it,
and run `verify --corpus DIR`; `generate` does not write a lockfile.

- `generate --out DIR [--only NAME]... [--dry-run]`: write each entry's
  manifest and media file to `DIR`.
- `verify [--lock FILE] [--only NAME]...`: regenerate each entry in
  memory and compare its digests and lengths against `FILE` (default
  `catalog.lock` in the current directory); `--only` limits the check
  to the named entries and an unknown name is an error. Prints `ok NAME`
  per matching entry to stdout. Prints one line per difference and one
  line per entry that could not be checked to stderr. Prints a summary
  line to stdout with the count of entries compared and, if any could
  not be checked, that count. Lockfile lines the catalogue does not
  produce are reported as missing: all of them without `--only`, and
  those of the named entries with it. A lockfile that lists a name twice
  is rejected. Exit `0` if every entry matches and every entry could be
  checked; nonzero otherwise, including when the lockfile cannot be read.
- `verify --corpus DIR`: compare every file in `DIR` against
  `DIR/catalog.lock`. Exit `0` if all digests and lengths match; nonzero
  otherwise, printing one line per difference (missing, extra, invalid
  name, digest or length mismatch).
- `list [--format text|json]`: print all entry names.
- `manifest --name NAME`: print one entry's manifest to stdout.
- `lock --corpus DIR [--write] [--dry-run]`: print the lockfile text the
  current code produces; with `--write`, write it to `DIR/catalog.lock`.

## 6. Determinism

Guarantee: the same catalog entry yields byte-identical manifest and
media files on every platform and CPU architecture.

The guarantee holds by construction:

1. Authored quantities are exact rationals (`i64` over nonzero `i64`),
   checked and normalized. 30000/1001 is not representable as `f64`, and
   float formatting varies by platform and locale; no float appears in
   any serialized manifest.
2. Trigonometry is the in-crate `sin_turns`/`cos_turns`. The standard
   library `sin`, `cos`, and related functions dispatch to the platform
   math library and are not bit-identical between platforms. No
   floating-point type appears in the pure crates at all: every quantity
   is an integer or an exact rational, and CI rejects `f32` and `f64`
   there.
3. Collection iteration order is fixed: `BTreeMap`, `BTreeSet`, or sorted
   vectors. `HashMap`, `HashSet`, and `RandomState` are prohibited; their
   iteration order is randomized per process.
4. Motion is closed-form in the frame number. A value at frame N is a
   function of N, never an accumulation over frames 0..N; evaluation
   order (forward, backward, shuffled) does not affect results.
5. Accumulation order is fixed: work is partitioned by index and combined
   in index order, never in thread-completion order.
6. Library crates take no ambient input: no clocks, environment variables,
   process IDs, pointer addresses, system randomness, network, or
   filesystem access. Randomness comes only from the in-crate PRNG seeded
   from the scene description. Only `synthvid-cli` touches the
   filesystem.
7. All algorithms are implemented in-crate (PRNG, SHA-256, canonical JSON
   writer, JPEG encoder, both containers) with zero dependencies. A
   specification constrains an interface, not an algorithm: a dependency
   update that fixes a bug is indistinguishable from a regression, as
   both invalidate every recorded digest.
8. Failure is an error, never a substitution. Overflow, a failed
   transform, or an oversized payload returns an error naming what
   failed; no default, skip, or clamp is substituted. `verify` therefore
   confirms reproduction of the correct output, not identical
   reproduction of an incorrect one.

## 7. Layout

```text
crates/synthvid-scene/    numbers, geometry, motion, rendering (pure)
crates/synthvid-encode/   pixel coding and containers (pure)
crates/synthvid-catalog/  declared corpus, manifests, digests (pure)
crates/synthvid-cli/      the only crate that touches the filesystem
crates/synthvid-validate/ external-tool oracles used by tests only
```

## 8. License

MIT or Apache-2.0, at your option.
