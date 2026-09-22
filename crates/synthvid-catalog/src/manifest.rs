//! Manifest types for serializing rendered scenes.
//!
//! A manifest describes the objects rendered, their bounding boxes, and camera
//! placement for each frame. It is serialized to canonical JSON and its digest
//! is recorded for reproducibility checking across architectures.
//!
//! Every type here keeps its fields private behind a constructor that takes
//! quantities already validated by `synthvid-scene`. A manifest is the last
//! place a value is written down before it is hashed and compared across
//! architectures, so a state that cannot occur in a scene must not become
//! expressible merely by being copied into one of these structs.

use crate::json_names::{JsonEntryName, JsonKey};
use crate::keys::ManifestKey;
use crate::names::{BackgroundName, ShapeName};
use crate::writer::{JsonArray, JsonObject};
use core::fmt;
use core::num::NonZeroI64;
use synthvid_scene::{
    Affine, Bounds, Dimensions, FrameCount, FrameRate, FrameSpan, ObjectIndex, Ratio,
};

/// Converts a manifest key to the writer's key type.
///
/// Infallible: the argument is one of a closed set, so there is no validation
/// left to fail and no panic path in this module.
fn key(k: ManifestKey) -> JsonKey {
    k.into()
}

/// A rational number as it appears in a manifest.
///
/// Always normalized with `gcd(|numer|, denom) == 1` and `denom > 0`, because
/// the only way to build one is from a [`Ratio`], which guarantees both.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestRatio {
    /// Numerator, carrying the sign if any.
    numer: i64,
    /// Denominator, strictly positive.
    denom: NonZeroI64,
}

impl ManifestRatio {
    /// Creates a manifest ratio from a normalized [`Ratio`].
    #[must_use]
    pub const fn from_ratio(r: Ratio) -> Self {
        Self {
            numer: r.numer(),
            denom: r.denom(),
        }
    }

    /// Serializes this ratio as a JSON object `{"den":D,"num":N}`.
    fn to_json_string(self) -> String {
        format!("{{\"den\":{},\"num\":{}}}", self.denom.get(), self.numer)
    }
}

/// Error returned when attempting to construct an invalid [`OnScreen`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OnScreenError {
    /// On-screen fraction must be within [0, 1].
    OutOfRange,
}

impl fmt::Display for OnScreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "on-screen fraction must be within [0, 1]"),
        }
    }
}

impl core::error::Error for OnScreenError {}

/// The fraction of an object's bounding box that lies inside the frame.
///
/// Strictly within `[0, 1]`. It is its own type because `1` and `0` are the
/// only interesting values and every number outside the interval is
/// meaningless; a bare ratio would let a manifest claim an object was five
/// times on screen.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OnScreen(ManifestRatio);

impl OnScreen {
    /// Creates a visible fraction, rejecting anything outside `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns `Err(OnScreenError::OutOfRange)` when `value` is negative or greater than one.
    pub fn new(value: Ratio) -> Result<Self, OnScreenError> {
        let zero = Ratio::from_integer(0);
        let one = Ratio::from_integer(1);
        (value >= zero && value <= one)
            .then(|| Self(ManifestRatio::from_ratio(value)))
            .ok_or(OnScreenError::OutOfRange)
    }

    /// Serializes as a JSON rational object.
    fn to_json_string(self) -> String {
        self.0.to_json_string()
    }
}

/// A point in a manifest (screen or world space).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestPoint {
    /// Horizontal coordinate.
    x: ManifestRatio,
    /// Vertical coordinate.
    y: ManifestRatio,
}

impl ManifestPoint {
    /// Creates a manifest point from two exact coordinates.
    #[must_use]
    pub const fn new(x: Ratio, y: Ratio) -> Self {
        Self {
            x: ManifestRatio::from_ratio(x),
            y: ManifestRatio::from_ratio(y),
        }
    }

    /// Serializes this point as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_raw(key(ManifestKey::X), self.x.to_json_string());
        obj.insert_raw(key(ManifestKey::Y), self.y.to_json_string());
        obj
    }
}

/// An axis-aligned bounding box in a manifest.
///
/// Built only from a [`Bounds`], whose constructor has already rejected an
/// inverted box. Public corner fields would have let one back in.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestBounds {
    /// Smallest horizontal coordinate.
    min_x: ManifestRatio,
    /// Smallest vertical coordinate.
    min_y: ManifestRatio,
    /// Largest horizontal coordinate.
    max_x: ManifestRatio,
    /// Largest vertical coordinate.
    max_y: ManifestRatio,
}

impl ManifestBounds {
    /// Creates manifest bounds from an already-ordered [`Bounds`].
    #[must_use]
    pub const fn from_bounds(b: Bounds) -> Self {
        Self {
            min_x: ManifestRatio::from_ratio(b.min_x),
            min_y: ManifestRatio::from_ratio(b.min_y),
            max_x: ManifestRatio::from_ratio(b.max_x),
            max_y: ManifestRatio::from_ratio(b.max_y),
        }
    }

    /// Serializes these bounds as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_raw(key(ManifestKey::MaxX), self.max_x.to_json_string());
        obj.insert_raw(key(ManifestKey::MaxY), self.max_y.to_json_string());
        obj.insert_raw(key(ManifestKey::MinX), self.min_x.to_json_string());
        obj.insert_raw(key(ManifestKey::MinY), self.min_y.to_json_string());
        obj
    }
}

/// A scene-to-screen transform in a manifest.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestAffine {
    /// Linear entry mapping input `x` to output `x`.
    a: ManifestRatio,
    /// Linear entry mapping input `y` to output `x`.
    b: ManifestRatio,
    /// Translation added to output `x`.
    tx: ManifestRatio,
    /// Linear entry mapping input `x` to output `y`.
    c: ManifestRatio,
    /// Linear entry mapping input `y` to output `y`.
    d: ManifestRatio,
    /// Translation added to output `y`.
    ty: ManifestRatio,
}

impl ManifestAffine {
    /// Creates a manifest transform from an [`Affine`].
    #[must_use]
    pub const fn from_affine(aff: Affine) -> Self {
        Self {
            a: ManifestRatio::from_ratio(aff.a),
            b: ManifestRatio::from_ratio(aff.b),
            tx: ManifestRatio::from_ratio(aff.tx),
            c: ManifestRatio::from_ratio(aff.c),
            d: ManifestRatio::from_ratio(aff.d),
            ty: ManifestRatio::from_ratio(aff.ty),
        }
    }

    /// Serializes this transform as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_raw(key(ManifestKey::A), self.a.to_json_string());
        obj.insert_raw(key(ManifestKey::B), self.b.to_json_string());
        obj.insert_raw(key(ManifestKey::C), self.c.to_json_string());
        obj.insert_raw(key(ManifestKey::D), self.d.to_json_string());
        obj.insert_raw(key(ManifestKey::Tx), self.tx.to_json_string());
        obj.insert_raw(key(ManifestKey::Ty), self.ty.to_json_string());
        obj
    }
}

/// What one object looked like in one frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FrameObjectState {
    /// Bounding box in screen space.
    bbox: ManifestBounds,
    /// Centre in screen space.
    centre_screen: ManifestPoint,
    /// Centre in world units, when the scene declares a scale.
    centre_world: Option<ManifestPoint>,
    /// Which object this is, in scene declaration order.
    index: ObjectIndex,
    /// Fraction of the bounding box inside the frame.
    on_screen: OnScreen,
}

impl FrameObjectState {
    /// Records one object's state for one frame.
    #[must_use]
    pub const fn new(
        index: ObjectIndex,
        bbox: ManifestBounds,
        centre_screen: ManifestPoint,
        centre_world: Option<ManifestPoint>,
        on_screen: OnScreen,
    ) -> Self {
        Self {
            bbox,
            centre_screen,
            centre_world,
            index,
            on_screen,
        }
    }

    /// Serializes this state as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_object(key(ManifestKey::Bbox), self.bbox.to_json_object());
        obj.insert_object(
            key(ManifestKey::CentreScreen),
            self.centre_screen.to_json_object(),
        );
        if let Some(world) = self.centre_world {
            obj.insert_object(key(ManifestKey::CentreWorld), world.to_json_object());
        }
        obj.insert_int(key(ManifestKey::Index), i64::from(self.index.get()));
        obj.insert_raw(key(ManifestKey::OnScreen), self.on_screen.to_json_string());
        obj
    }
}

/// One frame of a manifest.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestFrame {
    /// Scene-to-screen transform for this frame.
    camera: ManifestAffine,
    /// Objects on screen this frame, in scene declaration order.
    objects: Vec<FrameObjectState>,
}

impl ManifestFrame {
    /// Records one frame.
    #[must_use]
    pub const fn new(camera: ManifestAffine, objects: Vec<FrameObjectState>) -> Self {
        Self { camera, objects }
    }

    /// Serializes this frame as a JSON object with the given frame index for inclusion in a manifest.
    pub(crate) fn to_json_object(&self, frame_index: u32) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_object(key(ManifestKey::Camera), self.camera.to_json_object());
        obj.insert_int(key(ManifestKey::Index), i64::from(frame_index));
        let mut array = JsonArray::new();
        for state in &self.objects {
            array.push_object(state.to_json_object());
        }
        obj.insert_array(key(ManifestKey::Objects), array);
        obj
    }
}

/// An invariant object declaration.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestObjectDecl {
    /// Index in the scene declaration.
    index: ObjectIndex,
    /// Which shape this object is.
    shape: ShapeName,
    /// Frames this object is drawn in.
    ///
    /// A [`FrameSpan`], not a pair of integers. Its constructor rejects an
    /// empty span and an inverted one; two public `u32` fields made both
    /// representable again at the only point where the value is written down
    /// permanently.
    visible: FrameSpan,
}

impl ManifestObjectDecl {
    /// Declares one object.
    #[must_use]
    pub const fn new(index: ObjectIndex, shape: ShapeName, visible: FrameSpan) -> Self {
        Self {
            index,
            shape,
            visible,
        }
    }

    /// Serializes this object declaration as a JSON object for inclusion in a manifest.
    pub(crate) fn to_json_object(self) -> JsonObject {
        let mut obj = JsonObject::new();
        obj.insert_int(key(ManifestKey::Index), i64::from(self.index.get()));
        obj.insert_raw(
            key(ManifestKey::Shape),
            format!(r#""{}""#, self.shape.as_str()),
        );
        let mut visible = JsonObject::new();
        visible.insert_int(key(ManifestKey::End), i64::from(self.visible.end.get()));
        visible.insert_int(key(ManifestKey::Start), i64::from(self.visible.start.get()));
        obj.insert_object(key(ManifestKey::Visible), visible);
        obj
    }
}

/// The scene-level facts a manifest records, independent of any frame.
///
/// Grouped into their own type because positional parameters are a hazard:
/// it is a list a caller can misorder, and a signature no reader can check
/// at a glance. Frame count is derived from the manifest's frames rather than
/// stored here, making divergence unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManifestHeader {
    /// Which backdrop the scene declared.
    background: BackgroundName,
    /// Frame dimensions in pixels.
    dimensions: Dimensions,
    /// Exact frame rate.
    frame_rate: FrameRate,
    /// Catalogue entry name.
    name: JsonEntryName,
    /// Scene units per world unit, if declared.
    scale: Option<ManifestRatio>,
}

impl ManifestHeader {
    /// Records the scene-level facts.
    ///
    /// Every parameter is a type whose constructor has already refused the
    /// states a manifest must never contain: a zero dimension, a frame rate
    /// that is not an exact positive rational, a backdrop outside its closed set.
    /// Frame count is derived from the manifest's frames at construction, not
    /// passed here.
    #[must_use]
    pub const fn new(
        name: JsonEntryName,
        dimensions: Dimensions,
        frame_rate: FrameRate,
        background: BackgroundName,
        scale: Option<Ratio>,
    ) -> Self {
        Self {
            background,
            dimensions,
            frame_rate,
            name,
            scale: match scale {
                Some(r) => Some(ManifestRatio::from_ratio(r)),
                None => None,
            },
        }
    }
}

/// A complete manifest for a rendered scene.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Manifest {
    /// Scene-level facts.
    header: ManifestHeader,
    /// Frames in order. Never empty; an empty vector cannot form a valid frame count.
    frames: Vec<ManifestFrame>,
    /// Invariant object declarations.
    objects: Vec<ManifestObjectDecl>,
    /// Number of frames, derived from `frames.len()` to ensure consistency.
    frame_count: FrameCount,
}

impl Manifest {
    /// Assembles a manifest from a header, its objects, and its frames.
    ///
    /// Returns `None` if `frames` is empty, since `FrameCount` wraps `NonZeroU32`
    /// and an empty frame vector cannot have a valid count. Every other invariant
    /// is guaranteed by the parameter types.
    #[must_use]
    pub fn new(
        header: ManifestHeader,
        objects: Vec<ManifestObjectDecl>,
        frames: Vec<ManifestFrame>,
    ) -> Option<Self> {
        let frame_count = FrameCount::new(u32::try_from(frames.len()).ok()?).ok()?;
        Some(Self {
            header,
            frames,
            objects,
            frame_count,
        })
    }

    /// Serializes this manifest to canonical JSON.
    ///
    /// The output contains no whitespace outside string literals, keys are
    /// sorted by UTF-8 byte value, and every byte is stable across platforms.
    #[must_use]
    pub fn to_canonical_json(&self) -> String {
        let mut root = JsonObject::new();

        root.insert_raw(
            key(ManifestKey::Background),
            format!(r#""{}""#, self.header.background.as_str()),
        );

        let mut dimensions = JsonObject::new();
        dimensions.insert_int(
            key(ManifestKey::Height),
            i64::from(self.header.dimensions.height.get().get()),
        );
        dimensions.insert_int(
            key(ManifestKey::Width),
            i64::from(self.header.dimensions.width.get().get()),
        );
        root.insert_object(key(ManifestKey::Dimensions), dimensions);

        root.insert_int(
            key(ManifestKey::FrameCount),
            i64::from(self.frame_count.get().get()),
        );
        root.insert_ratio(key(ManifestKey::FrameRate), self.header.frame_rate.get());

        let mut frames_array = JsonArray::new();
        // Use frame_count to drive the loop, knowing it matches frames.len()
        for i in 0..self.frame_count.get().get() {
            let idx = usize::try_from(i).ok();
            if let Some(idx) = idx {
                if let Some(frame) = self.frames.get(idx) {
                    frames_array.push_object(frame.to_json_object(i));
                }
            }
        }
        root.insert_array(key(ManifestKey::Frames), frames_array);

        root.insert_string(key(ManifestKey::Name), &self.header.name);

        let mut objects_array = JsonArray::new();
        for obj_decl in &self.objects {
            objects_array.push_object(obj_decl.to_json_object());
        }
        root.insert_array(key(ManifestKey::Objects), objects_array);

        if let Some(scale) = self.header.scale {
            root.insert_raw(key(ManifestKey::Scale), scale.to_json_string());
        }

        root.insert_int(key(ManifestKey::SchemaVersion), 1);

        root.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroI64;
    use synthvid_scene::{FrameIndex, Height, Width};

    /// Builds an exact rational for a test fixture.
    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, NonZeroI64::new(d).unwrap()).ok().unwrap()
    }

    fn identity_camera() -> ManifestAffine {
        ManifestAffine::from_affine(Affine::new(
            r(1, 1),
            r(0, 1),
            r(0, 1),
            r(0, 1),
            r(1, 1),
            r(0, 1),
        ))
    }

    fn span(start: u32, end: u32) -> FrameSpan {
        FrameSpan::new(FrameIndex::new(start), FrameIndex::new(end)).unwrap()
    }

    fn dims(w: u16, h: u16) -> Dimensions {
        Dimensions::new(Width::new(w).unwrap(), Height::new(h).unwrap())
    }

    fn bounds(min_x: i64, min_y: i64, max_x: i64, max_y: i64) -> ManifestBounds {
        ManifestBounds::from_bounds(
            Bounds::new(r(min_x, 1), r(min_y, 1), r(max_x, 1), r(max_y, 1)).unwrap(),
        )
    }

    fn header(
        name: &str,
        background: BackgroundName,
        side: u16,
        rate: i64,
        scale: Option<Ratio>,
    ) -> ManifestHeader {
        ManifestHeader::new(
            JsonEntryName::new(name).unwrap(),
            dims(side, side),
            FrameRate::new(r(rate, 1)).unwrap(),
            background,
            scale,
        )
    }

    fn one_object_manifest(
        head: ManifestHeader,
        shape: ShapeName,
        bbox: ManifestBounds,
        centre: ManifestPoint,
        on_screen: OnScreen,
    ) -> Manifest {
        let state = FrameObjectState::new(ObjectIndex::new(0), bbox, centre, None, on_screen);
        let frame = ManifestFrame::new(identity_camera(), vec![state]);
        let decl = ManifestObjectDecl::new(ObjectIndex::new(0), shape, span(0, 1));
        Manifest::new(head, vec![decl], vec![frame]).expect("manifest with one frame must be valid")
    }

    fn make_example_manifest() -> Manifest {
        one_object_manifest(
            header("example", BackgroundName::Grid, 16, 30, None),
            ShapeName::Disc,
            bounds(6, 6, 10, 10),
            ManifestPoint::new(r(8, 1), r(8, 1)),
            OnScreen::new(r(1, 1)).unwrap(),
        )
    }

    fn make_test_manifest() -> Manifest {
        one_object_manifest(
            header("test-scene", BackgroundName::Solid, 32, 24, None),
            ShapeName::Rect,
            bounds(5, 5, 15, 15),
            ManifestPoint::new(r(10, 1), r(10, 1)),
            OnScreen::new(r(1, 1)).unwrap(),
        )
    }

    /// The frozen manifest vector. These bytes are the specification; if this
    /// test fails the serialiser changed and the serialiser is what to fix.
    #[test]
    fn test_manifest_frozen_vector() {
        let expected = r#"{"background":"grid","dimensions":{"height":16,"width":16},"frame_count":1,"frame_rate":{"den":1,"num":30},"frames":[{"camera":{"a":{"den":1,"num":1},"b":{"den":1,"num":0},"c":{"den":1,"num":0},"d":{"den":1,"num":1},"tx":{"den":1,"num":0},"ty":{"den":1,"num":0}},"index":0,"objects":[{"bbox":{"max_x":{"den":1,"num":10},"max_y":{"den":1,"num":10},"min_x":{"den":1,"num":6},"min_y":{"den":1,"num":6}},"centre_screen":{"x":{"den":1,"num":8},"y":{"den":1,"num":8}},"index":0,"on_screen":{"den":1,"num":1}}]}],"name":"example","objects":[{"index":0,"shape":"disc","visible":{"end":1,"start":0}}],"schema_version":1}"#;
        let result = make_example_manifest().to_canonical_json();
        assert_eq!(
            result, expected,
            "manifest must reproduce frozen vector exactly"
        );
    }

    #[test]
    fn test_manifest_frozen_vector_rfc8259() {
        let json_str = make_example_manifest().to_canonical_json();
        assert!(!json_str.is_empty(), "manifest JSON must not be empty");
        assert_eq!(
            json_str.chars().next(),
            Some('{'),
            "manifest JSON must start with object"
        );
        assert_eq!(
            json_str.chars().next_back(),
            Some('}'),
            "manifest JSON must end with object"
        );
    }

    #[test]
    fn test_manifest_determinism() {
        let json1 = make_test_manifest().to_canonical_json();
        let json2 = make_test_manifest().to_canonical_json();
        assert_eq!(
            json1, json2,
            "identical manifests must produce byte-identical JSON"
        );
    }

    #[test]
    fn test_manifest_no_floats() {
        let manifest = one_object_manifest(
            header("example", BackgroundName::Grid, 16, 30, Some(r(100, 1))),
            ShapeName::Disc,
            bounds(3, 5, 7, 9),
            ManifestPoint::new(r(11, 3), r(13, 5)),
            OnScreen::new(r(1, 2)).unwrap(),
        );
        let json_str = manifest.to_canonical_json();

        let mut in_string = false;
        let mut escape_next = false;
        let mut found_float = false;
        for ch in json_str.chars() {
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
        assert!(
            !found_float,
            "manifest JSON must not contain floating-point numbers"
        );
    }

    /// The fraction on screen is a proportion, so values outside [0, 1] are
    /// rejected rather than recorded.
    #[test]
    fn test_on_screen_rejects_values_outside_unit_interval() {
        assert!(OnScreen::new(r(0, 1)).is_ok(), "zero is on the boundary");
        assert!(OnScreen::new(r(1, 1)).is_ok(), "one is on the boundary");
        assert!(OnScreen::new(r(1, 2)).is_ok(), "a half is inside");
        assert!(OnScreen::new(r(-1, 2)).is_err(), "negative is rejected");
        assert!(OnScreen::new(r(3, 2)).is_err(), "above one is rejected");
    }

    /// A duplicate key cannot survive into the output.
    #[test]
    fn test_duplicate_key_cannot_appear_twice() {
        let mut obj = JsonObject::new();
        obj.insert_int(key(ManifestKey::Index), 1);
        obj.insert_int(key(ManifestKey::Index), 2);
        assert_eq!(
            obj.build(),
            r#"{"index":2}"#,
            "a repeated key replaces its value rather than appearing twice"
        );
    }
}
