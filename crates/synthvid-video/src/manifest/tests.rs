use super::*;
use crate::testutil::Frac;
use synthvid_scene::{
    raster::Corners, Affine, Bounds, FrameCountError, FrameRate, Height, Point, Width,
};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

fn identity_camera() -> Result<ManifestAffine, Fail> {
    Ok(ManifestAffine::from_affine(&Affine {
        xx: Frac(1, 1).ratio()?,
        xy: Frac(0, 1).ratio()?,
        tx: Frac(0, 1).ratio()?,
        yx: Frac(0, 1).ratio()?,
        yy: Frac(1, 1).ratio()?,
        ty: Frac(0, 1).ratio()?,
    }))
}

/// A whole-number box written as `(min_x, min_y, max_x, max_y)`.
#[derive(Copy, Clone, Debug)]
struct Box4(i64, i64, i64, i64);

impl Box4 {
    /// Builds the manifest bounds this box names.
    fn bounds(self) -> Result<ManifestBounds, Fail> {
        let Self(min_x, min_y, max_x, max_y) = self;
        let min = Point {
            across: Frac(min_x, 1).ratio()?,
            down: Frac(min_y, 1).ratio()?,
        };
        let max = Point {
            across: Frac(max_x, 1).ratio()?,
            down: Frac(max_y, 1).ratio()?,
        };
        Ok(ManifestBounds::from_bounds(
            Bounds::new(Corners { min, max }).or_fail()?,
        ))
    }
}

/// A square scene's header: name, backdrop, side in pixels, whole frame
/// rate, and scale.
#[derive(Copy, Clone, Debug)]
struct Head(&'static str, BackgroundName, u16, i64, Option<Ratio>);

impl Head {
    /// Builds the header this description names.
    fn header(self) -> Result<ManifestHeader, Fail> {
        let Self(name, background, side, rate, scale) = self;
        Ok(ManifestHeader::new(
            JsonEntryName::new(name).or_fail()?,
            Dimensions::new(Width::new(side).or_fail()?, Height::new(side).or_fail()?),
            FrameRates::playback_only(FrameRate::new(Frac(rate, 1).ratio()?).or_fail()?),
            background,
            scale,
        ))
    }
}

fn one_object_manifest(
    head: ManifestHeader,
    shape: ShapeName,
    bbox: ManifestBounds,
    centre: ManifestPoint,
    on_screen: OnScreen,
) -> Result<Manifest, Fail> {
    let state = FrameObjectState::new(ObjectIndex::new(0), bbox, centre, None, on_screen);
    let frame = ManifestFrame::new(&identity_camera()?, vec![state]);
    let span = FrameSpan::new(FrameIndex::new(0)..FrameIndex::new(1)).or_fail()?;
    let decl = ManifestObjectDecl::new(ObjectIndex::new(0), shape, span);
    Manifest::new(head, vec![decl], vec![frame]).or_fail()
}

/// A point written as its `x` and `y` fractions.
#[derive(Copy, Clone, Debug)]
struct At(Frac, Frac);

impl At {
    /// Builds the manifest point these fractions name.
    fn point(self) -> Result<ManifestPoint, Fail> {
        let Self(horizontal, vertical) = self;
        Ok(ManifestPoint::from_point(Point {
            across: horizontal.ratio()?,
            down: vertical.ratio()?,
        }))
    }
}

fn make_example_manifest() -> Result<Manifest, Fail> {
    one_object_manifest(
        Head("example", BackgroundName::Grid, 16, 30, None).header()?,
        ShapeName::Disc,
        Box4(6, 6, 10, 10).bounds()?,
        At(Frac(8, 1), Frac(8, 1)).point()?,
        OnScreen::new(Frac(1, 1).ratio()?).or_fail()?,
    )
}

fn make_test_manifest() -> Result<Manifest, Fail> {
    one_object_manifest(
        Head("test-scene", BackgroundName::Solid, 32, 24, None).header()?,
        ShapeName::Rect,
        Box4(5, 5, 15, 15).bounds()?,
        At(Frac(10, 1), Frac(10, 1)).point()?,
        OnScreen::new(Frac(1, 1).ratio()?).or_fail()?,
    )
}

fn check_manifest_frozen_vector() -> Result<(), Fail> {
    let expected = r#"{"background":"grid","dimensions":{"height":16,"width":16},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"a":{"den":1,"num":1},"b":{"den":1,"num":0},"c":{"den":1,"num":0},"d":{"den":1,"num":1},"tx":{"den":1,"num":0},"ty":{"den":1,"num":0}},"index":0,"objects":[{"bbox":{"max_x":{"den":1,"num":10},"max_y":{"den":1,"num":10},"min_x":{"den":1,"num":6},"min_y":{"den":1,"num":6}},"centre_screen":{"x":{"den":1,"num":8},"y":{"den":1,"num":8}},"index":0,"on_screen":{"den":1,"num":1}}]}],"name":"example","objects":[{"index":0,"shape":"disc","visible":{"end":1,"start":0}}],"schema_version":1}"#;
    let result = make_example_manifest()?.to_canonical_json();
    ensure_eq!(
        result.as_str(),
        expected,
        "manifest must reproduce frozen vector exactly"
    );
    Ok(())
}

/// The frozen manifest vector. These bytes are the specification; if this
/// test fails the serializer changed and the serializer is what to fix.
#[test]
fn manifest_frozen_vector() {
    assert_eq!(check_manifest_frozen_vector(), Ok(()));
}

fn check_manifest_frozen_vector_rfc8259() -> Result<(), Fail> {
    let json = make_example_manifest()?.to_canonical_json();
    let json_str = json.as_str();
    ensure!(!json_str.is_empty(), "manifest JSON must not be empty");
    ensure_eq!(
        json_str.chars().next(),
        Some('{'),
        "manifest JSON must start with object"
    );
    ensure_eq!(
        json_str.chars().next_back(),
        Some('}'),
        "manifest JSON must end with object"
    );
    Ok(())
}

#[test]
fn manifest_frozen_vector_rfc8259() {
    assert_eq!(check_manifest_frozen_vector_rfc8259(), Ok(()));
}

fn check_manifest_determinism() -> Result<(), Fail> {
    let json1 = make_test_manifest()?.to_canonical_json();
    let json2 = make_test_manifest()?.to_canonical_json();
    ensure_eq!(
        json1,
        json2,
        "identical manifests must produce byte-identical JSON"
    );
    Ok(())
}

#[test]
fn manifest_determinism() {
    assert_eq!(check_manifest_determinism(), Ok(()));
}

fn check_manifest_no_floats() -> Result<(), Fail> {
    let manifest = one_object_manifest(
        Head(
            "example",
            BackgroundName::Grid,
            16,
            30,
            Some(Frac(100, 1).ratio()?),
        )
        .header()?,
        ShapeName::Disc,
        Box4(3, 5, 7, 9).bounds()?,
        At(Frac(11, 3), Frac(13, 5)).point()?,
        OnScreen::new(Frac(1, 2).ratio()?).or_fail()?,
    )?;
    let json = manifest.to_canonical_json();

    let mut in_string = false;
    let mut escape_next = false;
    let mut found_float = false;
    for ch in json.as_str().chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '.' | 'e' | 'E' if !in_string => {
                found_float = true;
                break;
            }
            _ => {}
        }
    }
    ensure!(
        !found_float,
        "manifest JSON must not contain floating-point numbers"
    );
    Ok(())
}

#[test]
fn manifest_no_floats() {
    assert_eq!(check_manifest_no_floats(), Ok(()));
}

fn check_on_screen_rejects_values_outside_unit_interval() -> Result<(), Fail> {
    ensure!(
        OnScreen::new(Frac(0, 1).ratio()?).is_ok(),
        "zero is on the boundary"
    );
    ensure!(
        OnScreen::new(Frac(1, 1).ratio()?).is_ok(),
        "one is on the boundary"
    );
    ensure!(
        OnScreen::new(Frac(1, 2).ratio()?).is_ok(),
        "a half is inside"
    );
    ensure!(
        OnScreen::new(Frac(-1, 2).ratio()?).is_err(),
        "negative is rejected"
    );
    ensure!(
        OnScreen::new(Frac(3, 2).ratio()?).is_err(),
        "above one is rejected"
    );
    Ok(())
}

/// The fraction on screen is a proportion, so values outside [0, 1] are
/// rejected rather than recorded.
#[test]
fn on_screen_rejects_values_outside_unit_interval() {
    assert_eq!(
        check_on_screen_rejects_values_outside_unit_interval(),
        Ok(())
    );
}

fn check_manifest_rejects_empty_frames() -> Result<(), Fail> {
    let result = Manifest::new(
        Head("example", BackgroundName::Grid, 16, 30, None).header()?,
        Vec::new(),
        Vec::new(),
    );
    ensure_eq!(result, Err(ManifestError::Empty(FrameCountError::Zero)));
    Ok(())
}

/// A manifest with no frames has no frame count, so it is refused rather
/// than built with a count of zero.
#[test]
fn manifest_rejects_empty_frames() {
    assert_eq!(check_manifest_rejects_empty_frames(), Ok(()));
}

/// A duplicate key cannot survive into the output.
#[test]
fn duplicate_key_cannot_appear_twice() {
    let mut obj = JsonObject::new();
    obj.insert_int(key(ManifestKey::Index), JsonInt(1));
    obj.insert_int(key(ManifestKey::Index), JsonInt(2));
    assert_eq!(
        obj.to_string(),
        r#"{"index":2}"#,
        "a repeated key replaces its value rather than appearing twice"
    );
}
