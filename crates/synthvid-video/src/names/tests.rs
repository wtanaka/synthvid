use super::*;
use synthvid_testkit::{ensure, ensure_eq, Fail};

/// The position of a variant in the list that a test names it in, counted from zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Place(pub u16);

/// The number of variants of a closed name enum.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct VariantCount(pub u16);

/// Whether a closed name's text is non-empty and made of lowercase letters, digits, and
/// underscores only. Any other text needs an escape sequence in JSON, or lies outside the closed
/// alphabet, and fails.
fn ensure_plain_text(name: ClosedText) -> Result<(), Fail> {
    let ClosedText(text) = name;
    ensure!(!text.is_empty(), "empty closed name");
    for byte in text.bytes() {
        ensure!(
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_',
            "closed name {text} needs an escape sequence or has a disallowed character"
        );
    }
    Ok(())
}

/// Checks that a list names every variant of an enum, once each, in the order of its places.
///
/// The list must hold `count` names, and the name at each index must have that index as its place
/// in `place`. The places come from an exhaustive match, so a variant added to the enum fails to
/// compile until it is given a place, and the count and the list must then grow with it.
fn ensure_every_variant_listed<Name>(
    listed: &[Name],
    place: fn(Name) -> Place,
    count: VariantCount,
) -> Result<(), Fail>
where
    Name: Copy,
{
    let VariantCount(expected) = count;
    ensure_eq!(
        listed.len(),
        usize::from(expected),
        "the list must name every variant"
    );
    for (index, name) in listed.iter().enumerate() {
        let Place(position) = place(*name);
        ensure_eq!(
            usize::from(position),
            index,
            "a variant is listed out of place"
        );
    }
    Ok(())
}

/// The place of each `KindName`.
const fn kind_name_place(name: KindName) -> Place {
    match name {
        KindName::Sensor => Place(0),
        KindName::Film => Place(1),
        KindName::Lens => Place(2),
        KindName::Vignette => Place(3),
    }
}

/// The place of each `ApertureName`.
const fn aperture_name_place(name: ApertureName) -> Place {
    match name {
        ApertureName::F1_4 => Place(0),
        ApertureName::F2 => Place(1),
        ApertureName::F2_8 => Place(2),
        ApertureName::F4 => Place(3),
        ApertureName::F5_6 => Place(4),
        ApertureName::F8 => Place(5),
        ApertureName::F11 => Place(6),
        ApertureName::F16 => Place(7),
        ApertureName::F22 => Place(8),
    }
}

/// The place of each `ApertureKindName`.
const fn aperture_kind_name_place(name: ApertureKindName) -> Place {
    match name {
        ApertureKindName::Pinhole => Place(0),
        ApertureKindName::Iris => Place(1),
    }
}

/// The place of each `ApertureShapeKindName`.
const fn aperture_shape_kind_name_place(name: ApertureShapeKindName) -> Place {
    match name {
        ApertureShapeKindName::Disc => Place(0),
        ApertureShapeKindName::Polygon => Place(1),
    }
}

/// The place of each `FocusName`.
const fn focus_name_place(name: FocusName) -> Place {
    match name {
        FocusName::Infinity => Place(0),
    }
}

/// The place of each `CutoutName`.
const fn cutout_name_place(name: CutoutName) -> Place {
    match name {
        CutoutName::Disc => Place(0),
        CutoutName::Rect => Place(1),
        CutoutName::Polygon => Place(2),
    }
}

/// The place of each `VignetteKindName`.
const fn vignette_kind_name_place(name: VignetteKindName) -> Place {
    match name {
        VignetteKindName::None => Place(0),
    }
}

/// The place of each `ProjectionName`.
const fn projection_name_place(name: ProjectionName) -> Place {
    match name {
        ProjectionName::Perspective => Place(0),
    }
}

#[test]
fn every_closed_name_text_is_plain() {
    assert_eq!(check_every_closed_name_text_is_plain(), Ok(()));
}

fn check_every_closed_name_text_is_plain() -> Result<(), Fail> {
    // Each list names every variant of its enum, and the place of each variant is given by an
    // exhaustive match, so a missing or misplaced variant fails here.
    let kinds = [
        KindName::Sensor,
        KindName::Film,
        KindName::Lens,
        KindName::Vignette,
    ];
    ensure_every_variant_listed(&kinds, kind_name_place, VariantCount(4))?;
    for name in kinds {
        ensure_plain_text(name.as_str())?;
    }
    let apertures = [
        ApertureName::F1_4,
        ApertureName::F2,
        ApertureName::F2_8,
        ApertureName::F4,
        ApertureName::F5_6,
        ApertureName::F8,
        ApertureName::F11,
        ApertureName::F16,
        ApertureName::F22,
    ];
    ensure_every_variant_listed(&apertures, aperture_name_place, VariantCount(9))?;
    for name in apertures {
        ensure_plain_text(name.as_str())?;
    }
    let aperture_kinds = [ApertureKindName::Pinhole, ApertureKindName::Iris];
    ensure_every_variant_listed(&aperture_kinds, aperture_kind_name_place, VariantCount(2))?;
    for name in aperture_kinds {
        ensure_plain_text(name.as_str())?;
    }
    let shape_kinds = [ApertureShapeKindName::Disc, ApertureShapeKindName::Polygon];
    ensure_every_variant_listed(
        &shape_kinds,
        aperture_shape_kind_name_place,
        VariantCount(2),
    )?;
    for name in shape_kinds {
        ensure_plain_text(name.as_str())?;
    }
    let focuses = [FocusName::Infinity];
    ensure_every_variant_listed(&focuses, focus_name_place, VariantCount(1))?;
    for name in focuses {
        ensure_plain_text(name.as_str())?;
    }
    let cutouts = [CutoutName::Disc, CutoutName::Rect, CutoutName::Polygon];
    ensure_every_variant_listed(&cutouts, cutout_name_place, VariantCount(3))?;
    for name in cutouts {
        ensure_plain_text(name.as_str())?;
    }
    let vignettes = [VignetteKindName::None];
    ensure_every_variant_listed(&vignettes, vignette_kind_name_place, VariantCount(1))?;
    for name in vignettes {
        ensure_plain_text(name.as_str())?;
    }
    let projections = [ProjectionName::Perspective];
    ensure_every_variant_listed(&projections, projection_name_place, VariantCount(1))?;
    for name in projections {
        ensure_plain_text(name.as_str())?;
    }
    Ok(())
}

#[test]
fn closed_name_display_is_its_text() {
    assert_eq!(check_closed_name_display_is_its_text(), Ok(()));
}

fn check_closed_name_display_is_its_text() -> Result<(), Fail> {
    ensure!(KindName::Lens.to_string() == KindName::Lens.as_str().0);
    ensure!(ApertureName::F2_8.to_string() == "f2_8");
    ensure!(FocusName::Infinity.to_string() == "infinity");
    ensure!(CutoutName::Polygon.to_string() == "polygon");
    ensure!(ApertureShapeKindName::Disc.to_string() == "disc");
    ensure!(VignetteKindName::None.to_string() == "none");
    ensure!(ProjectionName::Perspective.to_string() == "perspective");
    Ok(())
}
