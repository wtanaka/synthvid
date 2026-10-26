# synthvid

## 1. Introduction

synthvid generates video files from declarative scene descriptions. The same
description produces byte-identical output on every platform and CPU
architecture. Every quantity in the generated frames is reported exactly in an
accompanying manifest.

### Pre-release status

synthvid is pre-release (version `0.0.x`). Nothing here is a stable interface
until 1.0, and any release may change any of it without notice:

- the manifest format, including its keys and `schema_version`;
- the catalog's contents and the names of its entries;
- the `catalog.lock` format and every digest in it;
- the bytes a given description renders to, since fixing an encoder bug changes
  them;
- the public Rust API, and how a defect is addressed.

The determinism guarantee in Section 6 holds within one release: the same
version produces the same bytes everywhere. It does not hold between versions.
To reproduce a corpus, pin the exact version and its `catalog.lock`.

## 2. Artifacts

For each catalog entry, `generate` writes two files to the output directory:

- `<name>.json`: the manifest (Section 3).
- `<name>-media.mp4` or `<name>-media.avi`: the media file. ISO base media
  container (`.mp4`) or AVI container (`.avi`), per the entry's container axis.

`catalog.lock` records the expected SHA-256 digest and byte length of every
artifact. It contains one line per artifact:

```text
name SP hex-digest SP length LF
```

Lines are sorted by name in ascending byte order. The last line is
LF-terminated.

## 3. Manifest format

A manifest is a JSON document with the following members:

| Member           | Type     | Definition                                             |
| ---------------- | -------- | ------------------------------------------------------ |
| `background`     | string   | one of `solid`, `checker`, `gradient`, `blobs`, `grid` |
| `dimensions`     | object   | `{"height":H,"width":W}`, frame size in pixels         |
| `frame_count`    | integer  | number of frames                                       |
| `frame_rate`     | rational | playback rate                                          |
| `frames`         | array    | one element per frame, in index order                  |
| `name`           | string   | catalog entry name                                     |
| `objects`        | array    | invariant object declarations                          |
| `scale`          | rational | scene units per world unit; absent when undeclared     |
| `schema_version` | integer  | `1`                                                    |

Each element of `frames` contains `index` (integer), `camera` (object), and
`objects` (array). `camera` contains `a`, `b`, `c`, `d`, `tx`, `ty`, each a
rational; it maps scene space to screen space. Each element of
`frames[].objects` contains `bbox` (`min_x`, `min_y`, `max_x`, `max_y`, all
rationals), `centre_screen` (point), `centre_world` (point; present only when
`scale` is present), `index` (position in scene declaration order), and
`on_screen` (rational in `[0, 1]`). An object appears in a frame's `objects`
only when its `visible` span contains that frame and its extent is non-empty.

Each element of `objects` contains `index` (integer), `shape` (one of `disc`,
`rect`, `polygon`, `cross`), and `visible` (`{"end":E,"start":S}`). The span is
half-open: `E` is one past the last visible frame.

A point is `{"x":R,"y":R}`. A rational `R` is `{"den":D,"num":N}` with `D > 0`
and `gcd(|N|, D) = 1`; `den` precedes `num`.

Serialization conforms to OLPC Canonical JSON: UTF-8, no whitespace outside
string literals, object keys sorted by byte value, no floating-point numbers.
The output is valid JSON per RFC 8259.

Example:

```json
{ "frame_rate": { "den": 1, "num": 30 } }
```

A flat manifest may also carry the optional members `medium` and `optics`.
Each is written only when it is not ideal. Both belong to schema 1, so a flat
manifest with either member still has `"schema_version":1`.

`medium` is an object whose `kind` is `sensor` or `film`. Seeds are written as
strings of decimal digits. A sensor writes:

| Member          | Type    | Definition                                  |
| --------------- | ------- | ------------------------------------------- |
| `column`        | integer | column offset spread, raw Q8.8 (256 is 1.0) |
| `kind`          | string  | `sensor`                                    |
| `pattern_seed`  | string  | fixed-pattern seed                          |
| `prnu`          | integer | per-sensel gain spread, parts per 65536     |
| `read`          | integer | read-noise deviation, raw Q8.8              |
| `shot`          | integer | shot-noise slope, raw Q8.8                  |
| `temporal_seed` | string  | temporal noise seed                         |

Shot noise is drawn from the light a sensel receives before any clamp to the code
range, so a sensel brighter than full scale is noisier than one at full scale.
The sensor saturates once, after the noise is added: the sum is rounded and then
clamped to `0` through `255`.

Film writes `blur`, `density`, `spread` (integers, raw Q8.8), `kind` (`film`),
`radius` (rational, a ratio of the frame height), and `seed` (string).

`optics` is written only for a vignette: `{"kind":"vignette","strength":N}`,
where `strength` is a raw Q8.8 integer.

### Deep shot manifest

A deep shot has `"schema_version":2` and records no bounding boxes or screen
positions. Its members, in canonical key order:

| Member           | Type     | Definition                                               |
| ---------------- | -------- | -------------------------------------------------------- |
| `backdrop`       | object   | `{"blue","green","red"}`, integers 0 to 255              |
| `dimensions`     | object   | `{"height":h,"width":w}`, output size in sensels         |
| `frame_count`    | integer  | number of frames                                         |
| `frame_rate`     | rational | playback rate                                            |
| `frames`         | array    | one element per frame, in index order                    |
| `medium`         | object   | as in a flat manifest; absent when the medium is `Off`   |
| `name`           | string   | catalog entry name                                       |
| `objects`        | array    | one declaration per layer, in stage order                |
| `optics`         | object   | the lens object; always written                          |
| `pixel_samples`  | integer  | samples along each axis of a sensel: 1, 2, 4, or 8       |
| `projection`     | string   | `perspective`                                            |
| `schema_version` | integer  | `2`                                                      |

Each element of `frames` contains `camera` (`roll` rational, and integers `x`,
`y`, `z`), `index`, and `objects`. Each element of `frames[].objects` contains
`index`, `x`, `y`, and `z`, and lists only the layers whose visible span contains
the frame, including layers behind the camera.

Each element of `objects` contains `fill` (`{"blue","green","red"}`), `index`,
`shape`, and `visible` (`{"end":E,"start":S}`). Its shape members are:

- `disc`: `radius` (integer, world units).
- `rect`: `half_height` and `half_width` (integers).
- `polygon`: `vertices` (array of `{"x":X,"y":Y}`).

The `optics` object has `aperture`, `focal_length` (rational, in frame heights),
`kind` (`lens`), and `vignette`. An `aperture` is `{"kind":"pinhole"}`, or an iris
with `focus` (an integer distance or `"infinity"`), `frame_height`, `kind`
(`iris`), `samples` (2, 4, or 8), `shape` (`{"kind":"disc"}` or
`{"kind":"polygon","blades":B}`), and `stop` (`f1_4` through `f22`). A `vignette`
is `{"kind":"none"}` or `{"kind":"vignette","strength":N}`.

The `perspective` projection places a point at offset `(dx, dy)` from the camera
axis and depth `z` at `16 * h * fn * (X, Y) / z` in 1/256-sensel units from the
frame center. `h` is the output height in sensels, `fn` the focal length in
sixteenths of a frame height, and `(X, Y)` the offset `(dx, dy)` rotated by the
roll.

### Lens model

A lens is either a pinhole or an iris. An iris has an f-number `Nn/Nd`, an
outline, a focus distance `s`, a frame height `H` in world units, and an
aperture grid of `A` samples per axis, where `A` is 2, 4, or 8. The focal
length is `f = fn * H / 16` world units, where `fn` is the lens focal length in
sixteenths of a frame height.

The manifest names an f-number by its stop. Each stop is the exact ratio below,
which is the `Nn/Nd` of the blur formula; the decimal column is for reading
only.

| Stop   | f-number | Decimal |
| ------ | -------- | ------- |
| `f1_4` | 7/5      | 1.4     |
| `f2`   | 2        | 2       |
| `f2_8` | 14/5     | 2.8     |
| `f4`   | 4        | 4       |
| `f5_6` | 28/5     | 5.6     |
| `f8`   | 8        | 8       |
| `f11`  | 11       | 11      |
| `f16`  | 16       | 16      |
| `f22`  | 22       | 22      |

Focus. A focus `s` must lie at least eight focal lengths from the lens, that is
`2 * s >= fn * H`. A closer focus, `2 * s < fn * H`, is refused with an error.
A focus at infinity and a pinhole are always accepted.

Thin-lens approximation. A thin lens of focal length `f` focused at distance
`s` forms its image at distance `v = f * s / (s - f)`. The blur formula below
takes the image distance to be `f` instead. That is a first-order
approximation, valid when the focus distance is much larger than the focal
length. At the closest focus allowed, `s = 8 * f`, the image distance is
`8 * f / 7`, so the approximation is off by one-seventh of `f`, and the error
shrinks for every farther focus.

Lens shift. Looking through the aperture sample with odd numerators `g` across
and `k` down, a layer at depth `z` shifts in the image, along each axis, by the
following amounts in 1/256-sensel units, where `h` is the output frame height in
sensels:

- focused at `s`: `-H * fn^2 * h * Nd * g * (s - z) / (2 * Nn * A * z * s)`;
- focused at infinity: `-H * fn^2 * h * Nd * g / (2 * Nn * A * z)`.

Each shift is rounded once, half up, to 1/256-sensel units. The sample's unit
aperture coordinate is `g / A`, so `g` ranges over odd values with `|g| <= A - 1`.

Aperture samples. The candidates of a grid are the points `(g / A, k / A)` with
`g` and `k` odd and within `A - 1` of zero. The outline is a disc of radius 1 or
a regular polygon whose vertex `m` lies at the cosine and sine of `m / n` turns,
where `n` is the blade count. A candidate is kept when it and its point
reflection through the center both lie inside or on the outline. The kept set is
therefore centrally symmetric: its numerators sum to zero on each axis, so a
defocused layer does not shift sideways by the mean aperture coordinate.

A shape and grid with no kept candidate is refused with an error. A three-blade
outline on the two-by-two grid is one such case: no candidate and its reflection
both lie inside the triangle. A polygon with four or more blades always
keeps points. The candidates nearest the center lie at a distance of `sqrt(2) / A`,
at most 0.707, and the polygon contains the circle of radius `cos(pi / n)`,
which is at least 0.707 for `n` at least 4, so those candidates and their
reflections are inside or on the outline. A triangle, whose inradius is 1/2,
keeps points on the four-by-four and eight-by-eight grids, and is refused only
on the two-by-two grid.

Guard band. Lattice units are 1/256 sensel. A layer whose projected extent, or
whose lens shift, has a magnitude above 2^28 lattice units, which is 2^20
sensels, fails the render with an error. Such a layer is not clipped or culled,
so a frame is never silently wrong. A layer must be kept farther from the camera
or made smaller to render.

## 4. Catalog

The catalog is a pairwise covering array over the axes below: every pair of
values from any two axes appears together in at least one entry, except pairs
ruled out as impossible or too large. Pairs a container cannot carry (an AVI
with an ISO box defect) and a size beside a frame count that would exceed 10
million pixels in total (1920x1080 with 1000 frames, say) are omitted, which
keeps the largest entry near 30 MB.

| Axis         | Values                                                |
| ------------ | ----------------------------------------------------- |
| dimensions   | 16x16, 320x240, 640x480, 1280x720, 1920x1080          |
| frame count  | 1, 10, 100, 1000, 20000                               |
| frame rate   | 30, 60, 120, 240, NTSC (30000/1001), absurd (8000/1)  |
| background   | solid, checker, gradient, blobs, grid                 |
| shape        | disc, rect, polygon, cross                            |
| motion       | fixed, linear, ballistic, circular, oscillating, walk |
| coding       | raw, mjpeg                                            |
| container    | iso (`.mp4`), avi (`.avi`)                            |
| track matrix | identity, rot90, rot180, rot270                       |
| scale        | scaled (5/2 scene units per world unit), unscaled     |
| defect       | nodef, plus one single fault per entry below          |

`raw` is uncompressed RGB; `mjpeg` is motion-JPEG.

Nine further entries sit outside the covering array. They are small ISO files
that declare a capture rate beside a playback rate, in one of two boxes. Six
write it in the project box `moov/udta/cfps`; three write it in the
QuickTime-style `moov/meta` box. Each entry pairs a playback rate with a
capture rate:

- 30 played, 240 captured (`cfps`);
- 30000/1001 played, 1000 captured (`cfps`);
- 30000/1001 played, 24000/1001 captured (`cfps`);
- 30000/1001 played, 25 captured (`cfps`);
- 30 played, 12 captured (`cfps`);
- 30 played, 120000/1001 captured (`cfps`);
- 30 played, 240 captured (`meta`);
- 30000/1001 played, 24000/1001 captured (`meta`);
- 30 played, 12 captured (`meta`).

The capture rate is any positive rational whose reduced numerator and reduced
denominator each fit a `u32`. A rate with a term above `u32::MAX` is refused with
`CaptureRateTooLarge`. The name segment is `cap<n>` when
the reduced rate is an integer and `cap<n>x<d>` when it is a fraction, such as
`cap24000x1001`. It sits after the scale segment and before the defect segment.
An entry whose rate is in the `meta` box puts `qt` before `cap`, so its
segment is `qtcap<n>` or `qtcap<n>x<d>`: `qtcap240`, `qtcap24000x1001`,
`qtcap12`. The `qt` marks the carrier, so the two forms of one rate have
different names. The media is timed at the playback rate. The capture rate is
not an axis of the array, so every other entry carries none.

The `cfps` box is a project-specific box inside `moov/udta`, not a registered
ISO box, and standard tools ignore it. Its layout is version 0, three zero flag
bytes, then the reduced capture rate as numerator and denominator, each a
big-endian `u32`. Because `cfps` keeps the exact rational, it is the form that
carries a rate exactly, within the `u32` limit on each term.

The `meta` box is a QuickTime-style metadata box, which standard tools may
read. It is the last child of `moov`, after `trak`, and a file carries its
capture rate in `udta` or in `meta`, never both. Its layout, every box with
its size, is:

- `meta`: a full box, version 0 with three zero flag bytes, holding in order
  the three boxes below;
- `hdlr`: a full box, handler type `mdta`, the other fields zero, and a name of
  one zero byte (the null terminator of an empty string);
- `keys`: a full box with entry count 1, then one key entry of a u32 size, the
  namespace `mdta` and the key bytes `com.apple.quicktime.capture.fps`. The
  entry is not a box;
- `ilst`: one item box whose type is the big-endian key index 1, holding one
  `data` box with type indicator 23, locale 0, and the capture rate as a
  big-endian IEEE-754 float32.

The float32 loses exactness: 24000/1001 is 23.976023976..., and its nearest
float32 is 0x41BFCEE6, which is 23.976024627685547. A reader of `meta` gets
that approximation, while `cfps` keeps the exact reduced rational. The float32
is computed in integer arithmetic, rounded to nearest with ties to even, so it
does not depend on the platform's floating point. The container's own timing
(`mvhd`, `tkhd`, `mdhd`, `stts`) is always the playback rate in both forms.

Ten further entries record through a sensor or film medium instead of an ideal
one. Each is a 64 by 48 ISO file of ten frames at 30 frames per second, with a
gradient background, a disc, fixed motion and no defect, once in raw pixels and
once in motion-JPEG. Each name ends in `rec` and the preset's name. The five
presets are:

- `reclowiso`: sensor, shot 18, read 128, prnu 0, column 0, temporal seed 1001,
  pattern seed 2001;
- `rechighiso`: sensor, shot 128, read 640, prnu 0, column 0, temporal seed 1002,
  pattern seed 2002;
- `recbanded`: sensor, shot 18, read 128, prnu 655, column 384, temporal seed
  1003, pattern seed 2003;
- `recfinegrain`: film, radius 1/400, spread 0, density 256, blur 0, seed 3001;
- `reccoarsegrain`: film, radius 1/16, spread 77, density 256, blur 128, seed
  3002.

The sensor and film values are raw Q8_8 numbers, except `prnu`, which is in
parts per 65536, and the radius, which is a fraction of the frame height. The
pairwise covering array does not include these ten entries.

Four further entries are deep shots, one per depth effect: parallax, where layers
at different depths cross the frame at different speeds as the camera moves; roll,
where the camera turns about its axis; disc bokeh, where out-of-focus highlights
blur into discs; and hexagonal bokeh, where the same highlights blur into
hexagons. The pairwise covering array does not include these four entries. A deep shot
has its own stage, so its name has no background, shape or motion segment: its name is
the flat name without those three segments, ending in the depth segment.

The defect axis contains the no-defect case (`nodef`) and fourteen single
faults: `trunc-before` (file cut just before a box) and `trunc-within` (file cut
part-way into a box's own bytes; between them they name every possible cut
position exactly once), `zero-pay` (zero-length payload), `over-len` /
`under-len` (overstated / understated box length), `off-end` (offset past the
end), `dup-box` (duplicated box), `unk-box` (unknown box), `zero-fps` (zero
frame rate), `zero-dim` (zero dimension), `no-dim` (size absent from the container), `absurd-fps` (absurd frame rate),
`non-mono` (non-monotonic timestamps), `count-mis` (declared frame count
disagreeing with the payload), `reorder` (reordered boxes).

Constraints: faults addressing ISO boxes combine only with the ISO container.
Raw entries whose frame data would exceed a classic container's 32-bit size
limit are not generated.

Entry names are derived from the axis values, never hand-written: identical axes
yield identical names; any axis difference changes the name. The media extension
follows the container.

## 5. Operation

The workspace has zero dependencies; all commands below work offline.

```sh
cargo build --release -p synthvid-cli
cargo run --release -p synthvid-cli -- verify
```

`verify` with no `--corpus` regenerates the catalogue one entry at a time,
compares each manifest and media file against `catalog.lock`, and discards the
entry before generating the next. Nothing is written to disk; peak memory is one
entry's manifest and media bytes. To check a corpus that is already on disk,
generate it into a scratch directory (it is several gigabytes), copy the
committed `catalog.lock` into it, and run `verify --corpus DIR`; `generate` does
not write a lockfile.

- `generate --out DIR [--only NAME]... [--dry-run]`: write each entry's manifest
  and media file to `DIR`.
- `verify [--lock FILE] [--only NAME]...`: regenerate each entry in memory and
  compare its digests and lengths against `FILE` (default `catalog.lock` in the
  current directory); `--only` limits the check to the named entries and an
  unknown name is an error. Prints `ok NAME` per matching entry to stdout.
  Prints one line per difference and one line per entry that could not be
  checked to stderr. Prints a summary line to stdout with the count of entries
  compared and, if any could not be checked, that count. Lockfile lines the
  catalogue does not produce are reported as missing: all of them without
  `--only`, and those of the named entries with it. A lockfile that lists a name
  twice is rejected. Exit `0` if every entry matches and every entry could be
  checked; nonzero otherwise, including when the lockfile cannot be read.
- `verify --corpus DIR`: compare every file in `DIR` against `DIR/catalog.lock`.
  Exit `0` if all digests and lengths match; nonzero otherwise, printing one
  line per difference (missing, extra, invalid name, digest or length mismatch).
- `list [--format text|json]`: print all entry names.
- `manifest --name NAME`: print one entry's manifest to stdout.
- `lock --corpus DIR [--write] [--dry-run]`: print the lockfile text the current
  code produces; with `--write`, write it to `DIR/catalog.lock`.

## 6. Determinism

Guarantee: the same catalog entry yields byte-identical manifest and media files
on every platform and CPU architecture.

The guarantee holds by construction:

1. Authored quantities are exact rationals (`i64` over nonzero `i64`), checked
   and normalized. 30000/1001 is not representable as `f64`, and float
   formatting varies by platform and locale; no float appears in any serialized
   manifest.
2. Trigonometry is the in-crate `sin_turns`/`cos_turns`. The standard library
   `sin`, `cos`, and related functions dispatch to the platform math library and
   are not bit-identical between platforms. No floating-point type appears in
   the pure crates at all: every quantity is an integer or an exact rational,
   and CI rejects `f32` and `f64` there.
3. Collection iteration order is fixed: `BTreeMap`, `BTreeSet`, or sorted
   vectors. `HashMap`, `HashSet`, and `RandomState` are prohibited; their
   iteration order is randomized per process.
4. Motion is closed-form in the frame number. A value at frame N is a function
   of N, never an accumulation over frames 0..N; evaluation order (forward,
   backward, shuffled) does not affect results.
5. Accumulation order is fixed: work is partitioned by index and combined in
   index order, never in thread-completion order.
6. Library crates take no ambient input: no clocks, environment variables,
   process IDs, pointer addresses, system randomness, network, or filesystem
   access. Randomness comes only from the in-crate PRNG seeded from the scene
   description. Only `synthvid-cli` touches the filesystem.
7. All algorithms are implemented in-crate (PRNG, SHA-256, canonical JSON
   writer, JPEG encoder, both containers) with zero dependencies. A
   specification constrains an interface, not an algorithm: a dependency update
   that fixes a bug is indistinguishable from a regression, as both invalidate
   every recorded digest.
8. Failure is an error, never a substitution. Overflow, a failed transform, or
   an oversized payload returns an error naming what failed; no default, skip,
   or clamp is substituted. `verify` therefore confirms reproduction of the
   correct output, not identical reproduction of an incorrect one.

## 7. Layout

```text
crates/synthvid-scene/    numbers, geometry, motion, rendering (pure)
crates/synthvid-encode/   pixel coding and containers (pure)
crates/synthvid-video/    video specification, rendering, manifests (pure)
crates/synthvid-catalog/  declared corpus and its digests (pure)
crates/synthvid-cli/      the only crate that touches the filesystem
crates/synthvid-validate/ external-tool oracles used by tests only
```

## 8. License

MIT or Apache-2.0, at your option.
